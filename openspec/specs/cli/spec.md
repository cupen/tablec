# cli Specification

## Purpose

Defines the behavior of the `tablec` command-line tool: its `build`, `check`, and `example` subcommands, input and config resolution, supported output formats, and the observable output and exit behavior.

## Requirements

### Requirement: build subcommand
`tablec build` SHALL accept an input path (file or directory), an output path (`-o`), a config path (`--config`), a format (`--format`), `--include-fields`, a parser name (`--parser`), and repeatable `--plugin-path`. The input location SHALL resolve in this order: an explicit `-i` argument wins; when omitted, the resolved config's `data.input_dir` is used. When the input is a file, a single merged project is exported and `-o` is required; when the input is a directory, all matching spreadsheet files are merged into one project. The directory-mode output path SHALL resolve as `-o` if given, otherwise `{output_dir}/{project.name}.{ext}` derived from config defaults — zero-config builds SHALL succeed without `-o`. The merged output SHALL write a JSON or MessagePack file and print `Exported data to <output>`.

#### Scenario: Build a single Excel file
- **WHEN** `tablec build -i input.xlsx -o out.json` runs on a valid workbook
- **THEN** `out.json` exists, is valid JSON, and contains the sheet data

#### Scenario: Build a directory with default discovery
- **WHEN** `tablec build` runs and the process cwd contains `tablec.toml` (or `.tablec.toml`)
- **THEN** the config is auto-discovered and the project is exported per its `[data]`/`[export]` settings

#### Scenario: Input dir from config when -i omitted
- **WHEN** `tablec build` runs without `-i` and the resolved config sets `input_dir = "sheets"`
- **THEN** spreadsheet files are scanned under `<config-dir>/sheets`

#### Scenario: Directory without config or -o
- **WHEN** `tablec build` runs with no `tablec.toml` discoverable and no `-o`
- **THEN** the command succeeds using defaults and writes `output/default.json` relative to the cwd

#### Scenario: Directory with no matching spreadsheets
- **WHEN** the resolved input directory contains no files matching the include/exclude patterns
- **THEN** the command fails with a message about no matching spreadsheet files

#### Scenario: Unsupported format
- **WHEN** `--format` is not one of `json`, `json-pretty`, `msgpack`
- **THEN** the command fails with a message listing the supported formats

### Requirement: Format selection and file extension
The `--format` flag SHALL support `json` (minified), `json-pretty` (indented), and `msgpack`. A config-derived default output SHALL take the extension `.json` for `json`/`json-pretty` and `.msgpack` for `msgpack`; an explicit `-o` path is used verbatim without appending an extension.

#### Scenario: Extension follows format
- **WHEN** a build uses `--format msgpack`
- **THEN** the output defaults to a `.msgpack` extension

