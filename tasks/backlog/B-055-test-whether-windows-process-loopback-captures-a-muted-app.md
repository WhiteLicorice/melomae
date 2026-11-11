# B-055 — Test whether Windows process loopback captures a muted app

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** —

## Impact

A live mode must take the audio of a player away from the speakers, restore it, and play the result. If Windows can capture an app that is muted in the volume mixer, Melomae needs no audio driver. If it cannot, the user hears the original and the restored audio together, and the product needs a virtual audio device.

## Evidence

Every point here comes from a web search in a research handoff of 2026-10-30. Nobody ran a Windows audio API. Treat each point as a hypothesis and read the current Microsoft documentation first.

- Unverified: Windows process loopback (`PROCESS_LOOPBACK_MODE`) captures the audio of one process tree. Microsoft's ApplicationLoopback sample shows it.
- Unverified: the original sound still plays during the capture.
- Unknown: whether the capture stays non-silent when the app is muted in the volume mixer.

Sources to read again:

- https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/
- https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ne-audioclientactivationparams-process_loopback_mode

## Why this does not block MVP

The MVP reads files. It captures no audio.

## Expected behavior

1. Build Microsoft's ApplicationLoopback sample.
2. Run it in include mode against an app that plays audio.
3. Mute that app in the volume mixer during the capture.
4. Check whether the captured file is silent. Measure its peak level. Do not judge by ear only.
5. Repeat with the app at a low, non-zero volume. Record whether the captured level follows the mixer volume.

## Promotion condition

Promote this record when the owner wants evidence for the live mode. It does not wait for B-053.

## Acceptance criteria

- [ ] A record in `docs/evidence` holds the Windows build, the sample commit, the steps, and the measured peak level of each capture.
- [ ] The record states GO (the muted capture is not silent) or NO-GO.
- [ ] The record states whether the captured level follows the mixer volume.
