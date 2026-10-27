# Proposal: add-validation-report

## Why

`tablec check` currently reports findings only as human-readable stderr lines that vanish once the terminal scrolls. In a gamedev pipeline the two real consumers are CI, which needs a machine-readable artifact to store and diff, and 策划 (game designers), who need a shareable file that shows what is broken and where. Neither exists today: the webui already returns JSON diagnostics to its frontend, but nothing in the repo produces a report file.

## What Changes

- Add a repeatable `--report <PATH>` flag to `tablec check`. Report format is inferred from the file extension (`.json` / `.html`); unknown or missing extensions are rejected. The flag may be repeated to emit both formats in one run.
- Add a JSON validation report: a versioned envelope (`schema_version`, `tablec_version`, `generated_at` UTC ISO-8601, `input_dir`, `parser`, `summary`, `diagnostics[]`) whose diagnostics reuse the core `Diagnostic` serde shape used by the webui `/api/check` endpoint. `summary` carries error/warning counts plus `files_checked` (files) and `tables_checked` (sheets).
- Add an HTML validation report: a self-contained single file (inline CSS/JS, zero external requests) with a summary card, diagnostics grouped by file → sheet with severity badges, a severity filter, and the list of tables that passed. UI labels in Chinese; diagnostic messages stay as produced by core.
- Keep all existing human-readable output and exit-code semantics unchanged. The report is an additional artifact: it is written when errors exist (exit code stays non-zero), when the check is clean (a "no issues" report), and in the no-matching-files case. A failure to write the report fails the command with a non-zero exit.
- Move nothing: the check pipeline, diagnostic model, webui, and Python bindings are untouched. Report generation lives in a reusable module so future surfaces (webui export, Python bindings) can adopt it.

## Capabilities

### New Capabilities

- `validation-report`: the validation report artifact contract — JSON envelope structure and stability rules, HTML self-containment and content rules, and the write/failure semantics shared by all report formats.

### Modified Capabilities

- `cli`: the `check` subcommand requirement gains the `--report` flag behavior (repeatable, extension-inferred format, rejection rules) and the boundary cases (report written on error/clean/no-files runs, report write failure fails the command).

## Impact

- `tablec-core`: new report generation module under `src/export/` (JSON and HTML renderers) consuming the existing `CheckOutcome` and `Diagnostic`; `Diagnostic` serde shape is reused as-is.
- `tablec-cli`: `CheckCommand` gains `--report` parsing, extension inference/validation, report writing and reporting of written paths.
- No changes to webui, Python bindings, config format, or the diagnostic model. No new runtime dependencies beyond, if design picks one, a compile-time HTML templating crate.
- Tests: unit tests in `tablec-core` (report renderers, subject to the ≥95% coverage rule) and `tablec-cli` (flag handling, boundary behavior); CLI integration tests via the existing `tests/` suite.
