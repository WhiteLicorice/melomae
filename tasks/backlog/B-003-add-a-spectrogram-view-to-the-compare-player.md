# B-003 — Add a spectrogram view to the compare player

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 23

## Impact

Apollo works mostly above the frequency cutoff of lossy codecs. A spectrogram shows that change. Listening alone does not always show it.

## Why this does not block MVP

SRS §10 needs an audible A/B switch. It does not need a picture.

## Expected behavior

The compare player shows the source and the restored spectrograms side by side on one frequency axis. It marks the playback position.

## Promotion condition

Promote this task when users ask how to see what restoration changed.

## Acceptance criteria

- [ ] Both spectrograms render for a 4-minute track in 2 s or less.
- [ ] The marked position stays in sync with the A/B player.
