## Context

`api_files` (`tablec-webui/src/handlers.rs`) lists direct children of the resolved input directory with `std::fs::read_dir`, filtering by extension in place, and returns a flat `Vec<FileEntry>` (`name`, absolute `path`, size, modified, git `status`, numstat). `build`/`check` resolve their input set through `tablec_core::core::config::find_excel_files` (glob include patterns, default `*.xlsx`, exclude patterns, extension whitelist `xlsx`/`xls`), which recurses when patterns contain `**/`. The frontend rail (`tablec-webui/webui/src/components/file-list.ts`) renders a flat `<ul>` with two-line rows (name + ext badge + status dot; size · date + numstat) inside a 280px column. The modified-only filter is applied server-side (`filter=modified`) and re-fetched via `refreshState()`; live reload re-runs the same path.

## Goals / Non-Goals

**Goals:**
- `/api/files` and the build/check input set coincide by construction, not by convention.
- The rail renders multi-level directories compactly (single-line rows) without losing status dots, numstat, empty states, or selection/preview behavior.
- Expansion state survives re-fetches (reload button, live refresh, filter toggle round-trips).

**Non-Goals:**
- Changing `find_excel_files` itself (extension whitelist, glob dialect) — any core fix is a separate change.
- Resizable/collapsible rails, drag-drop, or multi-select.
- Pagination or virtualization of very large listings.

## Decisions

- **Reuse `find_excel_files` in `api_files`** with the config's `data.include`/`data.exclude` (already available via `resolve_config`) instead of reimplementing a recursive walk. The list equals the build set because it *is* the build set; drift becomes impossible. Alternative (recursive `read_dir` + own glob matching) was rejected: two glob implementations guarantee eventual divergence.
- **Flat response + `rel_path` field, tree derived client-side.** `FileEntry` gains `rel_path` (path relative to the input directory, separators normalized to `/`). A nested-tree JSON was rejected: the backend merge of git statuses/meta stays untouched, and hierarchy derivation is a few lines in `store.ts` (`split('/')`, fold into a directory trie).
- **Expansion state as `Set<dirPath>` in the store, default-expanded.** New directories appear expanded (matches today's "everything visible" behavior); user toggles mutate the set; `refreshState` replaces `files` but never the set, so persistence across reload/live-refresh falls out for free. Persisting to `localStorage` was rejected as unnecessary for a single-session tool.
- **Effective expansion = stored set ∪ ancestors-of-visible when filtered.** Under `filter=modified`, ancestors of matching files are computed at render time and unioned in, so switching back to "All files" restores the user's manual state untouched (no state corruption from transient filter views).
- **Compact row anatomy (single line, ~24px):** status dot → chevron (directories only) → name → ext badge; directory rows show a contained-file count right-aligned; numstat (`+N −M`) stays right-aligned on modified files. Size · modification time moves to the row `title` tooltip. Directory toggling uses click-on-row (whole row is the hit target — compact and touch-friendly).
- **Deleted-status files keep flowing through.** The git merge already injects tracked-but-deleted files absent from disk; with recursion they simply carry their `rel_path` and render as leaves like any other entry.

- **Per-file error counts are frontend-driven (check-on-refresh).** After every listing fetch — initial load, reload button, `files_changed` live refresh (debounced, trailing) — the frontend runs one `POST /api/check` and groups `diagnostics[].location.file` into `diagnosticsByFile: Map<path, {errors, warnings}>` in the store. Rationale: diagnostics already carry source files, the build set and the listing are now the same set (so paths align), and a local dev tool tolerates a full recheck per save. Rejected alternative: server-side aggregation (watcher-triggered check cached in `WebuiState`, counts on `FileEntry`) — better latency but adds cache/trigger machinery and listing latency; revisit if per-save rechecks feel sluggish on real repos.
- **Failure and edge handling**: a failed or in-flight check renders rows without error indicators (no stale numbers); diagnostics with `location.file = None` (e.g. the "no spreadsheets found" warning) surface only in the header tooltip; path keys are normalized the same way as `rel_path` (`/` separators) so grouping matches entries exactly.
- **Sort state and semantics**: `sortFactor: 'name' | 'modified' | 'errors'` plus direction live in the store (default `name`, ascending). Files sort within their directory by the factor; directories sort by the matching aggregate — name (alphabetical), modified (latest contained mtime), errors (total contained errors, warnings as tiebreak). Error sort is stable when counts are missing: missing counts compare as zero, so ordering degenerates toward name order rather than jumping.
- **Compact row anatomy accommodates health + order**: the single-line row gains a trailing error/warning badge (count, colored) after the ext badge; directory rows show aggregate counts next to the file count; the rail header shows the listing total. The sort control lives in the compact header (factor cycle button or small select + direction arrow) so the rail stays one column wide.

## Risks / Trade-offs

- **Default listings shrink**: users relying on `.xlsb`/`.ods` appearing in the rail will no longer see them (the build path never accepted them). Mitigate via empty-state copy ("no files match the build include patterns — add `include` to `tablec.toml`") and a note in the README/design doc; file a follow-up issue for the core whitelist.
- **Glob dialect surprises**: `find_excel_files` strips a leading `**/` from patterns, so the default `*.xlsx` (and `**/*.xlsx`) match only the directory root; genuine recursion requires `/**/*.xlsx`. Same semantics as build, but now visible in the UI; the empty-state hint recommends `/**/*.xlsx`, and the dialect quirk belongs in the core follow-up issue.
- **Windows separators**: glob yields platform separators; `rel_path` normalization to `/` must happen backend-side so the frontend split is trivial and stable.
- **Deep trees**: no virtualization; acceptable for gamedev table projects (hundreds of files), revisit if listings reach thousands.
- **Full recheck per save**: every `files_changed` burst re-parses the whole build set; debouncing keeps editor save-storms to one check, but very large repos may feel it. Mitigation if needed later: server-side aggregation (the rejected alternative) or checking only the changed file.
- **Check/listing skew**: the check runs against the same build set the listing shows, but a file saved between listing fetch and check completion can make counts one refresh stale; acceptable for a live-reloading tool (the next refresh corrects it).
- **Dependency on the shared check pipeline (`unify-check-logic`)**: these badges consume `POST /api/check`, whose current implementation produces false `@ref` positives (project validation runs per-file over a partial table set) and duplicates project diagnostics once per file — the rail would render wrong counts today. `unify-check-logic` fixes both defects at the source and should land first; no design change needed here beyond the ordering.

## Migration

None — the webui frontend is the only known consumer of `/api/files`; it ships in the same change. The openapi-less HTTP surface changes shape additively (`rel_path` added) with a narrowing of the listing set, called out as **BREAKING** in the proposal.

## Testing

- Rust: extend the existing `handlers.rs` fixture tests — recursion into subdirectories, `rel_path` presence/normalization, exclude-pattern rejection, modified-filter across subdirectories.
- Frontend: `pnpm check && pnpm build` (typecheck); manual verification matrix (tree render, toggle, tooltip, persistence across live refresh, modified-only auto-expand, nested selection → preview, error/warning badges per row and directory, header total, sort by each factor with direction toggle, degraded rendering when check fails) via `pnpm dev` against a fixture tree; `cargo test -p tablec-webui` for the backend.
