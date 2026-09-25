# 01 — ONNX export spike: evidence

Task: `tasks/mvp/01-prove-the-onnx-export-against-stock-apollo.md`.

Measured on 2026-09-25 on the owner's machine: AMD Ryzen 7 6800H, 8 physical cores, 16 logical, 15.2 GiB RAM, Windows 11. All runs used `C:\Lab\Apollo\asserts\input_wav.wav` (stereo, 44.1 kHz, 6.0 s, SHA-256 `3c9a0539…ddf771`). Stock torch means the vendored `apollo.py` with the pinned checkpoint.

In this document, "mono" means one channel as the model input. The engine runs L, then R, through the model and joins them into a stereo file (Stack §C, task 04). The product output keeps the channel layout of the source.

## Sources of each number

| Command | Numbers |
|---|---|
| `uv run --project tools python tools/export/export.py` | Artifact metadata |
| `uv run --project tools python tools/export/measure.py` | Parity, RTF, channel independence, peak working set, determinism |
| `tools/export/run_stock.py` in `tools/oldtorch` and in `tools` | Old against new torch |

The torch channel-independence row and the torch memory row came from one-off snippets that call the same functions. The next sections name them.

## Environment

| Item | Value |
|---|---|
| Python | 3.14.7 (`tools`), 3.10.21 (`tools/oldtorch`) |
| torch | 2.11.0+cpu (`tools`), 2.0.0+cpu (`tools/oldtorch`) |
| ONNX Runtime | 1.30.0, `CPUExecutionProvider` |
| Threads | 8 intra-op threads for ONNX Runtime and torch (the physical core count) |
| Checkpoint | `JusperLee/Apollo` revision `c68bd80f`, SHA-256 `99d9af7f…8d76`, verified |

## Artifact (scope 4, 10)

| Item | Value |
|---|---|
| File | `tools/export/artifacts/apollo.onnx` (gitignored) |
| Size | 77,649,552 bytes |
| SHA-256 | `fdf62f6c0ba5daf3a38c1761443456e4656707218b91a19cdad41a5d4bad78b9` |
| Opset | `ai.onnx` 20, IR version 10 |
| Graph | 3954 nodes, 858 initializers |
| Input, output | `audio` and `audio_out`, shape `(batch, channels, samples)`, all three axes dynamic |
| Exporter | `torch.onnx.export(dynamo=True)` with onnxscript |

### Variant (scope 5)

The in-graph DSP variant wins. It passes parity and speed, so the model-core-only fallback was not built. The STFT is a DFT-basis `conv1d` over a reflect-padded signal with a periodic Hann window. The iSTFT is a `conv_transpose1d` overlap-add with window-envelope normalization. The DFT kernels are built in float64 with the angle reduced as `(k * n) % win`, then cast to float32. Float32 angles add 1.35e-3 of STFT error, and the band normalization amplifies it to about 1.1 at the output.

### Determinism

Two exports produce byte-identical files. Four exports in four separate processes confirmed it: two through the script and two through `export_to` from the package.

An earlier handoff reported two different hashes (`4c9b4a74…` and `fdf62f6c…`). The cause was the import path. The script imported `real_dsp`, and the package imported `export.real_dsp`. The dynamo exporter writes the module path into each node's `namespace` metadata. The initializers and the graph were identical after the metadata was cleared. `export.py` now imports `export.real_dsp` on both paths. `tests/test_determinism.py` guards this. The `measure.py` replay export matched the artifact hash.

## Parity (scope 6, criterion 1)

| Comparison, 6 s stereo | Max abs | RMSE |
|---|---|---|
| ONNX Runtime stereo batch against stock torch stereo | 3.611e-4 | 1.982e-5 |
| ONNX Runtime one channel at a time against stock torch stereo (shipped path) | 3.611e-4 | 1.978e-5 |
| Limit | 1e-3 | 1e-4 |

For scale, CUDA against CPU torch gives max abs 7.2e-4 and RMSE 6.4e-5 (Stack §K).

## Real-time factor (scope 6, criterion 2)

### Method

`measure.py` loads torch and ONNX Runtime in one process and warms both on 1 s. Each duration then runs 7 rounds. Each round runs both engines once and flips the order every round, so a thermal drift affects both engines equally. The table gives the median RTF of each engine and the median of the per-round ratios. RTF is wall time divided by audio duration.

### Results

| Input | torch RTF | ONNX RTF | Ratio median | Ratio range |
|---|---|---|---|---|
| Mono 1 s | 1.113 | 1.039 | 0.925 | 0.824–1.057 |
| Mono 3 s | 1.136 | 1.063 | 0.941 | 0.902–0.980 |
| Mono 6 s | 1.099 | 1.060 | 0.973 | 0.947–1.014 |
| Stereo 1 s | 2.077 | 2.079 | 1.023 | 0.918–1.052 |
| Stereo 3 s | 1.949 | 2.020 | 1.038 | 1.026–1.039 |
| Stereo 6 s | 2.069 | 2.522 | 1.202 | 0.784–1.628 |
| Limit | | | 1.2 | |

The mono rows are the shipped path: the engine feeds the model one channel at a time and still writes stereo. The stereo rows feed both channels in one batch, which the product does not do. The stereo 6 s row measures paging, not compute. A per-round log showed 0.57 to 3.58 GB of free RAM and 1.2 to 7.4 million page faults per round. The ONNX Runtime arena keeps its 7.35 GB peak while torch allocates about 3.8 GB in the same process. Mono 6 s showed near zero page faults per round after the first.

The Stack §K baseline (torch RTF 2.70 at 3 s stereo) used torch 2.11.0+cu130 in another environment. This run measured 1.95 with the CPU wheel. The ratio uses only numbers from the same run.

