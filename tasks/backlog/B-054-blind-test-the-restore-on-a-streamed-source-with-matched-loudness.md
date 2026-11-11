# B-054 — Blind-test the restore on a streamed source with matched loudness

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 07

## Impact

The owner hears a "fuller" sound when Apollo restores a high-bitrate streamed source. Nobody has tested that impression blind. If the listener cannot tell the versions apart, live restore has no audible gain, and B-055 to B-060 are wasted work.

## Evidence

- The owner's report is an impression. No record holds a measurement.
- Inferred, not confirmed: such a source already reaches about 20 kHz. The gain is then not bandwidth extension. A loudness difference between the two versions can also cause a "fuller" impression.
- The Apollo paper (arXiv 2409.08514v1) reports training on MP3 at 24 to 128 kbps only. This claim comes from a research handoff of 2026-10-30. Nobody checked it again on 2026-11-11.
- The cutoff triage (`crates/melomae-core/src/triage/cutoff.rs`) would probably give such a source a low-benefit verdict. This is inferred. Do not use the cutoff as an automatic bypass for a live mode until this test has a result.
- Task 07 (DONE) gives the command-line restore. Task 32 (the in-app ABX mode) is TODO.

## Why this does not block MVP

The MVP restores files that lost high frequencies. This test covers a source that lost little.

## Expected behavior

1. The owner captures a few short clips of a high-bitrate streamed source. The owner first decides whether a private capture is acceptable under the terms of that service. Do not name the service in any file.
2. Restore each clip with the command-line entry of task 07.
3. Measure the integrated loudness of each pair. Record the difference in dB. Match the loudness before the test.
4. Run an ABX test with a fixed trial count that the owner sets before the first trial. An external ABX tool is acceptable while task 32 is not done.
5. Record the score and the one-sided binomial p-value for each clip.

Keep the clips out of the repository.

## Promotion condition

Promote this record when the owner wants evidence for the live mode. It does not wait for B-053.

## Acceptance criteria

- [ ] A record in `docs/evidence` holds the loudness difference, the trial count, the score, and the p-value for each clip.
- [ ] The record names no streaming service.
- [ ] The owner records GO or NO-GO for the live mode from the result.
