# 61 — Compare the four-minute peak with its total

**Status:** TODO
**Phase:** 1 — Restoration engine
**Depends on:** 46
**SRS:** §6, §12
**Stack:** §C
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Make the ignored four-minute restore measurement compare like numbers.

## Evidence

- The task 46 review (non-blocking 5): `crates/melomae-cli/tests/restore_e2e.rs:502` parses `predicted inference peak`, the ONNX chunk only. The test compares it with the whole-process peak, which also holds the file reserve.
- For the 240 s stereo input the reserve is 254016000 bytes, so the printed error is about 5.6% high. The fixed line still prints `total`.

## Scope

1. Parse `total ` from the line, or add the reserve before the division.

## Acceptance criteria

- [ ] The printed error compares the whole-process peak with the predicted total.
- [ ] `cargo test -p melomae-cli -- --ignored` runs the four-minute test and prints the corrected error.
