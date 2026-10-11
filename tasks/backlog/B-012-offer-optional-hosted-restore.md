# B-012 — Offer optional hosted restore

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 17

## Impact

On CPU, a 4-minute song takes about 12.5 min on the dev machine (Stack §K). A 1,000-track collection would take days. A hosted GPU backend could finish it in hours.

## Why this does not block MVP

The MVP is free and local (SRS §2.4). The `RestoreBackend` trait (task 04) leaves room for a remote backend without a pipeline change.

## Expected behavior

The user chooses local or hosted restore for a batch. The app shows the cost before upload. The hosted backend is closed source. The client stays GPL-3.0-only. Licensing notes are in `docs/evidence/2026-10-10-pivot.md`.

SRS §11.10 records the license terms for hosted use of the model.

## Promotion condition

Promote this task when the local app has active users and a paid pilot shows that users will buy hosted processing at a price above its full cost. Competitor price anchors are in the evidence record.

## Acceptance criteria

- [ ] A benchmark records the cost per audio minute on the chosen GPU host.
- [ ] The owner records the decision on MetaBrainz terms before launch, if B-011 has shipped.
