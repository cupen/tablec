## MODIFIED Requirements

### Requirement: File and preview endpoints
`GET /api/files` SHALL list the same set of spreadsheets that `POST /api/build` and `POST /api/check` operate on: it SHALL recursively scan the resolved input directory using the active configuration's include and exclude glob semantics (when the configuration defines no include patterns, the default pattern is `*.xlsx`, and the extension acceptance SHALL match the build path exactly). Each entry SHALL carry the file name, its path, its path relative to the input directory, size, and modification time. `GET /api/sheets?path=` SHALL list a workbook's sheets (excluding `#`-prefixed names) with row/column counts. `GET /api/preview?path=&sheet=&max_rows=` SHALL return the raw sheet grid as rows of cells with a `sheet` name, clamped between 5 and 1000 rows. `GET /api/parsed_preview?path=&sheet=&parser=&max_rows=` SHALL run the schema parser plus per-cell type check and return `{ sheet, schema, data_start_row, rows, summary }` with an error-count summary; an unknown parser name SHALL be rejected with a bad-request error naming the available parsers.

`GET /api/files` entries SHALL additionally carry a git change status for the file (`modified`, `added`, `untracked`, `deleted`, or `clean`) and, for `modified` files, the added/deleted line counts, derived from the repository's current branch HEAD. The parsed preview rows SHALL additionally carry a per-cell diff status (`added`, `deleted`, `modified`, or `unchanged`) derived from the same git baseline. The webui SHALL expose a filter that returns only changed files; the file count reported by the webui SHALL honor the active filter (total vs changed).

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

#### Scenario: Parsed preview includes cell diff statuses
- **WHEN** a client calls `/api/parsed_preview` for a modified spreadsheet
- **THEN** every parsed cell includes a `diff` status field with one of `added`, `deleted`, `modified`, `unchanged`

#### Scenario: Filter returns only changed files
- **WHEN** the client requests the file list with the modified-only filter
- **THEN** the list contains only files whose git status is not `clean`; the reported count reflects that filtered set

#### Scenario: Changed files in subdirectories are filterable
- **WHEN** the modified-only filter is active and a modified spreadsheet is nested in a subdirectory
- **THEN** the filtered list contains that file's entry

## ADDED Requirements

### Requirement: Compact tree file rail
The webui file rail SHALL render the `/api/files` listing as a directory tree derived from the entries' input-directory-relative paths: directories appear as collapsible rows, spreadsheets appear as selectable leaf rows beneath their directories, and nesting SHALL be visually indicated by indentation. Selecting a leaf row SHALL load that file's preview exactly as the flat list did. Each row SHALL occupy a single line; per-file size and modification time SHALL be available on hover rather than occupying a second row. The webui SHALL preserve directory expansion state across listing re-fetches (manual reload and live file-change refresh). When the modified-only filter is active, the webui SHALL expand the ancestors of every visible file so filtered results are reachable without manual expansion.

#### Scenario: Tree shows hierarchy
- **WHEN** the listing contains files at the root and inside nested subdirectories
- **THEN** the rail shows one directory row per directory with its file rows indented beneath it

#### Scenario: Directories collapse and expand
- **WHEN** the user toggles a directory row
- **THEN** the directory's descendant rows are hidden or shown accordingly

#### Scenario: Rows stay single-line
- **WHEN** the rail renders a file row
- **THEN** the row occupies one line, and the file's size and modification time are revealed on hover instead of consuming a second row

#### Scenario: Expansion survives a re-fetch
- **WHEN** the user collapses a directory and a live file-change refresh re-fetches the listing
- **THEN** that directory remains collapsed

#### Scenario: Modified-only filter auto-expands ancestors
- **WHEN** the modified-only filter is active and a modified file lives inside a collapsed directory
- **THEN** the directory's ancestors are expanded so the file row is visible

#### Scenario: Selecting a nested file previews it
- **WHEN** the user clicks a spreadsheet row located in a subdirectory
- **THEN** the preview pane loads that file's content

### Requirement: File rail error status and sorting
The webui file rail SHALL indicate per file whether the check pipeline reports problems with that file: file rows SHALL show an error indicator with the file's error count (errors visually distinct from warnings), directory rows SHALL show the aggregate counts of their contained files, and the rail header SHALL show the total across the visible listing. The webui SHALL keep these counts current whenever the listing refreshes, including live file-change refreshes. The rail SHALL provide a sort control that orders the tree by a user-chosen factor — file name, modification time, or error count — with a toggleable sort direction; files SHALL order within their directories by the chosen factor and directories SHALL order by the matching aggregate (alphabetical for name, latest contained modification time for modification time, total contained problems for error count). The default sort SHALL be by file name. When check results are unavailable (not yet computed or the check failed), rows SHALL render without error indicators and error-count ordering SHALL degrade to file-name ordering.

#### Scenario: File row shows its error count
- **WHEN** the check pipeline reports 2 errors for a spreadsheet
- **THEN** that file's row shows an error indicator with the count 2

#### Scenario: Warnings are distinct from errors
- **WHEN** the check pipeline reports only warnings for a spreadsheet
- **THEN** that file's row shows a warning indicator rather than an error indicator

#### Scenario: Directory rows aggregate their subtree
- **WHEN** files inside a directory carry error counts
- **THEN** the directory row shows the total problems of its contained files

#### Scenario: Header shows the total
- **WHEN** the listing contains files with problems
- **THEN** the rail header shows the total error count across the visible listing

#### Scenario: Counts refresh on live file changes
- **WHEN** a file change triggers a listing refresh
- **THEN** the error indicators reflect the state after the change

#### Scenario: Sorting by each factor
- **WHEN** the user selects file name, modification time, or error count as the sort factor
- **THEN** files within each directory order by that factor and directories order by the matching aggregate

#### Scenario: Sort direction toggles
- **WHEN** the user toggles the sort direction
- **THEN** the tree ordering reverses

#### Scenario: Name sort is the default
- **WHEN** the webui loads with no user-selected sort
- **THEN** the tree is ordered by file name

#### Scenario: Missing check results degrade gracefully
- **WHEN** check results are unavailable for the current listing
- **THEN** rows render without error indicators and error-count ordering behaves as file-name ordering
