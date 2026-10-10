# B-014 — Make the gate independent of NODE_ENV

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 00

## Impact

`pnpm verify` fails at Vitest when the shell sets `NODE_ENV=production`. React then loads its production build, which has no `act`, and `src/App.test.tsx` fails with `TypeError: React.act is not a function`. The CommandCode CLI sets this variable in its shells. The orchestrator reproduced the failure on 2026-10-10 with `NODE_ENV=production npx vitest run`.

## Why this does not block MVP

The gate passes in a shell without `NODE_ENV`, and the GitHub workflow does not set it. Agents can remove the variable before the gate.

## Expected behavior

The Vitest run sets the test mode itself, for example with `mode` or an `env` entry in `vitest.config.ts`, so that an inherited `NODE_ENV` cannot change the React build that the tests load.

## Promotion condition

Promote this task when another agent or developer hits the failure, or with the next change to `vitest.config.ts`.

## Acceptance criteria

- [ ] `NODE_ENV=production pnpm verify` passes.
- [ ] A test or a recorded run shows the red result before the change.
