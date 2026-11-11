# B-060 — Choose the shipped Windows capture path

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** B-055, B-059
**Assigned to:** Human

## Impact

The prototype of B-059 needs a cable that the user installs by hand. A shipped live mode needs a capture path that a non-technical user can turn on. The choice sets the license cost, the signing work, and the support risk.

## Evidence

From a web search in a research handoff of 2026-10-30. Nobody verified these points. Read each source again before the decision:

- Unverified: audio processing objects (APOs) run inside the Windows audio service. They are a poor fit for a GPU model with a lookahead of about 0.55 s. The fit is inferred.
- Unverified: an open-source audio enhancer under AGPL-3.0 keeps its driver out of its repository (https://github.com/fxsound2/fxsound-app).
- Unverified: a custom virtual audio driver needs Microsoft signing through the Hardware Dev Center. Developers report error 52 on some machines.
- B-055 records whether process loopback captures a muted app.

## Why this does not block MVP

The MVP captures no audio.

## Expected behavior

The owner chooses one path and records the cost and the risk of each:

1. Process loopback with the player muted. This path needs no driver. It is open only if B-055 records GO.
2. A license to bundle a third-party virtual cable.
3. A custom signed virtual audio driver.
4. No bundle. The user installs a cable by hand, as in B-059.

The record also states how each path fits GPL-3.0-only (SRS §11.1).

## Promotion condition

Promote this record when B-055 has a result and B-059 is DONE.

## Acceptance criteria

- [ ] A record holds the cost, the license terms, and the risk of each path, with a dated source for each claim.
- [ ] The owner records the chosen path.
- [ ] A numbered task exists for the chosen path, or the live mode stays a manual setup.

A causal retrain is a separate, later option. It applies only if the owner rejects the measured lookahead. It needs new weights under CC BY-SA 4.0, a dataset license check, and a quality comparison.
