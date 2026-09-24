# 18 — Generate typed Tauri commands and progress events

**Status:** TODO
**Phase:** 4 — Desktop UI
**Depends on:** 17
**SRS:** §6
**Stack:** §A, §B
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Connect the React UI to `melomae-core` through typed commands and events, with TypeScript types generated from Rust.

## Scope

1. Generate the TypeScript bindings from the Rust types. The candidates are tauri-specta and ts-rs. Confirm Tauri 2 support for each at execution time. Record the pick and the reason.
2. Expose commands for search, browse, queue, cancel, retry, job list, review decisions, settings, library scan, and playback control.
3. Stream progress over Tauri Channels.
4. Keep heavy work off the main thread.
5. Add `pnpm bindings:check` to the gate. It fails when the committed bindings differ from the generated ones.

## Out of scope

- Screens (tasks 19 to 25)

## Test-first plan

1. The drift check fails when a Rust type changes and the bindings do not.
2. A command test through Tauri's mock runtime calls one command and receives one Channel event.

Run the tests. They fail because the bridge does not exist.

## Acceptance criteria

- [ ] The bindings are generated, committed, and checked by the gate.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm bindings:check
pnpm verify
```