### Requirement: check subcommand
`tablec check [path]` SHALL validate the tables found in `path` (a file or directory, defaulting to the config's `input_dir` or the current directory) using the same shared check pipeline as the webui check endpoint: files are enumerated with the configured include/exclude globs, parsed with the resolved schema parser (CLI `--parser`/`--plugin-path` overriding config, config overriding the built-in default), validated per table, and then validated once at project level across all parsed tables so cross-table `@ref` constraints are enforced. It SHALL print a per-sheet result and a summary of the number of errors found; when any error is found it SHALL exit non-zero, otherwise it SHALL exit zero.

#### Scenario: Check with no errors
- **WHEN** all tables validate
- **THEN** the command prints a success message and exits zero

#### Scenario: Check with errors
- **WHEN** at least one table violates a constraint
- **THEN** the command prints the diagnostics and an error count, and exits non-zero

#### Scenario: Check finds no spreadsheet files
- **WHEN** the target contains no matching files
- **THEN** the command prints a notice and exits zero

#### Scenario: Check enforces cross-table references
- **WHEN** a checked project contains a `@ref` whose host value is absent from the target column
- **THEN** the command reports a foreign-key violation and exits non-zero

#### Scenario: Check with an unregistered parser fails
- **WHEN** `tablec check --parser not-registered` runs (no parser by that name is registered)
- **THEN** the command fails with an error naming the parser; parser selection precedence (CLI flag over config over the `standard` default) is defined by the schema-parser-plugins capability

#### Scenario: Each project diagnostic is reported once
- **WHEN** the check target contains multiple files and a cross-table violation exists
- **THEN** that violation appears exactly once in the output

### Requirement: example subcommand
`tablec example -o <path>` SHALL generate an example `.xlsx` file with a standard 5-row schema header and `-r <n>` (default 10) data rows covering the type system (int, string, float, bool, string array, struct, nested array, map, and struct array), with `@unique` on the id column. It SHALL refuse to overwrite an existing file unless `--force` is given.

#### Scenario: Generate example file
- **WHEN** `tablec example -o example.xlsx -r 10` runs
- **THEN** `example.xlsx` is created with 10 data rows and the documented columns

#### Scenario: Refuse overwrite without --force
- **WHEN** `tablec example -o existing.xlsx` runs and the file already exists without `--force`
- **THEN** the command fails with a message about the existing file

### Requirement: Error reporting
Command failures SHALL render diagnostics to stderr as plain-text lines prefixed with the severity (`error` / `warning`); no color-coding is applied. Errors found during `check` SHALL be summarized with a final `Found N errors.` count printed to stdout, and the command SHALL exit non-zero. Build progress and per-table stats SHALL be printed to stderr, and successful export messages SHALL be printed to stdout.

#### Scenario: Diagnostics render to stderr
- **WHEN** a build or check encounters parse or constraint diagnostics
- **THEN** they appear on stderr

#### Scenario: Check error count lands on stdout
- **WHEN** `tablec check` finds errors
- **THEN** the summary `Found N errors.` appears on stdout and the command exits non-zero

### Requirement: Tablec toml config
The tool SHALL accept `tablec.toml` (preferred) or `.tablec.toml` with `[project]` (name, version, description), `[data]` (input_dir, include, exclude), `[export]` (format, output_dir, include_fields), `[parser]` (name), and `[[plugins]]` (path). Every section and field SHALL be optional with defaults: `project.name = "default"`, `data.input_dir = "."`, `data.include = ["*.xlsx"]`, `export.format = "json"`, `export.output_dir = "output"`, `export.include_fields = false`; `exclude`, `[parser]`, and `[[plugins]]` default to empty/absent. The `export.pretty` field SHALL NOT exist (indentation is expressed by `format = "json-pretty"`). Unknown keys SHALL be rejected with an error naming the offending key. Config discovery SHALL resolve identically for `build` and `check`: `--config` if given, else `tablec.toml`/`.tablec.toml` in the process cwd, else `tablec.toml`/`.tablec.toml` in the target directory, else all defaults. Relative values of `data.input_dir`, `export.output_dir`, and `plugins[].path` SHALL resolve against the directory containing the config file (absolute paths pass through); when no config file was found they resolve against the cwd. CLI flags SHALL override config values. Include/exclude patterns follow a fixed normalization: a pattern written with a leading `**/` (e.g. `**/*.xlsx`) is rewritten to match only the top level of the input directory — the same set as `*.xlsx`; to match nested paths the pattern MUST be written with a leading `/` (e.g. `/**/*.xlsx`).

#### Scenario: Recursive include needs a leading slash
- **WHEN** `include = ["**/*.xlsx"]` is configured and the input directory contains a root-level and a nested spreadsheet
- **THEN** only the root-level file matches; recursion requires `include = ["/**/*.xlsx"]`

#### Scenario: CLI overrides config
- **WHEN** config sets `format = "msgpack"` but `--format json` is passed
- **THEN** the output is JSON

#### Scenario: Include/exclude globs filter input
- **WHEN** `[data] include = ["*.xlsx"]` and `exclude` excludes a file
- **THEN** the excluded file is not parsed

#### Scenario: Minimal config parses
- **WHEN** a `tablec.toml` contains only a `[parser]` section (or is empty)
- **THEN** it parses successfully and all other values come from defaults

#### Scenario: Unknown key rejected
- **WHEN** a config contains a misspelled key such as `output_dirr`
- **THEN** loading fails with an error naming the unknown key

#### Scenario: cwd config wins over target directory config
- **WHEN** the cwd contains `tablec.toml` and the target input directory also contains one
- **THEN** the cwd config is used

#### Scenario: Relative paths resolve against the config file directory
- **WHEN** a config at `/proj/data/tablec.toml` sets `input_dir = "sheets"` and the command runs from any cwd
- **THEN** spreadsheets are scanned under `/proj/data/sheets`