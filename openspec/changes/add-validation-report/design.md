# Design: add-validation-report

## Context

`check_project()` in `tablec-core` already returns `CheckOutcome { tables, diagnostics }`; `Diagnostic` is serde-serializable and its shape is what the webui `/api/check` serves. The CLI (`tablec-cli/src/cmd/check.rs`) renders stderr text and exits by error count. There is no timestamp-formatting dependency in `tablec-core` and no templating crate anywhere. Core test coverage must stay ≥ 95% (`.claude/rules/how-to.md`).

## Goals / Non-Goals

**Goals:**
- One reusable report module in core that both formats render from a single run description, so a future webui-export or Python surface renders identical artifacts.
- Fail-fast flag validation in the CLI (bad extension rejected before any file scan).
- Report writing integrated into every check exit path (clean, errors, warnings-only, no-files, no-target).

**Non-Goals:**
- No webui, Python-binding, or config-format changes; no stdout JSON mode; no report from `tablec build`.
- No report schema beyond version 1; no localization of diagnostic messages (only HTML UI labels are Chinese).

## Decisions

- **Placement: `tablec-core/src/export/report/` (`mod.rs`, `json.rs`, `html.rs`), sibling of the existing `export/json.rs`.** A single `Report` struct (metadata + summary + diagnostics) is built once and consumed by both renderers. Alternatives rejected: CLI-only rendering (blocks webui/Python reuse, the stated future surface); a separate crate (overkill for two renderers over existing types).
- **HTML rendering: hand-rolled writer with a single dedicated `html_escape` helper — no template crate.** All message/code/location content passes through it. Alternatives rejected: askama/maud (add proc-macro compile weight to core for one fixed-structure artifact, and move the markup out of reach of the coverage rule). The filter interaction is a small inline vanilla-JS severity toggle; no framework.
- **`generated_at`: hand-rolled UTC RFC 3339 formatter in the report module** (civil-from-days epoch conversion), pinned by known-vector unit tests. Alternative rejected: adding `time`/`chrono` for one timestamp.
- **`tablec_version` is supplied by the caller**: the CLI passes its own `CARGO_PKG_VERSION` into the `Report` builder; core stays agnostic of which binary is producing the artifact.
- **`files_checked` derivation**: distinct non-empty `location.file` values across the outcome's tables and diagnostics. This is the only file census the pipeline currently carries; a file that both parsed and produced diagnostics is counted once. Pinned by a unit test.
- **JSON is pretty-printed** (`serde_json::to_string_pretty`): CI artifacts stay diffable; minified saves nothing that matters.
- **Report writing sits in front of every early return** in `check.rs` (`!path.exists()` notice and the no-matching-files notice included), so each exit path emits the report of the outcome it produced. The CLI validates all `--report` extensions first, runs the check once, then renders each requested format from the same `Report` value.
- **Failure semantics**: a report write error aborts the command with a non-zero status even when the check itself passed (the user asked for an artifact; silently succeeding without it would be worse). Extension errors fail before scanning; write errors fail after checking. Both print an error naming the offending path.

## Risks / Trade-offs

- [Escaping bug in hand-rolled HTML] → single `html_escape` helper, unit-tested against `<>&"'`, control chars, and non-UTF8-safe input paths; no direct `format!` of diagnostic content anywhere in `html.rs`.
- [`files_checked` undercounts exotic cases (file with no sheets, diagnostic without file)] → derivation rule is documented and pinned by tests; the summary is informational, not a contract about file discovery.
- [Timestamp formatter drift] → known-vector tests (epoch 0, leap-year day, year boundaries) pin UTC correctness.
- [JSON shape drift] → a structure test locks the exact top-level member set for `schema_version: 1`; changes require a version bump per spec.

## Migration Plan

Purely additive: new flag, new module, new tests. No data migration; no behavior change without the flag. Rollback = revert the single commit.

## Open Questions

None — the deferred choice (template crate vs hand-rolled) is resolved above; timestamp and version sources are pinned.
