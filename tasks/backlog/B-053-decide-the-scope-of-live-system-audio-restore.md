# B-053 — Decide the scope of live system-audio restore

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** —
**Assigned to:** Human

## Impact

The owner wants Melomae to restore the audio that the machine plays, live, with a fixed start delay. The use case is listening to a streamed source. The owner accepts the delay, and accepts that video loses lip sync. The SRS does not admit this mode today, so no later record in this group can start.

## Evidence

Confirmed in the repository on 2026-11-11 at `master` `23f0bcf`:

- The SRS intro says that Melomae "restores a music collection that the user already owns" (`docs/melomae-srs.md:5`). SRS §1.1 defines the product as folders of files in and restored files out (`docs/melomae-srs.md:11`).
- SRS §1.5 says that the README and the app do not name, link to, or suggest any streaming service (`docs/melomae-srs.md:15`). `docs/evidence/2026-10-10-pivot.md` records why.
- SRS §1.3 and §1.4 forbid quality claims (`docs/melomae-srs.md:13-14`).
- SRS §12.1 defines parity against stock Apollo "on the same input and the same chunk settings" (`docs/melomae-srs.md:154`). A live mode has no 20 s chunks.
- B-012 owns hosted restore. A hosted live mode would send audio off the machine.

## Why this does not block MVP

The MVP restores a library of files. Live restore is a second product surface. It starts after the library restore works.

## Expected behavior

The owner records these decisions in the SRS, or rejects the mode:

1. Does the SRS admit a live mode? How do the intro and §1.1 word it?
2. How does §1.5 word the use case? The records in this group say "a streamed source" and name no service.
3. Is the parity reference for a live mode one whole-file run, in place of the chunked output of §12.1?
4. Is live restore local only? This record assumes yes, and leaves hosted restore to B-012.
5. Which caveat does the live mode show? The output goes to the sound device as PCM, so no bitrate or format claim applies.

## Promotion condition

Promote this record first, before B-054 to B-060. Promote it when the library restore of the MVP works on the owner's machine.

## Acceptance criteria

- [ ] The SRS holds the owner's decision on each of the five questions, with the date.
- [ ] If the owner rejects the mode, B-054 to B-060 are CANCELLED with the reason.
