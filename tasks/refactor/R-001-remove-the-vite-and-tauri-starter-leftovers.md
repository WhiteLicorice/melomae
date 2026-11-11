# R-001 — Remove the Vite and Tauri starter leftovers

**Status:** TODO
**Severity:** Refactor
**Depends on:** —

## Impact

The app still carries parts of the Vite and Tauri starter. They do nothing for Melomae, and they hide what the app uses.

## Evidence

- `index.html:5`: the favicon is `/vite.svg`.
- `public/vite.svg`, `public/tauri.svg`, and `src/assets/react.svg`: no file in `src` or `index.html` uses the last two. A search for the three names on 2026-11-11 found only the favicon line.
- `src/App.css:1-7`, `:33-47`, and `:94-96`: rules for `.logo`, `.row`, and `#greet-input`. `src/App.tsx` renders none of them.
- `src/App.css:36`: `will-change: filter` on the unused `.logo`.
- `src-tauri/src/lib.rs:2-5` and `:11`: the `greet` command. No frontend code calls it.

- The ponytail audit of 2026-11-11 found the same leftovers. It also proposed to remove `tauri-plugin-opener` (`src-tauri/Cargo.toml:23`, `src-tauri/src/lib.rs:10`, `src-tauri/capabilities/default.json:8`, `package.json:19`). No code calls the plugin today. Keep it: task 21 needs an "open file location" action. If task 21 opens the folder in another way, remove the plugin and its capability then.

## Why this is a refactor

No user sees a defect. The window shows the title and one sentence. The change removes dead files, dead rules, and one dead command.

## Expected behavior

- No file refers to `vite.svg`, `tauri.svg`, or `react.svg`.
- No rule targets `.logo`, `.row`, or `#greet-input`.
- The `greet` command does not exist.
- The favicon is the app icon.

## Promotion condition

Promote this task when task 18 or task 19 starts. Each one edits the same files.

## Smallest correct fix boundary

- Delete the three SVG files, the dead rules, and the `greet` command with its handler entry.
- Point the favicon at an icon from `src-tauri/icons`.
- Do not change the remaining rules. Task 19 replaces the stylesheet.

## Test-first plan

- The gate is the characterization test. Record the `pnpm verify` counts before the change.

## Acceptance criteria

- [ ] A search for the three file names and the three selectors finds nothing.
- [ ] The app builds and shows the same window.
- [ ] `pnpm verify` gives the same counts before and after the change.
