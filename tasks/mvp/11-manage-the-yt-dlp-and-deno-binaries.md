# 11 — Manage the yt-dlp and Deno binaries

**Status:** TODO
**Phase:** 2 — Metadata and acquisition
**Depends on:** 02, 30
**SRS:** §3, §5
**Stack:** §E
**Assigned to:** Agent
**Started:** —
**Outcome:** —

## Goal

Download, verify, update, and locate the yt-dlp and Deno binaries without user action.

## Evidence

Task 02 recorded the release asset names, sizes, checksum sources, and licenses. Read its evidence file first.

## Scope

1. Download the official release assets for the current OS into the app data directory.
2. Before first use, verify each binary against its published SHA-256.
3. Check for updates on a schedule and on demand.
4. Replace a binary atomically. A failed update keeps the old binary.
5. Report the installed versions to the caller.
6. When the machine is offline, return an error that names the missing tool.
7. Use HTTPS only. Never run a binary that failed verification.
8. **Source interface (core requirement).** Install the tools that each registered source declares in `required_tools()` (task 30). Do not hardcode a YouTube-only list. In the MVP, the YouTube Music source declares yt-dlp and Deno. A later Bandcamp or SoundCloud source can declare the same yt-dlp binary, and the manager installs it once.

## Out of scope

- Search and download logic (tasks 12 and 13)
- The first-run UI (task 25)

## Test-first plan

Use a mock HTTP server:

1. A checksum mismatch is rejected, and the binary is not kept.
2. An interrupted download leaves the old binary intact.
3. An update replaces the binary and reports the new version.

Run the tests. They fail because the manager does not exist.

## Acceptance criteria

- [ ] A real first-run download on Windows passes. `Outcome` records the versions.
- [ ] The new tests failed first for the expected reason.
- [ ] `pnpm verify` passes.

## Verify

```powershell
cargo test -p melomae-core tools_manager
pnpm verify
```
