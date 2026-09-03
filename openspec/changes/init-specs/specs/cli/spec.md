## Purpose

Defines the behavior of the `tablec` command-line tool: its `build`, `check`, and `example` subcommands, input and config resolution, supported output formats, and the observable output and exit behavior.

## ADDED Requirements

### Requirement: build subcommand
`tablec build` SHALL accept an input path (file or directory), an output path (`-o`), a config path (`--config`), a format (`--format`), `--include-fields`, a parser name (`--parser`), and repeatable `--plugin-path`. When the input is a file, a single merged project is exported; when the input is a directory, all matching spreadsheet files are merged into one project. The merged output SHALL write a JSON or MessagePack file and print `Exported data to <output>`.

#### Scenario: Build a single Excel file
- **WHEN** `tablec build -i input.xlsx -o out.json` runs on a valid workbook
- **THEN** `out.json` exists, is valid JSON, and contains the sheet data

#### Scenario: Build a directory with default discovery
- **WHEN** `tablec build <dir>` runs and `<dir>` contains `tablec.toml` (or `.tablec.toml`)
- **THEN** the config is auto-discovered and the project is exported per its `[export]` settings

#### Scenario: Directory without config or -o
- **WHEN** `tablec build <dir>` runs with no `tablec.toml` and no `-o`
- **THEN** the command fails with a message about a missing `-o` or config

#### Scenario: Unsupported format
- **WHEN** `--format` is not one of `json`, `json-pretty`, `msgpack`
- **THEN** the command fails with a message listing the supported formats

### Requirement: Format selection and file extension
The `--format` flag SHALL support `json` (minified), `json-pretty` (indented), and `msgpack`. The extension of the written file SHALL be `.json` for `json`/`json-pretty` and `.msgpack` for `msgpack`.

#### Scenario: Extension follows format
- **WHEN** a build uses `--format msgpack`
- **THEN** the output defaults to a `.msgpack` extension

### Requirement: check subcommand
`tablec check [path]` SHALL validate the tables found in `path` (a file or directory, defaulting to the config's `input_dir` or the current directory), running per-table constraint validation. It SHALL print a per-sheet result and a summary of the number of errors found; when any error is found it SHALL exit non-zero, otherwise it SHALL exit zero. Per-table validation MUST be run with `validate_table` (cross-table `@ref` is not part of this command's checks).

#### Scenario: Check with no errors
- **WHEN** all tables validate
- **THEN** the command prints a success message and exits zero

#### Scenario: Check with errors
- **WHEN** at least one table violates a constraint
- **THEN** the command prints the diagnostics and an error count, and exits non-zero

#### Scenario: Check finds no spreadsheet files
- **WHEN** the target contains no matching files
- **THEN** the command prints a notice and exits zero

### Requirement: example subcommand
`tablec example -o <path>` SHALL generate an example `.xlsx` file with a standard 5-row schema header and `-r <n>` (default 10) data rows covering the type system (int, string, float, bool, string array, struct, nested array, map, and struct array), with `@unique` on the id column. It SHALL refuse to overwrite an existing file unless `--force` is given.

#### Scenario: Generate example file
- **WHEN** `tablec example -o example.xlsx -r 10` runs
- **THEN** `example.xlsx` is created with 10 data rows and the documented columns

#### Scenario: Refuse overwrite without --force
- **WHEN** `tablec example -o existing.xlsx` runs and the file already exists without `--force`
- **THEN** the command fails with a message about the existing file

### Requirement: Error reporting
Command failures SHALL render diagnostics to stderr with severity color-coding, and errors found during `check` SHALL be reported on stderr with a final error count. Build progress and per-table stats SHALL be printed to stderr, and successful export messages SHALL be printed to stdout.

#### Scenario: Diagnostics render to stderr
- **WHEN** a build or check encounters parse or constraint diagnostics
- **THEN** they appear on stderr

### Requirement: Tablec toml config
The tool SHALL accept `tablec.toml` (preferred) or `.tablec.toml` with `[project]` (name, version, description), `[data]` (input_dir, include, exclude), `[export]` (format, output_dir, pretty, include_fields), `[parser]` (name), and `[[plugins]]` (path). CLI flags SHALL override config values. `--config` SHALL override auto-discovery.

#### Scenario: CLI overrides config
- **WHEN** config sets `format = "msgpack"` but `--format json` is passed
- **THEN** the output is JSON

#### Scenario: Include/exclude globs filter input
- **WHEN** `[data] include = ["*.xlsx"]` and `exclude` excludes a file
- **THEN** the excluded file is not parsed