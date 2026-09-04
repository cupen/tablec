## Why

The webui left rail lists only the direct children of the input directory (`read_dir`, non-recursive) as a two-line-per-file flat list, while `build`/`check` operate on the glob-based, recursion-capable `find_excel_files` set. Two problems follow: spreadsheets in subdirectories are invisible in the UI even though build compiles them, and the flat two-line rows waste vertical space as table projects grow. A compact, directory-aware file rail is needed for multi-level table projects.

## What Changes

- **BREAKING** `GET /api/files` now returns exactly the set `build`/`check` operate on: it recursively scans the resolved input directory through the same include/exclude glob semantics as `find_excel_files` (default pattern `*.xlsx`). Consequence: with default configuration, `.xlsb`/`.ods` files no longer appear (the build set never accepted them); a config with explicit `include` patterns restores them to the degree the patterns match. Each entry additionally carries its path relative to the input directory so the UI can derive hierarchy.
- The left rail (`<file-list>`) is redesigned for compactness: single-line rows (fixed ~24px row height), the size/date meta moves into a tooltip, and directory tree rendering with expand/collapse, indentation guides, and file-count badges per directory.
- The rail surfaces table health: per-file error/warning counts derived from the check pipeline (grouping check diagnostics by source file), error badges on file rows, aggregate badges on directory rows, and a total in the rail header. The counts refresh with the listing (initial load, reload, live file-change refresh).
- The rail gains a sort control: sort factor (file name, modification time, error count) with direction toggling; files order within their directories and directories order by the matching aggregate.
- Directory expand/collapse state survives re-fetches (live-reload refreshes, filter switches); "Modified only" auto-expands the ancestors of matching files.
- Empty-state, count, and git status/numstat presentation are preserved; only the layout and listing semantics change.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `webui`: The "File and preview endpoints" requirement changes — `/api/files` lists recursively with build-set semantics (include/exclude globs, default `*.xlsx`) and returns each entry's path relative to the input directory; a new requirement covers the compact tree view of the file rail (hierarchy, expand/collapse, persistence across refreshes, filter interaction).

## Impact

- **Code**: `tablec-webui/src/handlers.rs` (recursive scan + relative paths in `FileEntry`, honoring config include/exclude), `tablec-webui/webui/src/components/file-list.ts` (tree rendering, compact single-line rows, error badges, sort control), `tablec-webui/webui/src/store.ts` (expanded-dirs state, tree derivation, per-file diagnostic counts, sort state), `tablec-webui/webui/src/api.ts` (check invocation on listing refresh).
- **APIs**: `GET /api/files` response gains a relative-path field; listing scope changes from "direct children" to "build set" (**BREAKING** for clients relying on the old flat/direct-children behavior; the webui's own frontend is the only known consumer). `/api/check` is consumed as-is — no endpoint changes.
- **Behavior**: Files under subdirectories now appear and are selectable/previewable; default-config listings shrink to `*.xlsx` matches; the rail shows live per-file error status after each listing refresh (one check run per refresh, front-end driven).
- **Non-goals**: Changing `find_excel_files`'s extension whitelist (`.xlsb`/`.ods` acceptance in core) — recorded as a known follow-up, not part of this change. No server-side check-result caching or watcher-triggered validation (a possible follow-up if per-save rechecks feel slow). No changes to preview/build/check endpoints, layout elsewhere, or the live-reload mechanism.
