# 01 — Prove the ONNX export against stock Apollo

**Status:** DONE
**Phase:** 0 — Foundation and go/no-go spikes
**Depends on:** 00
**SRS:** §6, §12
**Stack:** §C, §K
**Assigned to:** Agent
**Started:** 2026-09-25 — Base commit 2c4ddcb, clean tree. Baseline gate: `python tasks/validate_board.py` prints "Task board valid: 41 records."; `python -m unittest discover -s tasks -p "test_*.py"` prints 9 passed.
**Outcome:** GO, DONE 2026-09-25. Evidence: `docs/evidence/01-onnx-spike.md`. Variant: in-graph real-arithmetic DSP. Artifact: 77,649,552 bytes, SHA-256 `fdf62f6c…78b9`, opset 20, ONNX Runtime 1.30.0. Two exports are byte-identical. Criterion 1: max abs 3.611e-4, RMSE 1.982e-5 on the 6 s WAV. One channel at a time gives 3.611e-4 and 1.978e-5. Criterion 2: ORT/torch RTF ratio medians over 7 alternating rounds, one channel at a time: 0.925 (1 s), 0.941 (3 s), 0.973 (6 s). Stereo batch: 1.023, 1.038, and 1.202 at 6 s. That last run paged and does not measure compute. Criterion 3: the owner chose the model reading on 2026-09-25. Stock torch gives 4.2e-7, and ORT gives 4.2e-7 at 1 thread and 2.8e-7 at 4 threads. ORT at 8 threads gives 1.55e-5, and a single channel moves by the same amount when only the thread count changes. The owner accepted the cross-machine variation as a known limitation (Stack §C). Criterion 4: `RealApollo` reuses `BN`, `net`, and `output`. Peak working set: about 0.36 GB plus 0.58 GB per channel-second. Torch 2.0.0 against 2.11.0: max abs 3.58e-7. Gate: `pnpm verify` exit 0. Board 41 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 1 passed, pytest 8 passed in 377 s. Red-first: `test_parity.py` failed with `ModuleNotFoundError: No module named 'export.export'`. `test_measure.py` failed with a peak of 0. `test_determinism.py` got `real_dsp` where it expected `export.real_dsp`. The torch-free CI run failed collection on the missing numpy. CI: the owner chose to skip the export tests in CI. The export dependencies are in the `export` uv group, and CI sets `UV_NO_GROUP=export`. The workflow also caches Rust, uv, and apt. No GitHub run exercised these changes yet.

This task is a spike. It ends with `GO` or `NO-GO`.

## Goal

Prove that Apollo, exported to ONNX and run by ONNX Runtime on CPU, matches stock torch in output and speed. If it does not, send the board to the Python sidecar fallback (Stack §C).

## Evidence

- `C:\Lab\Apollo\look2hear\models\apollo.py:253-254` calls `torch.stft` with complex output. `:295-296` calls `torch.istft`. ONNX does not export complex tensors, and it has no iSTFT operator.
- `apollo.py:132` uses `F.scaled_dot_product_attention`.
- Model constants: `win` = 882 samples, hop 441, 80 bands (`apollo.py:218-228`).
- `base_model.py:64-66` loads the checkpoint with `torch.load(weights_only=False)`.
- The checkpoint SHA-256 is `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76`.
- Baselines (Stack §K): torch CPU RTF 2.70 at 3 s stereo. CUDA vs CPU differ by max abs 7.2e-4 and RMSE 6.4e-5.
- The math is channel-independent. `apollo.py:251-253,292,296` fold channels into the batch.

## Scope

1. Vendor `look2hear/models/apollo.py` and `look2hear/models/base_model.py` unmodified from upstream `JusperLee/Apollo@e84bcac` into `tools/export/vendor/`. Add a `PROVENANCE.md` with the URL, the commit, the SHA-256 of each file, and the CC BY-SA 4.0 notice. Upstream `apollo.py` prints the band widths at init. Keep that line.
2. Download the checkpoint through `huggingface_hub` at a pinned revision. Verify its SHA-256.
3. Write an export wrapper that reuses the loaded submodules (`BN`, `net`, `output`). Express the STFT and iSTFT in real arithmetic:
   - a DFT-basis `conv1d`
   - reflect padding (`center=True`)
   - a periodic Hann window
   - overlap-add with window-envelope normalization
4. Export audio → audio with a dynamic sample axis at the current default opset.
5. If in-graph DSP fails parity or speed, export the model core only. The DSP then moves to Rust in task 04. Record which variant wins, and why.
6. Measure ONNX Runtime CPU against torch CPU in the same session at 1, 3, and 6 s. Set the intra-op threads to the physical core count.
7. Measure the peak working set per channel-second for ONNX Runtime. Task 05 uses this number.
8. Measure channel independence: a stereo batch against L and R run separately.
9. Measure old against new torch. Build a uv environment with `torch==2.0.0` CPU on Python 3.10. Compare its output to torch 2.11 on `asserts/input_wav.wav`. The briefer asked for this.
10. Record the ONNX file's SHA-256, size, opset, and ONNX Runtime version. Record whether two exports produce identical files.
11. Write all results to `docs/evidence/01-onnx-spike.md`.

## GO criteria

All four must hold:

- [x] ONNX Runtime CPU output vs torch CPU on the 6 s fixture: max abs ≤ 1e-3 and RMSE ≤ 1e-4.
- [x] ONNX Runtime CPU RTF ≤ 1.2 × torch CPU RTF, measured in the same session.
- [x] Stereo batch vs separate channels: max abs ≤ 1e-6.
- [x] The export changes no model math. Only the STFT and iSTFT change their form.

## NO-GO path

Set task 04 to `BLOCKED`. Write a board amendment for the Python sidecar. Stop and report to the owner.

## Out of scope

- Rust inference (task 04)
- GPU execution providers (task 08)
- Quantization (backlog B-004)

## Test-first plan

1. Write `tools/export/tests/test_parity.py`. It asserts the ONNX vs torch tolerance and the channel independence. Run it. It fails because `export.py` does not exist.
2. Implement the export. Run the test again.

## Acceptance criteria

- [x] `Outcome` records `GO` or `NO-GO` with the measured numbers.
- [x] `docs/evidence/01-onnx-spike.md` holds every measurement in the scope.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
uv run --project tools python tools/export/export.py
uv run --project tools pytest tools/export
pnpm verify
```
