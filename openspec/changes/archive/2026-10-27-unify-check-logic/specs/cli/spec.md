## MODIFIED Requirements

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

#### Scenario: Check honors the selected parser
- **WHEN** `tablec check --parser custom` targets data requiring the `custom` schema parser
- **THEN** the check parses the workbooks with the `custom` parser and reports its diagnostics

#### Scenario: Each project diagnostic is reported once
- **WHEN** the check target contains multiple files and a cross-table violation exists
- **THEN** that violation appears exactly once in the output
