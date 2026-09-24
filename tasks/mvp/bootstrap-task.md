# Bootstrap handoff

**Status:** TODO
**Assigned to:** Owner

The owner supplies these inputs. Agents give current instructions through the human-input protocol in `tasks/README.md`. Agents do not perform these actions.

| # | Input | Needed by | Why |
| --- | --- | --- | --- |
| 1 | A MusicBrainz User-Agent contact: a project URL or an email address | 09 (a placeholder is allowed in development), 29 | MusicBrainz requires a User-Agent with enough information to contact the maintainer (`Melomae/<version> ( <contact> )`). It throttles anonymous agents. |
| 2 | A public GitHub repository and its remote | 00 (the CI runs), 27, 28 | The CI workflow runs the Linux build. The release assets need a home. |
| 3 | Confirm the app identifier. The proposal is `io.github.whitelicorice.melomae`. | 27 | Tauri keys the app data directory on the identifier. A change after the first release moves user data. |
| 4 | The Windows code-signing decision: unsigned, or a named certificate | 27 | An unsigned installer shows a SmartScreen warning. A certificate costs money and needs a secret store. |
| 5 | Acceptance of the YouTube Terms of Service and copyright risk for a public FOSS release | 29 | Downloading from YouTube can break its Terms of Service. youtube-dl received a DMCA takedown in 2020. The owner decides whether to publish. |
| 6 | A name-conflict check for "Melomae" | 29 | A conflicting trademark forces a rename after release. |

## Completion evidence

For each item, record the non-secret evidence here: the contact string, the repository URL, the confirmed identifier, the signing decision, the risk decision with its date, and the name-check result with its date. Never record a secret in this file.
