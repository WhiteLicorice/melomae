# 04 — Port chunked inference to ONNX Runtime

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 01, 03
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

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
4. Run inference one channel at a time.
5. Report progress per chunk: chunk index, chunk count, and channel.
6. Check an `AtomicBool` between chunks. On cancel, stop and return a cancelled result.
7. Write the output to a temporary file. Rename it only when it is complete.
8. Load `apollo.onnx` from a path given by the caller. The app passes its resource path.
9. Add `tools/parity/make_golden.py`. It runs the **stock** `C:\Lab\Apollo\inference.py`:
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

Run the tests. They fail because the engine does not exist.

## Acceptance criteria

- [ ] The output matches the golden within the §12.1 tolerances. The numbers are in `Outcome`.
- [ ] A cancel takes effect within one chunk and leaves no partial file.
- [ ] Progress reports chunk n of m.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core inference
pnpm verify
```
