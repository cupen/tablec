## Context

Three hand-rolled pipelines exist today. CLI `check` (`tablec-cli/src/cmd/check.rs`) resolves a parser into `_parser` and never uses it, parsing with parser-less `read_excel`, then calls `validate_table` per table. CLI `build` (`build.rs`) also parses with parser-less `read_excel` and validates per table silently. WebUI `api_check` (`tablec-webui/src/handlers.rs`) parses with `read_excel_with` and calls `validate_project(&tables)` inside the per-file loop over the tables accumulated so far — so `@ref` targets in later files trigger false violations in early iterations, and because `validate_project` re-runs once per file over the growing set, each surviving project diagnostic is emitted once per remaining file. The `constraints` spec already defines project validation (per-table first, then `@ref` over the full set) as the normative model; only the `cli` spec codifies the per-table-only divergence.

## Goals / Non-Goals

**Goals:**
- One check pipeline in `tablec-core`, consumed by both `tablec check` and `POST /api/check`, with identical diagnostics for identical inputs.
- Fix the two webui-only defects (false `@ref` positives from partial file sets; per-file duplication of project diagnostics) as a side effect of the shared semantics.
- Make `--parser`/plugins effective in CLI `check` (today they are silently discarded).

**Non-Goals:**
- CLI `build` keeps its current parser-less parse + per-table silent validation (follow-up issue; unifying it would change build success criteria).
- No changes to the `Diagnostic` model, severity, `DiagnosticCode` set, rendering, or exit-code conventions.
- `/api/validate` stays 501.

## Decisions

- **Home the pipeline in `tablec-core`** (e.g. `core::check`): `check_project(input_dir, include, exclude, parser) -> CheckOutcome { tables: Vec<Table>, diagnostics: Vec<Diagnostic> }` — enumerate with `find_excel_files`, parse each file with `read_excel_with` (parser errors and parse diagnostics accumulated), then `validate_table` per table, then **one** `validate_project` over the complete table set. Alternative (a trait/strategy abstraction over check phases) rejected — there is exactly one semantic today; the function boundary is the reuse contract.
- **Project validation runs once, after all files are read.** This matches the `constraints` spec's normative layering and kills both webui defects by construction. Ordering: parse diagnostics (file order) → per-table diagnostics (table order) → project diagnostics. Consumers that need per-file grouping (the webui rail) group by `location.file` downstream, unchanged.
- **CLI check maps the outcome onto its existing output contract**: per-sheet result lines, total error count, non-zero exit when any diagnostic has `Severity::Error`. The parser lookup moves from dead-store to use; `--plugin-path` flows through `resolve_parser` as before.
- **WebUI `api_check` becomes a thin adapter**: request parsing, registry lookup, input-dir validation (400 with hint), then one call; `CheckResponse` fields keep their meaning (`sheets_checked` = total tables). No frontend change required.
- **"No files found" stays a warning-level diagnostic in the webui response and a notice + exit-zero in the CLI**, preserving each surface's current contract for the empty case (unchanged behavior, now expressed in one place).

## Risks / Trade-offs

- **CLI behavior tightening**: projects with latent `@ref` violations (or data that only parses under a non-default parser) will start failing `tablec check`. This is the point of the change, but it is user-visible; the release note should call it out. Mitigated by the diagnostics' precise file/sheet locations.
- **`sheets_checked` semantics**: previously the webui counter counted tables as accumulated; with the shared routine it is the same total — no expected drift, but the handler tests should pin it.
- **Plugin parity**: CLI resolves plugins from config + `--plugin-path`; the webui still rejects `plugin_paths` over HTTP. Identical semantics therefore only hold when the server's parser override/config matches the CLI invocation — documented, unchanged from today.

## Testing

- Core: unit tests for `check_project` — multi-file fixture where `@ref` crosses files (passes only when all files are in the set), a violation reported exactly once, parse-error accumulation, parser selection honored (custom parser fixture), empty-set warning.
- CLI: integration-style tests — exit codes for clean/violating fixtures, `--parser` effect, `@ref` failure output.
- WebUI: handler tests — response shape unchanged, cross-file `@ref` appears once, missing input dir → 400.
- Full gate: `cargo test --workspace`, `cargo fmt`.
