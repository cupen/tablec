## MODIFIED Requirements

### Requirement: File and preview endpoints
`GET /api/files` SHALL list supported spreadsheets (`.xlsx`, `.xls`, `.xlsb`, `.ods`) under the resolved input directory with name, path, size, and modification time. `GET /api/sheets?path=` SHALL list a workbook's sheets (excluding `#`-prefixed names) with row/column counts. `GET /api/preview?path=&sheet=&max_rows=` SHALL return the raw sheet grid as rows of cells with a `sheet` name, clamped between 5 and 1000 rows. `GET /api/parsed_preview?path=&sheet=&parser=&max_rows=` SHALL run the schema parser plus per-cell type check and return `{ sheet, schema, data_start_row, rows, summary }` with an error-count summary; an unknown parser name SHALL be rejected with a bad-request error naming the available parsers.

`GET /api/files` entries SHALL additionally carry a git change status for the file (`modified`, `added`, `untracked`, `deleted`, or `clean`) and, for `modified` files, the added/deleted line counts, derived from the repository's current branch HEAD. The parsed preview rows SHALL additionally carry a per-cell diff status (`added`, `deleted`, `modified`, or `unchanged`) derived from the same git baseline. The webui SHALL expose a filter that returns only changed files; the file count reported by the webui SHALL honor the active filter (total vs changed).

#### Scenario: Files lists only recognized spreadsheets
- **WHEN** the input directory contains `a.xlsx`, `notes.csv`, and `b.ods`
- **THEN** `/api/files` returns entries for `a.xlsx` and `b.ods` only

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

### Requirement: Check endpoint
`POST /api/check` SHALL accept `{ dir, parser?, plugin_paths? }`, parse the configured input directory, and return `{ diagnostics, duration_ms, sheets_checked }`. Unlike the CLI `check`, this endpoint SHALL run cross-table `@ref` validation (`validate_project`) across all parsed tables. A missing resolved input directory SHALL be rejected as 400 with a hint about `tablec.toml`.

The check endpoint SHALL NOT itself change the diff baseline; the git HEAD baseline is the reference for change detection and is not mutated by build or check actions.

#### Scenario: Check runs @ref validation
- **WHEN** the parsed tables contain a violating `@ref`
- **THEN** the returned `diagnostics` includes the foreign-key violation

#### Scenario: Missing input directory
- **WHEN** the config points at a nonexistent `data` directory
- **THEN** the response is 400 with a message mentioning the input directory

#### Scenario: Diff baseline is not a build cache
- **WHEN** a build or check runs after a diff was displayed
- **THEN** the diff statuses remain relative to git HEAD, not to the last build/check result