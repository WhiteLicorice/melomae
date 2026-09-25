import numpy as np
import pytest
import torch

from export.export import (
    build_onnx_session,
    build_wrapper,
    ensure_checkpoint,
    export_to,
    load_apollo,
)

SAMPLE_RATE = 44100
PARITY_SECONDS = 6.0
CHANNEL_SECONDS = 6.0


def _fixture(seconds, seed=0):
    """A deterministic broadband stereo fixture.

    Broadband content matters. The Apollo band normalization divides each band
    by its power. A pure tone leaves most bands near the epsilon floor, where
    a small transform difference is amplified. White noise keeps every band
    above the floor, so the fixture measures the transform, not the floor.
    """
    count = int(round(seconds * SAMPLE_RATE))
    rng = np.random.default_rng(seed)
    audio = rng.standard_normal((1, 2, count)).astype(np.float32) * 0.2
    return torch.from_numpy(audio)


def _run_ort(session, audio):
    return session.run(None, {"audio": audio.numpy()})[0]


@pytest.fixture(scope="session")
def checkpoint():
    path = ensure_checkpoint()
    if path is None:
        pytest.skip("The Apollo checkpoint is unavailable and cannot be downloaded.")
    return path


@pytest.fixture(scope="session")
def apollo(checkpoint):
    return load_apollo(checkpoint)


@pytest.fixture(scope="session")
def onnx_path(checkpoint, tmp_path_factory):
    output = tmp_path_factory.mktemp("onnx") / "apollo.onnx"
    export_to(output, checkpoint=checkpoint)
    return output


@pytest.fixture(scope="session")
def ort_session(onnx_path):
    return build_onnx_session(onnx_path)


def test_wrapper_keeps_the_apollo_shape(apollo):
    wrapper = build_wrapper(apollo)
    assert wrapper.win == 882
    assert wrapper.enc_dim == 442
    assert len(wrapper.band_width) == 80


def test_in_graph_dsp_matches_stock_apollo(apollo):
    audio = _fixture(PARITY_SECONDS)
    wrapper = build_wrapper(apollo).eval()
    with torch.no_grad():
        stock = apollo(audio)
        real = wrapper(audio)
    assert (stock - real).abs().max().item() <= 1e-4


def test_onnx_matches_torch_within_tolerance(apollo, ort_session):
    audio = _fixture(PARITY_SECONDS)
    with torch.no_grad():
        stock = apollo(audio).numpy()
    onnx = _run_ort(ort_session, audio)
    diff = np.abs(stock - onnx)
    max_abs = float(diff.max())
    rmse = float(np.sqrt(np.mean(diff**2)))
    assert max_abs <= 1e-3, f"max abs {max_abs}"
    assert rmse <= 1e-4, f"rmse {rmse}"


def test_model_is_channel_independent(onnx_path):
    # One thread fixes the reduction order. At 6 or more threads, ONNX
    # Runtime changes the output by about 1.6e-5 even for one channel, so a
    # multi-thread comparison measures the thread count, not the model.
    session = build_onnx_session(onnx_path, intra_op_threads=1)
    audio = _fixture(CHANNEL_SECONDS)
    stereo = _run_ort(session, audio)
    left = _run_ort(session, audio[:, :1])
    right = _run_ort(session, audio[:, 1:])
    assert np.abs(stereo[:, :1] - left).max() <= 1e-6
    assert np.abs(stereo[:, 1:] - right).max() <= 1e-6


def test_shipped_path_matches_stock_stereo(apollo, ort_session):
    # The engine runs one channel at a time (Stack §C).
    audio = _fixture(PARITY_SECONDS)
    with torch.no_grad():
        stock = apollo(audio).numpy()
    shipped = np.concatenate(
        [_run_ort(ort_session, audio[:, :1]), _run_ort(ort_session, audio[:, 1:])], axis=1
    )
    diff = np.abs(stock - shipped)
    max_abs = float(diff.max())
    rmse = float(np.sqrt(np.mean(diff**2)))
    assert max_abs <= 1e-3, f"max abs {max_abs}"
    assert rmse <= 1e-4, f"rmse {rmse}"
