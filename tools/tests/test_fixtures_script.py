"""Guards for the dev-only fixture generator.

The script itself needs ffmpeg, so these checks stay static: they confirm the script parses and
still declares the source, the reference settings, and every format the decoder claims to
support. The decode behavior is covered by the Rust suite.
"""

import ast
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "fixtures" / "make_fixtures.py"

FORMAT_FIXTURES = [
    "wav_44100.wav",
    "aiff_44100.aiff",
    "flac_44100.flac",
    "mp3_44100.mp3",
    "aac_44100.m4a",
    "alac_44100.m4a",
    "vorbis_44100.ogg",
    "opus_48000.opus",
    "opus_48000.webm",
]

AWKWARD_NAMES = ["Florida!!!.opus", "Habits (Stay High).opus", "Sigur Rós.flac"]


def test_fixture_script_parses_and_declares_every_format() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    ast.parse(source)

    for name in FORMAT_FIXTURES:
        assert name in source, f"the fixture script does not build {name}"
    for name in AWKWARD_NAMES:
        assert name in source, f"the fixture script does not build {name}"


def test_fixture_script_records_the_swr_reference_settings() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    assert "filter_size=256:phase_shift=24:cutoff=0.98" in source
    assert "input_wav.wav" in source
    assert "CC BY-SA 4.0" in source
