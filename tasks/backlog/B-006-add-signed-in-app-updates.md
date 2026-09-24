# B-006 — Add signed in-app updates

**Status:** TODO
**Severity:** Future-deferred non-MVP
**Depends on:** 27, 28

## Impact

Without in-app updates, users must download each new version by hand. yt-dlp updates itself through task 11. The app does not.

## Why this does not block MVP

The first release can ship without updates. Updates need a signing key and a release host, and bootstrap item 2 does not exist yet.

## Expected behavior

The app checks a signed release manifest. It downloads the update, verifies the signature, and installs it after the user confirms. It uses the Tauri updater plugin. Confirm the plugin's current state at execution time.

## Promotion condition

Promote this task after the first public release.

## Acceptance criteria

- [ ] An update from version N to N+1 installs on Windows and Linux.
- [ ] A tampered update is rejected.
