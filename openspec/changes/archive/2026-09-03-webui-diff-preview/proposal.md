## Why

The webui today can preview, build, and check spreadsheets, but there is no way to see what changed in the data — the user has to eyeball cells against whatever they remember or run external `git diff` on the generated JSON. For a table compiler living inside a git repository, the natural baseline is git itself: compare each spreadsheet's working-tree content against the committed version on the current branch (HEAD), highlight per-cell changes, and let the left file menu filter to only the files that changed.

## What Changes

- New backend diff capability: resolve the git repo containing the working directory, diff each spreadsheet against the same path at the current branch's HEAD.
- Cell-level diff annotation for the parsed preview: each parsed cell gets a `diff` status — `added` (green), `deleted` (red), `modified` (yellow), or `unchanged`.
- Git status integration for the file list: each file gets an overall change status (`modified`, `added`, `untracked`, `deleted`, or clean), and the header shows red/green per-file change counts (additions/deletions).
- Left menu filter toggle: "All files" / "Modified only" — filtering shows only files with changes; count badge reflects the active filter.
- Baseline is the current branch's HEAD commit; the design keeps branch-to-branch comparison (e.g. `@ref` git refs) as a future extension and records it as such.
- **BREAKING** (none for existing users): diff is additive; existing endpoints keep their shapes. New/adjacent query params are additive.

## Capabilities

### New Capabilities

- `git-diff`: Resolve the containing git repository, compare spreadsheet working-tree content against the current branch HEAD, and produce per-cell diff statuses plus per-file git status and change counts. Includes requirements for clean/unmodified handling, untracked files, path containment safety, and non-git/corrupt-repo fallbacks.

### Modified Capabilities

- `webui`: Extend the parsed preview response with a per-cell diff status, extend the files listing with per-file change status/counts and a modified-only filter, and require the diff to be applied by path against the git baseline rather than any server-side build cache.

## Impact

- **Code**: `tablec-webui/` — a new diff module (git plumbing commands), new/extended request handlers (`/api/files`, `/api/parsed_preview`) and their tests, `file-list.ts` (filter toggle + change badges), `file-preview.ts` (cell diff colors).
- **New dependency**: none at the Rust level — diff delegates to the `git` binary via `Command`; no new crate needed (no git2/libgit2). Frontend: no new package.
- **Docs**: `docs/design.md` / webui README notes updated; the existing `webui` capability spec is extended.
- **CI / test surface**: handler-level tests now spawn the `git` binary against temp repos (guarded: skipped when `git` is unavailable) and verify cell statuses, per-file counts, untracked handling, and the filter.
- **Note**: requires `git` installed and a git repo with a HEAD; outside a repo (or with no commits) the diff falls back to a documented no-diff state rather than failing.