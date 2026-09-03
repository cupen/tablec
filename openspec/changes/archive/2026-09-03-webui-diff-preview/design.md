## Context

The webui currently exposes `/api/files` (name/path/size/mtime), `/api/sheets`, `/api/parsed_preview` (schema + typed rows + diagnostics), `/api/build`, `/api/check`, and `/api/validate` (501). The frontend (`tablec-webui/webui/`) is Lit + TypeScript: `file-list.ts` renders the left rail with a count badge, `file-preview.ts` renders the parsed grid, and `store.ts` holds the `files`/`parsed` state. The user wants to preview what changed in the spreadsheets, with the baseline being git: compare working-tree content against the current branch HEAD, filter the left menu to changed files, and color cell changes (deleted=red, added=green, modified=yellow).

Non-negotiable from the user: the baseline is git's committed-vs-uncommitted relationship (like `git diff`), keyed off the current branch, with future cross-branch comparison left as an extension.

## Goals / Non-Goals

**Goals:**

- Per-cell diff status in the parsed preview for the previewed sheet, derived from git HEAD.
- Per-file git status + change counts in the file list; a Modified-only filter toggle with a count badge that matches the active filter.
- Keep the whole pipeline additive — existing endpoints, payloads, and UI behavior keep working.
- Robust fallbacks: not a git repo, no HEAD, missing `git` binary, unreadable blob — all degrade to clean/no-diff with a warning, never a hard failure.

**Non-Goals:**

- Cross-branch diffing (e.g. comparing against another branch's HEAD) — the design keys the baseline to the current branch and records the seam for later; no branch-name API now.
- Diffing across time snapshots or build caches — the baseline is git HEAD only.
- Editing or committing spreadsheets from the webui; no index/stage mutations from the server.
- Styling the diff in the raw (`/api/preview`) view — cell colors apply to the parsed preview (the default view).

## Decisions

### D1: Use the `git` binary via `std::process::Command`, not libgit2
The backend shells out to `git` (`rev-parse --show-toplevel`, `status --porcelain`, `diff --numstat`, `show HEAD:<path>`, `ls-files`) instead of adding a `git2` crate.

Rationale: no new dependency, git is already a hard requirement for this feature (the baseline is a git repo), and the project convention favors minimal deps. Alternatives considered: `git2` adds a vendored libgit2 build and a new Cargo dep for little gain over plumbing commands. Trade-off: we need `git` installed — the spec already covers the "missing binary" fallback (clean + warning); handler tests skip when `git` is missing.

### D2: Baseline = current branch HEAD, resolved once per request tree
Each diff request resolves `git rev-parse --show-toplevel` from the working directory (cached per dir), and compares against `HEAD`. Branch-name plumbing (e.g. `git diff <branch>`) is left as the future seam for cross-branch comparison.

Rationale: matches "committed vs uncommitted" semantics with no extra state; the seam is a parameter to the same plumbing.

### D3: Per-file status from `git status --porcelain -- <files>`, counts from `git diff --numstat`
One `status --porcelain` sweep over the input directory yields each file's `XY` code → mapped to `modified`/`added`/`untracked`/`deleted`/`clean`. `--numstat` gives insertions/deletions for `modified` files.

Rationale: a single porcelain sweep is cheap and authoritative; numstat is only for files that actually changed. Alternative (parsing `diff --stat`) was rejected — numstat is machine-stable.

### D4: Per-cell diff by round-tripping the HEAD blob through the same parser
To diff cell-by-cell, we need the HEAD version of the sheet. Strategy: read the HEAD blob for the file path (`git show HEAD:<path>`), and — because the blob is the same bytes the working tree was also read from — run the same calamine parsing on a temp file written from the blob, then align the two parsed sheets by primary key (the `@unique` column; fall back to row index when no unique column exists) and compare cell by cell.

Rationale: avoids reimplementing Excel/CSV parsing, reuses `read_excel_with`, and automatically inherits value-normalization (so numeric equalities like `1` vs `1.0` compare equal). Alternative (custom binary/XML diff of `.xlsx` zip parts) rejected: it would diverge from what tablec actually reads and produce spurious diffs. Temp-file approach: calamine opens by path; a `NamedTempFile` in a `tempdir()` is used per request and cleaned up after.

### D5: Row alignment
When the table has a unique column (a field with `@unique`), rows in the two versions are paired by that key's value; this makes sorting a sheet still show "modified" cells rather than a wall of add/delete. Without a unique column, rows are paired by index. Deleted rows (HEAD-only keys) produce `deleted` cells; new rows (working-tree-only keys) produce `added` cells; equal keys are compared cell-wise.

Rationale: key alignment is what makes the diff readable for hand-edited sheets; index fallback keeps it simple when no key exists.

### D6: Diff statuses ride inside existing payloads
Per-cell `diff` is added to each parsed cell object and a `status`/per-file counts are added to each `/api/files` entry; new query params (`filter=modified`) and a new return field are added rather than new endpoints.

Rationale: the UI already consumes these payloads; additive fields keep the frontend contract stable and the change backward-compatible.

### D7: Diff is derived per previewed sheet on the parsed view
Per-cell coloring applies in the parsed preview (the default view). Raw view and header/schema rows are not colored; `#`-named sheets are skipped by existing parsing and thus not diffed.

### D8: Frontend filter + status badges in `file-list.ts`
Add a toggle ("All files" / "Modified only") that switches the rendered list; the count badge shows `files.length` in All mode and the changed count in Modified mode. Each row gains a colored status dot/count (red deletions, green additions) from the per-file status.

## Risks / Trade-offs

- [Diff of binary .xlsx via `git show` requires a temp file and re-parse per request] → Requests are user-driven previews, not hot paths; `tempdir()` + cleanup keeps it cheap. Guard tests skip when `git` is missing.
- [Row pairing by unique key may mispair when keys collide or change] → Unique-key rows that don't match any HEAD key are labeled `added` (and their old counterpart `deleted`), never silently merged; index fallback when no unique column.
- [Git plumbing output parsing can be brittle across git versions] → Use stable porcelain/plumbing (`--porcelain`, `--numstat`, `--no-color`); tests assert the documented statuses.
- [Big binaries make `git show` heavy] → Diff only runs for the previewed sheet/file; the file-list status sweep uses cheap porcelain pathspecs.
- [Not a git repo / no HEAD / missing git] → Documented clean-with-warning fallback; UI shows all files with no diff colors and a subtle note.

## Migration Plan

Additive: no payload or behavior changes for clients that ignore the new fields. The `webui` spec's file-listing and parsed-preview requirements are updated in place; existing tests continue to pass (new fields are optional in assertions). No data migration. Rollback: revert the backend fields and frontend filter toggle — old shapes are preserved by additive fields.

## Open Questions

- Exact UX for the "no git baseline" note (banner vs inline) — deferrable; the spec only requires a non-fatal fallback, the designs can choose the visual.
- Whether `untracked` spreadsheets should be colored as fully-added or shown with a distinct badge — deferrable to implementation; both satisfy the spec (`untracked` is a change status; per-cell statuses for untracked files read as `added`).