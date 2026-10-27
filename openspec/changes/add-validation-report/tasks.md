# Tasks

## 1. Report module in tablec-core (`src/export/report/`)

- [x] 1.1 `Report` struct + builder from `CheckOutcome` (metadata: `tablec_version`, `generated_at`, `input_dir`, `parser`; summary: `errors`, `warnings`, `files_checked` = distinct non-empty `location.file` across tables and diagnostics, `tables_checked` = table count) with unit tests: counts by severity, distinct-file rule (file parsed + failing counted once), empty outcome, no-files outcome
- [x] 1.2 UTC RFC 3339 formatter with known-vector unit tests (epoch 0, leap day, year boundary, sub-second truncation); no new dependencies
- [x] 1.3 JSON renderer (`serde_json::to_string_pretty`): exact top-level member set for `schema_version: 1`, diagnostics passthrough in pipeline order, summary counters match builder — unit tests include a structure-lock test (member set and diagnostics entry shape) and one asserting a diagnostic's serialized fields equal the core `Diagnostic` serde output used by the webui endpoint
- [x] 1.4 `html_escape` helper with unit tests: `<>&"'`, control characters, already-escaped-looking input (no double-escape requirement beyond correct single-pass output)
- [x] 1.5 HTML renderer: self-contained document (all CSS/JS inline — unit test asserts no `src=`/`href=`/`url(` referencing `http`, `https`, or protocol-relative URLs), summary card (counts, generated time, tool version, parser, input dir), diagnostics grouped file → sheet with severity badges, code, message, location, Chinese UI labels, inline severity-filter script, passed-sheets list, distinct clean-run empty state — unit tests verify grouping order, badge classes, escaped content end-to-end, and the Chinese labels

## 2. CLI flag and wiring (`tablec-cli`)

- [x] 2.1 Repeatable `--report <PATH>` flag on `check`; extension resolver (`.json` → json, `.html` → html, else error naming supported extensions) validated before any file scan — unit tests: both extensions resolve, `.htm`/no-extension/`.xml` rejected with supported-extension message
- [x] 2.2 Report writing on every check exit path (clean, errors-only, warnings-only, no-matching-files, non-existent target), overwrite existing file, one run renders all requested formats from the same `Report`, written paths printed to stdout; write failure (e.g. path under a regular file) exits non-zero naming the path even when the check found no errors — unit tests with tempdirs per scenario
- [x] 2.3 Output and exit codes unchanged when `--report` absent: existing `check` tests stay green plus one explicit regression test asserting no report file is created without the flag

## 3. Integration and e2e (review phase)

- [x] 3.1 Full gates: `cargo fmt` clean, `cargo build`, `cargo test` across workspace, core coverage ≥ 95% per `.claude/rules/how-to.md`
- [x] 3.2 CLI integration test in `tests/`: generate a table set with seeded errors and a clean set, run `check --report` for both formats, assert JSON parses with expected envelope/summary and HTML contains the seeded diagnostics, severity badges, and passed-sheet entries; assert no-report run writes nothing

## Workflow follow-up

- Archive the change after the user accepts the implementation (`openspec archive add-validation-report --yes`).