## Channel independence (scope 8, criterion 3)

Stereo batch against L and R run separately, 6 s real audio.

| Engine | Threads | L max abs | R max abs |
|---|---|---|---|
| ONNX Runtime | 1 | 4.17e-7 | 3.58e-7 |
| ONNX Runtime | 4 | 2.83e-7 | 2.38e-7 |
| ONNX Runtime | 8 | 1.55e-5 | 1.14e-5 |
| Stock torch | 8 | 4.17e-7 | 3.43e-7 |
| Export wrapper in torch | 8 | 4.77e-7 | 5.36e-7 |
| Limit | | 1e-6 | |

The torch rows came from a snippet that runs `load_apollo` and `build_wrapper` on the same segment.

The model math is channel-independent: `apollo.py` folds channels into the batch. The 8-thread difference does not come from channel coupling. A mono input shows the same difference when only the thread count changes:

| ONNX Runtime, mono 6 s | Max abs |
|---|---|
| 8 threads against 1 thread | 1.56e-5 |
| 4 threads against 1 thread | 0.0 |
| Same input run twice at 1, 4, or 8 threads | 0.0 |

So the 8-thread row measures a thread-count effect. The likely cause is a different reduction partition at 8 threads. This cause is inferred. No one traced it into the ONNX Runtime kernels. The difference is −96 dBFS, which is half of one 16-bit LSB (3.05e-5). It is 23 times smaller than the export's own parity error.

Consequence for task 04: at a fixed thread count, the output is bit-exact from run to run. Across thread counts, it can move by 1.6e-5. A golden test must compare with a tolerance, not a byte hash. Reading of criterion 3, decided by the owner on 2026-09-25: the criterion tests the model property. It passes in stock torch and in ONNX Runtime at 1 to 4 threads. The owner accepts the thread-count variation as a known limitation (Stack §C).

`tests/test_parity.py` checks both sides. `test_model_is_channel_independent` compares at 1 thread on 6 s of noise against 1e-6. `test_shipped_path_matches_stock_stereo` compares ONNX Runtime per channel, at the default thread count, against stock torch stereo at the criterion 1 limits.

### Thread-count sweep

Mono 6 s, one session per thread count, single-shot timing:

| Threads | Max abs against 1 thread | RTF |
|---|---|---|
| 1 | 0.0 | 4.17 |
| 2 | 0.0 | 2.04 |
| 3 | 0.0 | 3.15 |
| 4 | 0.0 | 1.33 |
| 5 | 0.0 | 1.19 |
| 6 | 1.56e-5 | 1.10 |
| 7 | 1.63e-5 | 1.09 |
| 8 | 1.56e-5 | 1.10 |
| 12 | 3.37e-5 | 1.15 |
| 16 | 3.37e-5 | 1.21 |

The 3-thread RTF is out of line with its neighbors. A single shot is noisy. `SessionOptions.use_deterministic_compute = True` gave the same differences at 8 and 16 threads.

The output can also change with the CPU instruction set (AVX2, AVX-512, ARM), because ONNX Runtime picks its kernels at run time. This is inferred. One machine cannot test it. CUDA against CPU differs by 7.2e-4 (Stack §K).

## Peak working set (scope 7)

Each point runs in a fresh process: create the session, run once, read `psutil` `peak_wset`.

| Seconds | Mono peak (bytes) | Stereo peak (bytes) |
|---|---|---|
| 1 | 953,479,168 | 1,550,295,040 |
| 2 | 1,545,056,256 | 2,699,018,240 |
| 4 | 2,693,656,576 | 4,995,022,848 |
| 6 | 3,850,375,168 | 7,354,900,480 |

- Slope: 579,379,200 bytes per channel-second (mono) and 580,460,544 (stereo). That is about 0.54 GiB per channel-second.
- The fit is linear. The step-to-step slopes stay between 574 and 592 MB per channel-second.
- The process holds about 363 MB after the session loads. Before the session it holds about 226 MB.
- Model for task 05: peak ≈ 0.36 GB + 0.58 GB × channel-seconds.
- Turning off the CPU memory arena and memory patterns gave 542 MB per channel-second at mono 2 s and 6 s. The arena is not the cause.
- Stock torch in the same harness gave about 258 MB per channel-second (mono 2 s and 6 s). ONNX Runtime uses about 2.2 times the torch memory. Task 05 cites the torch value from Stack §K (0.29 GiB). It must use the ONNX Runtime value.

## Old against new torch (scope 9)

`tools/oldtorch` pins torch 2.0.0+cpu, Python 3.10, and `numpy<2`. `run_stock.py` runs stock Apollo on the full WAV in each environment.

| Comparison | Max abs | RMSE |
|---|---|---|
| torch 2.0.0 against torch 2.11.0 | 3.58e-7 | 3.26e-8 |

## Model math (criterion 4)

`RealApollo` (`tools/export/real_dsp.py`) reuses the loaded `BN`, `net`, and `output` submodules. Only the STFT and iSTFT change form. The vendored files match upstream `e84bcac` byte for byte (`vendor/PROVENANCE.md`). The export wrapper in torch matches stock torch within 1e-4 on 6 s of noise (`test_in_graph_dsp_matches_stock_apollo`).

## Open risks

- The export dependencies are in the `export` uv group, which installs by default. CI sets `UV_NO_GROUP=export`, so it never installs torch, and the export tests skip. Local `pnpm verify` runs them in about 4 minutes. Nothing checks the export on Linux.
- The peak memory figures are for this machine and ONNX Runtime 1.30.0. Another thread count or version can change them.
