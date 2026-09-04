## 1. Core — shared check pipeline

- [x] 1.1 Add `core::check` (or the module layout `core/` already suggests) with `check_project(input_dir, include, exclude, parser) -> CheckOutcome { tables, diagnostics }`: `find_excel_files` → `read_excel_with` per file → `validate_table` per table → single `validate_project` over the full set; diagnostics ordered parse → per-table → project
- [x] 1.2 Unit tests in `tablec-core`: multi-file `@ref` cross-file fixture validates clean only with the full set; a project violation appears exactly once; parse diagnostics from all files accumulate; a custom parser fixture is honored; empty file set yields the warning diagnostic
- [x] 1.3 `cargo test -p tablec-core` passes with coverage maintained (≥ 95% crate rule)

## 2. CLI — check delegates

- [x] 2.1 Rewrite `tablec-cli/src/cmd/check.rs` to call `check_project`: remove the dead `_parser` store, pass the resolved parser, render per-sheet results + error count, exit non-zero on any error-severity diagnostic; keep the no-files notice + exit-zero contract
- [x] 2.2 CLI tests: clean fixture exits zero; `@ref`-violating fixture reports `ConstraintForeignKeyViolation` once and exits non-zero; `--parser custom` changes parse results on a custom-parser fixture; no-files fixture exits zero
- [x] 2.3 `cargo test --workspace` passes; `cargo fmt` clean

## 3. WebUI — check endpoint delegates

- [x] 3.1 Rewrite `api_check` in `tablec-webui/src/handlers.rs` as a thin adapter: request validation (registry lookup, `plugin_paths` rejection, input-dir 400 with hint) → one `check_project` call → `CheckResponse { diagnostics, duration_ms, sheets_checked }`; delete the incremental `validate_project` loop
- [x] 3.2 Handler tests: response shape unchanged (`sheets_checked` = total tables); cross-file `@ref` violation appears exactly once; missing input dir → 400 with hint; no-files → warning diagnostic in response
- [x] 3.3 `cargo test --workspace` passes; `cargo fmt` clean

## 4. Consistency and follow-ups

- [x] 4.1 Update in-repo docs that describe check behavior divergence — no in-repo doc describes the old divergence (`doc/design.md` does not exist; AGENTS.md makes no `@ref`-skip claim; the divergence wording lived only in the main `cli` spec, updated by this change's delta)
- [x] 4.2 `openspec validate --change unify-check-logic --strict` passes
- [ ] 4.3 File follow-up issues. **Blocked: `bd` embedded-dolt requires CGO (unavailable on this machine) and `gh` CLI not installed — recorded in Follow-ups below; migrate to `bd` when the database is restored**

## Follow-ups (migrate to `bd` when its dolt database is restored)

- **CLI `build` adopts the shared check pipeline** (from 4.3a): build still parses parser-less and validates per-table only; delegating to `core::check::check_project` would make `--parser`/plugins effective and enforce project-level `@ref` — requires a behavior decision because build exit criteria would tighten.
- **Release note for `tablec check`** (from 4.3b): `check` now enforces cross-table `@ref` (previously skipped) and honors `--parser`/`--plugin-path` (previously silently discarded); projects with latent `@ref` violations or non-default parser data will start failing.
