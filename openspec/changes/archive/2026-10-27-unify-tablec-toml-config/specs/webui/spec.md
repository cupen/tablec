## ADDED Requirements

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
