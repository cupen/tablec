## Purpose

Defines the behavior of the local webui server (`tablec webui`): the HTTP API surface for previewing, building and checking spreadsheets, the embedded static frontend, and the security boundary that keeps plugin loading CLI-only.

## ADDED Requirements

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
`GET /api/files` SHALL list supported spreadsheets (`.xlsx`, `.xls`, `.xlsb`, `.ods`) under the resolved input directory with name, path, size, and modification time. `GET /api/sheets?path=` SHALL list a workbook's sheets (excluding `#`-prefixed names) with row/column counts. `GET /api/preview?path=&sheet=&max_rows=` SHALL return the raw sheet grid as rows of cells with a `sheet` name, clamped between 5 and 1000 rows. `GET /api/parsed_preview?path=&sheet=&parser=&max_rows=` SHALL run the schema parser plus per-cell type check and return `{ sheet, schema, data_start_row, rows, summary }` with an error-count summary; an unknown parser name SHALL be rejected with a bad-request error naming the available parsers.

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
`POST /api/check` SHALL accept `{ dir, parser?, plugin_paths? }`, parse the configured input directory, and return `{ diagnostics, duration_ms, sheets_checked }`. Unlike the CLI `check`, this endpoint SHALL run cross-table `@ref` validation (`validate_project`) across all parsed tables. A missing resolved input directory SHALL be rejected as 400 with a hint about `tablec.toml`.

#### Scenario: Check runs @ref validation
- **WHEN** the parsed tables contain a violating `@ref`
- **THEN** the returned `diagnostics` includes the foreign-key violation

#### Scenario: Missing input directory
- **WHEN** the config points at a nonexistent `data` directory
- **THEN** the response is 400 with a message mentioning the input directory

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