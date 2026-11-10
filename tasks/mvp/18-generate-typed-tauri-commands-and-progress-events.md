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
2. Expose commands for adding inputs, triage, queue, cancel, retry, job list, settings, history, and playback control.
3. Stream progress over Tauri Channels.
4. Keep heavy work off the main thread.
5. Add `pnpm bindings:check` to the gate. It fails when the committed bindings differ from the generated ones.
6. Give every progress event one envelope: the job identifier, an attempt identifier, a sequence number that only increases, the stage, and the payload. The frontend drops an event from an old attempt and an event with an old sequence number.
7. Give every command error one type: a code, a message for the user, and a technical detail. No screen shows a raw Rust error string. Task 21 shows the user message first.

## Out of scope

- Screens (tasks 19 to 25)

## Test-first plan

1. The drift check fails when a Rust type changes and the bindings do not.
2. A command test through Tauri's mock runtime calls one command and receives one Channel event.
3. A frontend test sends events out of order, and one event from a cancelled attempt. The state holds the newest event of the current attempt.
4. A command test forces an error and receives the code, the user message, and the detail.

Run the tests. They fail because the bridge does not exist.

## Acceptance criteria

- [ ] The bindings are generated, committed, and checked by the gate.
- [ ] A late event from an old attempt does not change the state.
- [ ] Every command error carries a code, a user message, and a detail.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
pnpm bindings:check
pnpm verify
```
