## MODIFIED Requirements

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

### Requirement: Tablec toml config
The tool SHALL accept `tablec.toml` (preferred) or `.tablec.toml` with `[project]` (name, version, description), `[data]` (input_dir, include, exclude), `[export]` (format, output_dir, include_fields), `[parser]` (name), and `[[plugins]]` (path). Every section and field SHALL be optional with defaults: `project.name = "default"`, `data.input_dir = "."`, `data.include = ["*.xlsx"]`, `export.format = "json"`, `export.output_dir = "output"`, `export.include_fields = false`; `exclude`, `[parser]`, and `[[plugins]]` default to empty/absent. The `export.pretty` field SHALL NOT exist (indentation is expressed by `format = "json-pretty"`). Unknown keys SHALL be rejected with an error naming the offending key. Config discovery SHALL resolve identically for `build` and `check`: `--config` if given, else `tablec.toml`/`.tablec.toml` in the process cwd, else `tablec.toml`/`.tablec.toml` in the target directory, else all defaults. Relative values of `data.input_dir`, `export.output_dir`, and `plugins[].path` SHALL resolve against the directory containing the config file (absolute paths pass through); when no config file was found they resolve against the cwd. CLI flags SHALL override config values.

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
