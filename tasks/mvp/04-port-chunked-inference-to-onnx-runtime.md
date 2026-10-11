# 04 — Port chunked inference to ONNX Runtime

**Status:** DONE
**Phase:** 1 — Restoration engine
**Depends on:** 01, 03
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** 2026-10-11 — Base commit 0049784, clean tree. Baseline gate at 0049784: board 53 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 98 passed, pytest 13 passed.
**Outcome:** DONE 2026-10-11. `melomae_core::inference` ports `resolve_chunking`, `chunk_starts`, `crossfade_weights`, and `run_model` of `C:\Lab\Apollo\inference.py` with batch size one. The `Model` trait takes one channel at a time. `OnnxBackend` is the chunked engine over the `ort` session for `models/apollo.onnx`, and it is the only `RestoreBackend` implementation. Scope item 2 does not apply: the task 01 `Outcome` chose the in-graph DSP export, so the STFT and iSTFT are inside `apollo.onnx` and no Rust STFT exists. Version: `ort` 2.0.0-rc.13, license MIT OR Apache-2.0, current on crates.io on 2026-10-11. It gets ONNX Runtime from the `download-binaries` default feature, which fetches the prebuilt libraries at build time, so `windows-latest` and `ubuntu-22.04` need no manual install. Parity on the 18 s input, three copies of the committed input, with chunk 6 s, overlap 1 s, and pad 1 s against the stock golden: channel 0 max abs 3.611e-4 and RMSE 2.269e-5, channel 1 max abs 3.089e-4 and RMSE 1.851e-5, against the §12.1 limits 1e-3 and 1e-4. Progress reports chunk n of m and the channel. A cancel returns `RestoreOutcome::Cancelled` after three model calls when the flag is set during chunk 2, so the run stops within one chunk. Scope 8 and the no-partial-file half of the cancel criterion moved to tasks 06 and 07 by orchestrator ruling on 2026-10-11, because the backend returns channels (Stack §C, SRS §7 item 5, task 07 scope 4). The model, the golden, and the input are committed through Git LFS, and `git lfs ls-files` lists all three. Red-first: `cargo test -p melomae-core inference` failed with `unresolved import melomae_core::inference` before the module existed. Gate: `pnpm verify` exit 0. Board 53 records, tasks unittest 9 passed, Vitest 1 passed, cargo test 112 passed, pytest 14 passed. These are local counts. Evidence: `SCRATCH/evidence-04/`.

## Goal

Run `apollo.onnx` with ONNX Runtime on CPU and match the chunked output of stock `inference.py`.

## Evidence

- `C:\Lab\Apollo\inference.py:86-104` `resolve_chunking` converts seconds to samples and validates the overlap.
- `:118-124` `chunk_starts` covers the signal with no redundant last chunk.
- `:127-137` `crossfade_weights` builds linear ramps with `torch.linspace(0, 1, n)`.
- `:140-221` `run_model` runs padded chunks. It clamps the last padded start to the file end (`:178-180`), crops each padded output, and normalizes the overlap-add.
- `C:\Lab\Apollo\tests\test_inference.py` tests the algorithm with fake models: `IdentityModel`, `ShortOutputModel`, and `DegradedEdgesModel`.
- Task 01 confirmed channel independence and picked in-graph or Rust DSP. Read its `Outcome` first.

## Scope

1. Port `resolve_chunking`, `chunk_starts`, `crossfade_weights`, and `run_model` exactly. Keep batch size 1.
2. If task 01 chose model-core-only export, implement the STFT and iSTFT in Rust (`realfft`) to match `torch.stft` and `torch.istft` with `center=True`, reflect padding, and a periodic Hann window.
3. Put the model behind a trait so tests can use fakes.
4. Expose the whole chunked run through a file-level `RestoreBackend` trait (Stack §C). It takes one file's 44.1 kHz float channels, the settings, a progress callback, and a cancel flag. It returns the restored channels. The ONNX Runtime engine is its only implementation. Add no network code.
5. Run inference one channel at a time.
6. Report progress per chunk: chunk index, chunk count, and channel.
7. Check an `AtomicBool` between chunks. On cancel, stop and return a cancelled result.
8. Write the output to a temporary file. Rename it only when it is complete.
9. Load `apollo.onnx` from a path given by the caller. The app passes its resource path.
10. Add `tools/parity/make_golden.py`. It runs the **stock** `C:\Lab\Apollo\inference.py`:
   - in the conda env `look2hear_win`
   - with `PYTHONDONTWRITEBYTECODE=1`, so the clone stays read-only
   - with `--device cpu --checkpoint C:\Lab\Apollo\Apollo\pytorch_model.bin --chunk-seconds 6 --overlap-seconds 1 --chunk-pad-seconds 1`
   - on an 18 s input made from three copies of `input_wav.wav`

   Commit the golden output (about 6.4 MB) with its SHA-256.

## Out of scope

- Adaptive chunk size (task 05)
- GPU (task 08)
- Encoding (task 06)

## Test-first plan

1. Port the concepts of `tests/test_inference.py`:
   - an identity model round-trips the input exactly
   - a degraded-edges model with infinity markers leaves no infinity in the output when the pad is at least the degraded width
   - a short-output model returns an error
2. The Rust output on the 18 s input stays within max abs 1e-3 and RMSE 1e-4 of the golden.
3. An input no longer than one chunk takes the full-file path.
4. A cancel flag set during chunk 2 stops the run and leaves no output file.
5. A fake `RestoreBackend` drives one file through the trait and reports progress.

Run the tests. They fail because the engine does not exist.

## Acceptance criteria

- [x] The output matches the golden within the §12.1 tolerances. The numbers are in `Outcome`.
- [ ] A cancel takes effect within one chunk and leaves no partial file. The no-partial-file half moved to tasks 06 and 07 by orchestrator ruling on 2026-10-11. The cancel itself is proven by `cancel_during_chunk_two_stops_the_run` and `e2e_inference_cancel_returns_no_channels`.
- [x] Progress reports chunk n of m.
- [x] The new tests failed first for the expected reason.
- [x] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core inference
pnpm verify
```
