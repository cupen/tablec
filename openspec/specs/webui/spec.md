# webui Specification

## Purpose

Defines the behavior of the local webui server (`tablec webui`): the HTTP API surface for previewing, building and checking spreadsheets, the embedded static frontend, and the security boundary that keeps plugin loading CLI-only.

## Requirements

### Requirement: Server startup
`tablec webui` SHALL bind to a host (default `127.0.0.1`) and port (default 9527), serve the embedded web frontend, and auto-open the browser unless `--no-browser` is passed. The default host MUST be loopback so the server is not exposed publicly.

#### Scenario: Launch with default host
- **WHEN** `tablec webui --dir ./data --no-browser` runs
- **THEN** the server listens on `127.0.0.1` at the chosen port and serves the frontend at `/`

### Requirement: Health and state endpoints
`GET /api/health` SHALL return `{ ok: true, version, uptime_secs }`. `GET /api/state` SHALL return the resolved working directory, host, observed port, registered parser names, active parser, config path (present or not), and the resolved input directory.

#### Scenario: Health reports ok
- **WHEN** a client calls `GET /api/health`
- **THEN** the response is 200 with `ok: true` and a version string

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

### Requirement: Build endpoint
`POST /api/build` SHALL accept `{ dir, format, pretty, include_fields, write, output_path?, parser?, plugin_paths? }`, parse the configured input directory with the selected parser, and return `{ format, bytes, preview_first_500, diagnostics, output_path, duration_ms, written }`. Unsupported formats MUST be rejected with 400. When `write` is true the artifact SHALL be written to `output_path` (or the config-derived default) and the response SHALL report `written: true`. Build does not write by default.

#### Scenario: Build returns artifact preview
- **WHEN** a client posts a build request with `format: "json-pretty"` for a valid directory
- **THEN** the response is 200 with a positive `bytes` count, a `preview_first_500` starting with the project name, and `written: false`

#### Scenario: Build writes on request
- **WHEN** a client posts a build request with `write: true`
- **THEN** the output file exists at the reported `output_path` and the response has `written: true`

#### Scenario: Unsupported format rejected
- **WHEN** a client posts `format: "protobuf"`
- **THEN** the response is 400 with a message listing supported formats

### Requirement: Check endpoint
`POST /api/check` SHALL accept `{ dir, parser?, plugin_paths? }` and validate the configured input directory through the same shared check pipeline as the CLI `check` subcommand: files enumerated with the configured include/exclude globs, parsed with the selected parser (request `parser` or server override, resolved via the parser registry), validated per table, and validated once at project level across all parsed tables so cross-table `@ref` constraints are enforced. It SHALL return `{ diagnostics, duration_ms, sheets_checked }`, with each project-level diagnostic reported exactly once. A missing resolved input directory SHALL be rejected as 400 with a hint about `tablec.toml`.

The check endpoint SHALL NOT itself change the diff baseline; the git HEAD baseline is the reference for change detection and is not mutated by build or check actions.

#### Scenario: Check runs @ref validation
- **WHEN** the parsed tables contain a violating `@ref` whose target lives in another file
- **THEN** the returned `diagnostics` includes the foreign-key violation exactly once

#### Scenario: Missing input directory
- **WHEN** the config points at a nonexistent `data` directory
- **THEN** the response is 400 with a message mentioning the input directory

#### Scenario: Diff baseline is not a build cache
- **WHEN** a build or check runs after a diff was displayed
- **THEN** the diff statuses remain relative to git HEAD, not to the last build/check result

#### Scenario: Check matches CLI semantics
- **WHEN** the same input directory (same parser selection) is checked via `tablec check` and `POST /api/check`
- **THEN** both report the same set of diagnostics

### Requirement: Validate endpoint is not implemented
`POST /api/validate` SHALL return 501 with an error body `{ error: "not_implemented", message, todo }` — the data-validation feature is a documented TODO and is not part of the current system.

#### Scenario: Validate returns 501
- **WHEN** a client posts to `/api/validate`
- **THEN** the response is 501 with `error: "not_implemented"` and a `todo` field

### Requirement: Plugin paths never accepted over HTTP
`/api/build` and `/api/check` MUST reject any request whose `plugin_paths` is non-empty with a 400 error; plugin libraries are loadable only through CLI flags (`--plugin-path`). Unknown parser names in either endpoint SHALL be rejected with 400.

#### Scenario: Build rejects plugin_paths
- **WHEN** a build request includes `plugin_paths: ["/tmp/evil.so"]`
- **THEN** the response is 400 with a message about plugin paths being CLI-only

