## MODIFIED Requirements

### Requirement: File and preview endpoints
`GET /api/files` SHALL list the same set of spreadsheets that `POST /api/build` and `POST /api/check` operate on: it SHALL recursively scan the resolved input directory using the active configuration's include and exclude glob semantics (when the configuration defines no include patterns, the default pattern is `*.xlsx`, and the extension acceptance SHALL match the build path exactly). Each entry SHALL carry the file name, its path, its path relative to the input directory, size, and modification time. `GET /api/sheets?path=` SHALL list a workbook's sheets (excluding `#`-prefixed names) with row/column counts. `GET /api/preview?path=&sheet=&max_rows=` SHALL return the raw sheet grid as rows of cells with a `sheet` name, clamped between 5 and 1000 rows. `GET /api/parsed_preview?path=&sheet=&parser=&max_rows=` SHALL run the schema parser plus per-cell type check and return `{ sheet, schema, data_start_row, rows, summary }` with an error-count summary; an unknown parser name SHALL be rejected with a bad-request error naming the available parsers.

`GET /api/files`, `GET /api/sheets`, `GET /api/preview` and `GET /api/parsed_preview` SHALL accept optional `left` and `right` parameters naming the comparison sides (defaults `worktree` and `HEAD`); `worktree` SHALL only be valid as the left side. `/api/files` entries SHALL carry a change status of the left side relative to the right side (the baseline) — `added`, `deleted`, `modified`, or `clean`, preserving the working-tree-specific `untracked` and staged-`added` statuses when both sides are at their defaults — and, for `modified` files, the added/deleted line counts. With non-default sides the compared file set is every supported spreadsheet existing on either side under the resolved input directory. `/api/sheets` and both preview endpoints SHALL resolve their workbook from the same body source — the left side's content (the working-tree file when the left side is `worktree`, the materialized blob of the left ref otherwise), except for files that exist only on the right side, which SHALL serve the right side's blob so deleted files stay previewable in every mode. The parsed preview rows SHALL carry a per-cell diff status (`added`, `deleted`, `modified`, or `unchanged`) derived from the same comparison sides. A side naming a ref git cannot resolve SHALL be rejected with a bad-request error naming the ref. The webui SHALL expose a filter that returns only changed files; the file count reported by the webui SHALL honor the active filter (total vs changed).

#### Scenario: Files lists only recognized spreadsheets
- **WHEN** the input directory contains `a.xlsx`, `sub/b.xlsx`, `notes.csv`, and `b.ods` under the default configuration (`*.xlsx`, which matches only the directory root)
- **THEN** `/api/files` returns an entry for `a.xlsx` only, carrying its path relative to the input directory

#### Scenario: Files recurses when the include pattern says so
- **WHEN** the active configuration sets include patterns to `/**/*.xlsx` and the input directory contains `a.xlsx` and `sub/b.xlsx`
- **THEN** `/api/files` returns entries for both, with `sub/b.xlsx`'s entry carrying its path relative to the input directory

#### Scenario: Include and exclude patterns are honored
- **WHEN** the active configuration sets include patterns to `**/*.xlsx` and exclude patterns to `**/draft_*.xlsx`, and the input directory contains `a.xlsx` and `draft_b.xlsx` nested in subdirectories
- **THEN** `/api/files` returns an entry for `a.xlsx` and none for `draft_b.xlsx`

#### Scenario: Preview returns raw grid
- **WHEN** a client requests `/api/preview` for an existing file and sheet
- **THEN** the response is 200 with a `sheet` name and a `rows` array of cells

#### Scenario: Parsed preview runs schema parsing
- **WHEN** a client requests `/api/parsed_preview` for an existing file and sheet with the standard parser
- **THEN** the response includes a parsed `schema` with fields, `data_start_row` 5, typed `rows`, and a `summary` with `error_count` and `total_rows`

