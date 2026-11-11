# B-068 — Put a timeout on the pinned downloads

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 00

## Impact

`pnpm verify` calls three download scripts. A network read that stops leaves a local gate run with no end and no message.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `tools/fetch_ffmpeg.py:92`, `tools/fetch_onnxruntime.py:107`, and `tools/cargo_deny.py:98` call `urllib.request.urlopen(url)` with no timeout.
- Each script downloads only when its pinned file is missing.

## Why this does not block MVP

The download runs once for each machine. The developer can stop a run that does not continue.

## Expected behavior

Each download has a timeout. A failure names the asset and the URL. Do this in the one shared module if R-003 is done first.

## Promotion condition

Promote this task with R-003, or when a gate run stops on a download.

## Acceptance criteria

- [ ] A read that stops fails the script inside the timeout and names the asset.
- [ ] The offline tests of the three scripts pass.
- [ ] A test fails before the change and passes after it.
