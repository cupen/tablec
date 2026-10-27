# Spec Delta

## ADDED Requirements

### Requirement: Check report flag
`tablec check` SHALL accept a repeatable `--report <PATH>` flag. Each occurrence SHALL request one validation report written to `PATH`, with its format inferred from the file extension as defined by the validation-report capability. After reports are written successfully, the command output SHALL name each written report path.

#### Scenario: JSON report requested
- **WHEN** `tablec check --report out/report.json` runs on a table set
- **THEN** `out/report.json` exists, is a JSON validation report describing that run, and the command prints the written path

#### Scenario: Both formats requested
- **WHEN** `tablec check --report out/report.json --report out/report.html` runs
- **THEN** both report files are written describing the same run

#### Scenario: Report path printed after write
- **WHEN** one or more reports are written successfully
- **THEN** the command output names each written report path

### Requirement: Check report pre-validation
`tablec check` SHALL validate every requested report path's extension before scanning any table files. A request whose extension is not covered by the validation-report capability SHALL make the command exit non-zero with an error naming the supported extensions, and no report files SHALL be written.

#### Scenario: Unsupported extension fails fast
- **WHEN** `tablec check --report out/report.xml` runs
- **THEN** the command exits non-zero with an error naming the supported extensions and no files are scanned or written

### Requirement: Check behavior unchanged without reports
When `tablec check` runs without `--report`, its human-readable output, exit codes, and file-system effects SHALL be exactly as before this capability existed, and no report file SHALL be written.

#### Scenario: No report without the flag
- **WHEN** `tablec check` runs without `--report`
- **THEN** the command behaves exactly as before: same human-readable output and same exit codes, and no report file is written
