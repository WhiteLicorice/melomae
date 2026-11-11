# B-067 — Typecheck the Vite configuration files in the gate

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 00

## Impact

The gate does not typecheck `vite.config.ts` or `vitest.config.ts`. A type error in either file passes `pnpm verify`.

## Evidence

The ponytail audit of 2026-11-11 read the code at `master` `23f0bcf`. It built and ran nothing. The orchestrator read each cited line again on the same day.

- `package.json:12`: the script is `tsc --noEmit`.
- `tsconfig.json:23-24`: the project includes `src` only, and refers to `tsconfig.node.json`. `tsc --noEmit` with no `--build` does not check a referenced project.
- `tsconfig.node.json:9` includes `vite.config.ts` only. No project includes `vitest.config.ts`.
- `vite.config.ts:3`: an `@ts-expect-error` comment covers the missing `@types/node`. Nothing checks it.
- Nobody ran the gate with a deliberate error.

## Why this does not block MVP

Both files are short and change rarely. A bad configuration fails the build or the tests at run time.

## Expected behavior

The typecheck script covers both configuration files. `@types/node` replaces the `@ts-expect-error` comment.

## Promotion condition

Promote this task when task 19 changes either configuration file.

## Acceptance criteria

- [ ] A deliberate type error in `vite.config.ts` fails `pnpm typecheck`.
- [ ] A deliberate type error in `vitest.config.ts` fails `pnpm typecheck`.
- [ ] `vite.config.ts` has no `@ts-expect-error` comment.
- [ ] A test fails before the change and passes after it.