#### Scenario: Check rejects plugin_paths
- **WHEN** a check request includes `plugin_paths`
- **THEN** the response is 400

### Requirement: Static asset serving and caching
The frontend SHALL be served from the embedded Vite build. Content-hashed assets under `assets/` SHALL be served with `Cache-Control: public, max-age=31536000, immutable`; `index.html` (direct, root, and SPA-fallback routes) SHALL be served with `Cache-Control: no-cache`. Unknown extensionless paths SHALL fall back to `index.html`; unknown paths with an extension SHALL 404. Unknown `/api/*` paths SHALL 404 and MUST NOT be answered by the static catch-all.

#### Scenario: Hashed assets cached immutably
- **WHEN** a client requests a hashed asset under `assets/`
- **THEN** the response carries `Cache-Control: public, max-age=31536000, immutable`

#### Scenario: Index revalidates
- **WHEN** a client requests `/`, `/index.html`, or any unknown extensionless route
- **THEN** the response is 200 HTML with `Cache-Control: no-cache`

#### Scenario: Unknown API route 404s
- **WHEN** a client requests `/api/does-not-exist`
- **THEN** the response is 404

### Requirement: Live file-change notifications
The webui server SHALL watch the resolved input directory for file-system changes and SHALL notify connected browsers over a WebSocket endpoint so the file list refreshes without a manual reload. Change detection SHALL cover creation, modification, removal, and rename of files anywhere under the input directory, including spreadsheets and configuration files. A detected change SHALL cause any connected client to re-fetch the file list; the server SHALL NOT start a build or check, and SHALL NOT itself re-run the schema parser, as a consequence of a change notification. Watching SHALL operate regardless of whether the directory is inside a git repository.

#### Scenario: Spreadsheet modified while viewing
- **WHEN** a spreadsheet under the input directory is modified while a browser client is connected
- **THEN** the client receives a change notification over the WebSocket and the refreshed file list reflects the modification

#### Scenario: New spreadsheet appears
- **WHEN** a spreadsheet is created under the input directory while a client is connected
- **THEN** the client receives a change notification and the refreshed file list includes the new file

#### Scenario: Spreadsheet removed
- **WHEN** a spreadsheet is deleted from the input directory while a client is connected
- **THEN** the client receives a change notification and the refreshed file list no longer includes the file

#### Scenario: Change does not trigger a build
- **WHEN** a change notification is delivered to a client
- **THEN** no build or check is started by the server as a side effect

### Requirement: WebSocket endpoint and client lifecycle
The server SHALL expose a WebSocket endpoint for change notifications. A connected client SHALL receive a notification message whenever the watcher detects a change in the input directory; the message SHALL indicate that the file list changed. If the connection drops, the client SHALL reconnect with a backoff delay and SHALL re-fetch the file list after a successful reconnect, so no inotify/ReadDirectoryChangesW events are lost to a disconnected browser. A client that cannot maintain a connection SHALL still function via the existing manual reload control.

#### Scenario: Client receives change message
- **WHEN** a client is connected to the WebSocket endpoint and a change occurs in the input directory
- **THEN** the client receives a message indicating the file list changed

#### Scenario: Reconnect after disconnect
- **WHEN** a client's WebSocket connection drops and the client reconnects
- **THEN** the client re-fetches the file list after the reconnect succeeds

#### Scenario: Manual reload remains available
- **WHEN** the WebSocket is unavailable or disconnected
- **THEN** the existing reload control still refreshes the file list

### Requirement: Config resolution mirrors the CLI
The webui SHALL resolve its configuration through the same precedence as the CLI subcommands: an explicit `--config` path if given, else `tablec.toml`/`.tablec.toml` in the process cwd, else `tablec.toml`/`.tablec.toml` in the `--dir` target directory, else all defaults. Config-relative paths (`data.input_dir`, `export.output_dir`, `plugins[].path`) SHALL resolve against the directory containing the config file, with the `--dir` directory standing in when no config file was found. Unknown config keys SHALL be rejected with an error naming the offending key, and `export.pretty` SHALL NOT be accepted.

#### Scenario: cwd config wins over --dir config
- **WHEN** the webui is launched with `--dir ./proj` and both the cwd and `./proj` contain a `tablec.toml`
- **THEN** the cwd config governs input scanning and export settings

#### Scenario: input_dir resolves against the config file directory
- **WHEN** the governing config lives in `./proj/tablec.toml` with `input_dir = "sheets"`
- **THEN** the file tree, preview, build, and check endpoints operate on `./proj/sheets`

#### Scenario: Invalid config fails startup loudly
- **WHEN** the governing config contains an unknown key
- **THEN** the webui reports the config error instead of silently starting with defaults

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