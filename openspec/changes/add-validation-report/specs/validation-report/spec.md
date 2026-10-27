# Spec Delta

## Purpose

Defines the validation report artifact contract for `tablec check`: the JSON and HTML report formats, their content and stability rules, and the write semantics shared by all report formats so any surface (CLI today, webui or bindings later) produces identical artifacts.

## ADDED Requirements

### Requirement: Report format selection by file extension
A requested validation report SHALL have its format inferred from the file extension of the requested path: `.json` produces a JSON report and `.html` produces an HTML report. Any other extension, including `.htm` or a missing extension, SHALL be rejected with an error naming the supported extensions, and the command SHALL fail without producing report files.

#### Scenario: JSON extension selects JSON format
- **WHEN** a report is requested at path `out/report.json`
- **THEN** the written file is a JSON validation report

#### Scenario: HTML extension selects HTML format
- **WHEN** a report is requested at path `out/report.html`
- **THEN** the written file is an HTML validation report

#### Scenario: Unsupported extension is rejected
- **WHEN** a report is requested at path `out/report.htm` or `out/report`
- **THEN** the command fails with an error naming `json` and `html` as the supported extensions and no report file is written

### Requirement: Report writing semantics
Every requested report SHALL be written for each check run regardless of outcome: runs with errors, runs with only warnings, clean runs, and runs that find no matching spreadsheet files. Writing SHALL overwrite an existing file at the requested path. Human-readable check output SHALL be unaffected by report writing. If a report cannot be written, the command SHALL fail with a non-zero exit status and an error identifying the unwritable path, even when the check itself found no errors.

#### Scenario: Clean run still produces a report
- **WHEN** a check run reports no errors and no warnings with a report requested
- **THEN** the report file exists and records zero errors and zero warnings

#### Scenario: Errors do not suppress the report
- **WHEN** a check run reports errors with a report requested
- **THEN** the report file exists, contains those errors, and the exit status remains non-zero

#### Scenario: No-files run produces a report
- **WHEN** the target contains no matching spreadsheet files with a report requested
- **THEN** the report file exists and reflects a run that checked no files

#### Scenario: Overwrite existing report
- **WHEN** the requested report path already contains a file and a check runs
- **THEN** the file is replaced by the report of the new run

#### Scenario: Unwritable report path fails the command
- **WHEN** the requested report path cannot be written
- **THEN** the command exits non-zero with an error naming that path

### Requirement: JSON report envelope
The JSON report SHALL be a single JSON object whose top-level members are exactly: `schema_version` (number), `tablec_version` (string), `generated_at` (RFC 3339 UTC timestamp string), `input_dir` (string), `parser` (string), `summary` (object), and `diagnostics` (array). No other top-level members SHALL appear.

#### Scenario: Envelope members present
- **WHEN** a JSON report is generated from any check run
- **THEN** the object contains `schema_version` equal to 1, `tablec_version`, `generated_at` in RFC 3339 UTC form, `input_dir`, `parser`, `summary`, and `diagnostics`, and no other top-level members

### Requirement: JSON report summary counters
The `summary` object SHALL carry exactly `errors`, `warnings`, `files_checked`, and `tables_checked`. `errors` and `warnings` SHALL count the run's diagnostics by severity; `tables_checked` SHALL count checked sheets and `files_checked` SHALL count checked spreadsheet files.

#### Scenario: Summary counts match the run
- **WHEN** a check run produces diagnostics for several files and sheets
- **THEN** `summary.errors` and `summary.warnings` count the diagnostics by severity, `files_checked` counts the spreadsheet files in scope, and `tables_checked` counts the sheets checked

### Requirement: JSON diagnostics reuse core serialization
Each `diagnostics` entry SHALL use the same serialization as the core diagnostic model, including severity, code, message, and location.

#### Scenario: Diagnostics reuse the core serialization
- **WHEN** a diagnostic appears in the report
- **THEN** its serialized fields match the core diagnostic model's serialization

### Requirement: JSON report stability
The JSON report envelope SHALL carry `schema_version` starting at 1, and any later change that breaks consumers of the envelope structure or the diagnostics array shape SHALL bump `schema_version`.

#### Scenario: Version pins the contract
- **WHEN** a consumer reads a report with `schema_version` equal to 1
- **THEN** the envelope members and diagnostics entry shape are exactly those documented for version 1

### Requirement: Report completeness and order
A report SHALL contain every diagnostic produced by the check run — both errors and warnings — and none that was not produced. Diagnostics SHALL appear in the report in the order the check pipeline produced them.

#### Scenario: All diagnostics present
- **WHEN** a check run produces parse failures, per-table violations, and cross-table violations together
- **THEN** the report contains each of them and only them, in pipeline order

### Requirement: HTML report is self-contained
The HTML report SHALL be a single self-contained file: all styles and scripts inline, no requests to any external resource, so it renders correctly offline in a browser.

#### Scenario: Renders offline
- **WHEN** the HTML report file is opened in a browser with no network access
- **THEN** it renders with styling and filtering functional and issues no external requests

### Requirement: HTML report content and grouping
The HTML report SHALL present: a summary card with error and warning counts, generation time, tool version, parser name, and input directory; the diagnostics grouped by spreadsheet file and then sheet, each entry showing its severity badge, diagnostic code, message, and source location (sheet and line/column when present); and the list of sheets that passed the check. Diagnostic messages SHALL be rendered as produced by the diagnostic model. UI labels SHALL be in Chinese.

#### Scenario: Diagnostics grouped by file and sheet
- **WHEN** the report contains diagnostics from multiple files and sheets
- **THEN** entries are grouped under their file and sheet headings with severity badges visible

#### Scenario: Passed sheets are listed
- **WHEN** the check run validated sheets with no findings
- **THEN** those sheets appear in the report as passed

#### Scenario: Severity filter narrows the view
- **WHEN** the user activates the error-only or warning-only filter
- **THEN** only entries of the selected severity remain visible

### Requirement: Multiple reports per run
A single check run SHALL be able to write several reports. When both a JSON and an HTML report are requested, both SHALL be written and SHALL describe the same run.

#### Scenario: Both formats from one run
- **WHEN** a check runs with both `out/report.json` and `out/report.html` requested
- **THEN** both files exist and their summary counts match each other
