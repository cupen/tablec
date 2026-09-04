## Why

Data error detection is implemented three times with three different behaviors: CLI `check` parses with `read_excel` (the resolved parser is discarded, so `--parser` and plugins silently do nothing) and validates each table in isolation; CLI `build` also uses parser-less `read_excel` with per-table-only validation; webui `POST /api/check` is parser-aware and runs project-level `validate_project` — but inside the per-file loop, which produces false `@ref` violations for targets in not-yet-read files and duplicates every project diagnostic once per additional file. The webui and CLI must share one check pipeline; today a webui error badge built on `/api/check` would show wrong counts even by its own semantics.

## What Changes

- **New shared check entry point in `tablec-core`**: one routine (enumerate via `find_excel_files` → parse via parser-aware `read_excel_with` → per-table `validate_table` → a single project-level `validate_project` over the complete table set) returns tables plus diagnostics. Project validation runs exactly once, after all files are read.
- CLI `check` delegates to the shared routine. **BREAKING** (behavior-aligning): `--parser`/configured parser/plugins now actually apply, and cross-table `@ref` violations now fail the command (previously skipped; this closes the documented CLI gap).
- WebUI `POST /api/check` delegates to the shared routine. Response shape (`{ diagnostics, duration_ms, sheets_checked }`) is unchanged; diagnostics lose the false positives and duplicates. The "unlike the CLI check" asymmetry in the spec disappears — both run identical semantics.
- CLI `build` is out of scope (keeps parser-less `read_excel` + per-table silent validation); recorded as a follow-up issue.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `cli`: The "check subcommand" requirement changes — validation runs through the shared pipeline with the resolved parser, and cross-table `@ref` project validation is now part of the command's checks (removing the `validate_table`-only mandate).
- `webui`: The "Check endpoint" requirement changes — the endpoint runs the same shared pipeline as CLI `check` (identical parser handling and project validation), removing the "unlike the CLI" divergence wording.

## Impact

- **Code**: `tablec-core/src/core/` (new shared check routine + unit tests), `tablec-cli/src/cmd/check.rs` (delegate), `tablec-webui/src/handlers.rs` (`api_check` delegates; incremental `validate_project` loop removed).
- **APIs**: `POST /api/check` request/response shapes unchanged; CLI flags unchanged. `tablec check` output may show new `@ref` diagnostics and parser-dependent results (**BREAKING** for workflows relying on `@ref` being unchecked or on plugins being ignored).
- **Dependencies**: None added.
- **Related changes**: `webui-compact-file-tree` builds its per-file error badges on `/api/check`; this change should land first (or the badges will surface the false-positive/duplicate diagnostics this change fixes).
- **Non-goals**: Changing CLI `build`'s validation behavior or parser handling (follow-up issue); changing the `Diagnostic` model, severity semantics, or exit-code conventions; touching `/api/validate` (still 501).
