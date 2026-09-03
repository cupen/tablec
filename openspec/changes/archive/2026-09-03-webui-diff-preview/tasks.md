## 1. Backend — git diff plumbing module

- [x] 1.1 Add `tablec-webui/src/git.rs` (or `diff.rs`) with repo resolution (`git rev-parse --show-toplevel`), falling back to no-diff when outside a repo or HEAD-less; verify by unit test against a temp repo
- [x] 1.2 Implement per-file status via `git status --porcelain -- <dir>` mapped to `modified|added|untracked|deleted|clean` with `git diff --numstat` counts for `modified`; verify by unit test asserting each status class and the add/del counts
- [x] 1.3 Implement HEAD-blob retrieval (`git show HEAD:<path>` → bytes) and temp-file materialization for re-parsing; verify a modified `.xlsx` yields a parseable HEAD blob
- [x] 1.4 Implement per-cell diff of two parsed sheets: align rows by unique column (fallback: row index), mark added/deleted/modified/unchanged, numeric-equal values treated equal; verify with unit tests for modified cell, new row, removed row, numeric equality, and no-unique fallback
- [x] 1.5 Run `cargo test -p tablec-webui` and confirm the new module tests pass; confirm existing webui tests still pass

## 2. Backend — wire into handlers

- [x] 2.1 Extend `/api/files` — each `FileEntry` gains `status`, `numstat_added`, `numstat_deleted`; add `filter=modified` query param that returns only non-clean files; verify via new handler tests against a temp git repo (create-repo, commit, modify, list)
- [x] 2.2 Extend `/api/parsed_preview` — each parsed cell gains `diff` (`added|deleted|modified|unchanged`) computed against HEAD for the previewed sheet; verify via a handler test asserting per-cell statuses on a modified fixture
- [x] 2.3 Ensure untracked files report `untracked` in the file list and `added` per-cell; ensure missing-file (`deleted`) appears in the modified filter; verify with tests
- [x] 2.4 Ensure no-git / no-HEAD / missing-`git` fallbacks return clean + warning (non-500); ensure handler tests skip when `git` is unavailable; verify with a test marked `#[ignore]`-style or skipped
- [x] 2.5 Run `cargo test -p tablec-webui` again; run `cargo fmt --all --check` and `cargo clippy -p tablec-webui` clean

## 3. Frontend — file list filter + status badges

- [x] 3.1 Add `store.filesFilter: 'all' | 'modified'` and a filtered `changedFileCount` helper; extend `FileEntry` type with `status`/counts; verify via typing + a unit-style check in `store.ts`
- [x] 3.2 In `api.ts`, pass `filter` from the store to `/api/files` and keep `/api/parsed_preview` unchanged; verify the network call includes the query param when the filter is active
- [x] 3.3 In `file-list.ts`, add a toggle control ("All files" / "Modified only") that flips `store.filesFilter` and re-fetches; the count badge shows total vs changed per the active filter; verify by rendering both modes and asserting the rendered `li` set
- [x] 3.4 Add per-file status presentation — colored status dot (red deleted / green added), show add/del counts on `modified` rows; verify visually and with a DOM assertion on a fixture file list
- [x] 3.5 Run `pnpm check && pnpm build` in `tablec-webui/webui/` to typecheck and emit `dist/`; verify no TypeScript errors

## 4. Frontend — parsed preview cell diff colors

- [x] 4.1 Extend `ParsedCell` type with `diff`; in `file-preview.ts`, map `deleted`→red background, `added`→green background, `modified`→yellow (amber) background; verify via DOM/class assertions on a rendered parsed preview with mixed statuses
- [x] 4.2 Ensure legend/tooltip or subtle styling communicates the three colors; verify the rendered grid shows the classes in the real webui build
- [x] 4.3 Run `pnpm check && pnpm build` again and confirm the compiled bundle contains the diff classes

## 5. Integration + docs + quality gates

- [x] 5.1 Integration: `cargo build`, then run the webui against a temp repo with a modified spreadsheet and confirm (a) the left menu shows the status/counts, (b) the Modified-only filter hides clean files, (c) the preview shows green/red/yellow cells; verify by manual/browser check or an end-to-end handler test
- [x] 5.2 Update the `webui` main spec section (already delta'd) is synced in a follow-up; update `docs/design.md` and the webui README snippet about diff preview; verify docs mention the git-HEAD baseline and the fallback
- [x] 5.3 Run the full gates: `cargo test --workspace` (or `-p tablec-core -p tablec-webui`), `cargo fmt --all --check`, `cd tablec-webui/webui && pnpm check && pnpm build`; confirm all green
- [x] 5.4 File issues for any deferred items (cross-branch compare, `untracked` badge distinct styling) via `bd`; close or claim the implementation issue when done