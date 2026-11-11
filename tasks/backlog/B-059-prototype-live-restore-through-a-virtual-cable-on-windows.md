# B-059 — Prototype live restore through a virtual cable on Windows

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** B-058

## Impact

The speed numbers of B-058 do not prove that live restore sounds right. A prototype that the owner can listen to for a full session shows the real delay, the dropouts, and the sample-rate handling.

## Evidence

From a web search in a research handoff of 2026-10-30. Nobody verified these points on a machine:

- Unverified: system-wide audio tools on Windows use a virtual audio device. A signed kernel driver takes the audio, and a user-mode app processes it and plays it to the real device.
- Unverified: VB-Cable is donationware, and a bundle needs a separate license from its vendor. The terms are not public. Read https://vb-audio.com/Cable/ again before any decision.
- Inferred: the virtual device runs at 48 kHz on most machines, and the model needs 44.1 kHz. The prototype then converts the rate twice.

## Why this does not block MVP

The MVP plays audio only in the compare player.

## Expected behavior

1. The owner installs a virtual audio cable by hand and routes the player to its input. Melomae does not install it and does not bundle it.
2. A prototype command captures the cable output, converts it to 44.1 kHz, restores it with the stateful model of B-057, converts it to the device rate, and plays it to the real device.
3. The prototype logs each dropout and the buffer level.
4. Measure the start delay with a click track: record the time between the input click and the output click.

The prototype is a command-line tool. It adds no UI.

## Promotion condition

Promote this record when B-058 records GO.

## Acceptance criteria

- [ ] The prototype runs for 30 minutes with no logged dropout on the dev machine.
- [ ] A record holds the measured start delay, the block size, and the safety buffer.
- [ ] The same record holds the owner's listening check.
- [ ] No installer, driver, or third-party binary enters the repository.