#### Scenario: Unknown parser rejected
- **WHEN** a client requests `/api/parsed_preview?parser=does-not-exist`
- **THEN** the response is 400 with a message naming the unavailable parser

#### Scenario: Files endpoint includes change status
- **WHEN** a client calls `GET /api/files` for a directory inside a git repository containing a modified spreadsheet
- **THEN** each entry includes a `status` field, and the modified file's entry also includes added/deleted counts

#### Scenario: Files compares two branches
- **WHEN** a client calls `GET /api/files?left=feature-x&right=main`
- **THEN** the response lists spreadsheets present on either branch under the input directory, with statuses relative to `main`

#### Scenario: Preview body follows the left side
- **WHEN** a client requests a preview with `left=feature-x` and the file differs between `feature-x` and the working tree
- **THEN** the preview body is the content materialized from the `feature-x` blob

#### Scenario: Deleted file is previewable
- **WHEN** a client requests `GET /api/parsed_preview` for a file that exists only on the right side
- **THEN** the response is 200 rendering the right side's blob with every data row carrying `deleted`

#### Scenario: Sheets resolves the same body source
- **WHEN** a client requests `GET /api/sheets` for a file absent from the working tree but present on the left ref
- **THEN** the sheet list comes from the left ref's blob, not a 404

#### Scenario: Parsed preview includes cell diff statuses
- **WHEN** a client calls `/api/parsed_preview` for a modified spreadsheet
- **THEN** every parsed cell includes a `diff` status field with one of `added`, `deleted`, `modified`, `unchanged`

#### Scenario: Unknown comparison ref rejected
- **WHEN** a client calls `GET /api/files?right=no-such-branch`
- **THEN** the response is 400 with a message naming `no-such-branch`

#### Scenario: Filter returns only changed files
- **WHEN** the client requests the file list with the modified-only filter
- **THEN** the list contains only files whose git status is not `clean`; the reported count reflects that filtered set

#### Scenario: Changed files in subdirectories are filterable
- **WHEN** the modified-only filter is active and a modified spreadsheet is nested in a subdirectory
- **THEN** the filtered list contains that file's entry

## ADDED Requirements

### Requirement: Branch listing endpoint
`GET /api/branches` SHALL return the repository's local and remote-tracking branch names, deduplicated by short name with the local branch winning. Outside a git repository, or when git is unavailable, it SHALL return an empty list with a warning field rather than an error.

#### Scenario: Lists local and remote branches
- **WHEN** the repository has local branch `main` and remote-tracking branch `origin/dev`
- **THEN** `GET /api/branches` returns `main` and `origin/dev`

#### Scenario: Deduplicates local against remote
- **WHEN** `main` exists both locally and as `origin/main`
- **THEN** the listing shows `main` once

#### Scenario: Outside a repository
- **WHEN** the input directory is not inside a git repository
- **THEN** `GET /api/branches` returns 200 with an empty branch list and a warning

### Requirement: Comparison selector
The webui frontend SHALL provide a comparison selector with a left and a right dropdown. The left dropdown SHALL offer `worktree` plus the enumerated branches; the right dropdown SHALL offer the enumerated branches and accept free-form ref input (tag or commit hash). Defaults SHALL be `worktree` (left) and `HEAD` (right). The selection SHALL persist across sessions via localStorage. All change-status badges, the modified-only filter, and preview diff highlighting SHALL be rendered relative to the active selection.

#### Scenario: Default selection matches existing behavior
- **WHEN** the webui loads with no persisted selection
- **THEN** the comparison is `worktree` vs `HEAD` and the file list and previews behave exactly as before this change

#### Scenario: Selection persists across reloads
- **WHEN** the user selects `left=worktree`, `right=main` and reloads the page
- **THEN** the selector restores `worktree` vs `main` without user interaction

#### Scenario: Free-form ref on the right
- **WHEN** the user types a tag or abbreviated commit hash into the right dropdown
- **THEN** the comparison uses that ref, or the UI surfaces the backend's named-ref error when it does not resolve
