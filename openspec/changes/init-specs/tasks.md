## 1. Baseline Triggers — Verify No Code Change Is Needed

- [ ] 1.1 Confirm no production code is modified by this change; verify `openspec verify` (or the change's own validate) reports planning-only artifacts
- [ ] 1.2 Run `cargo test --workspace` (or `cargo test -p tablec-core`) and record the suite passes, so the baseline matches current behavior
- [ ] 1.3 Run `openspec validate --change init-specs` (or the repo's validate command) and confirm the change valid (no missing deltas, no zero-delta rejection)

## 2. Spec Baseline Review — Specs Reflect Identified Behavior

- [ ] 2.1 Review `specs/table-schema/spec.md` against `tablec-core/src/core/schema/mod.rs`, `core/table/table.rs`, `core/parser/field.rs`, and `core/table/field.rs`; confirm every requirement maps to an existing test (5-row layout, tags, fallback, default-not-null, date-as-string, hash-skip)
- [ ] 2.2 Review `specs/constraints/spec.md` against `core/table/constraint.rs` and `docs/design.md`; confirm grammar, all nine named constraints, layer execution, `@ref` skip semantics, and diagnostic-code mapping match the code and existing tests
- [ ] 2.3 Review `specs/compilation/spec.md` against `core/parser/value_parser.rs`, `core/project/project.rs`, `core/project/meta.rs`, and `export/*`; confirm value parsing failures, project hash, row key order, and `include_fields` gating match existing tests (`json.rs` T1–T5, `meta.rs`)
- [ ] 2.4 Review `specs/diagnostics/spec.md` against `core/diagnostic.rs`; confirm the 26-code set, display rules (sheet block, line:col block, file omitted), and aggregate validation match the code and its tests
- [ ] 2.5 Review `specs/cli/spec.md` against `tablec-cli/src/cmd/*.rs` and `parser_resolve.rs`; confirm build/check/example behavior, config discovery, extension mapping, and error rendering match existing CLI tests
- [ ] 2.6 Review `specs/webui/spec.md` against `tablec-webui/src/handlers.rs` and `router.rs`; confirm every endpoint, the `plugin_paths` rejection, `/api/validate` 501, and cache-control contract match handler tests
- [ ] 2.7 Review `specs/schema-parser-plugins/spec.md` against `core/schema/{mod,dynamic}.rs`; confirm static/dynamic registration, ABI symbols, toolchain note, and panic containment match the tests in those files
- [ ] 2.8 Review `specs/python-bindings/spec.md` against `binding-python/src/lib.rs` and `binding-python/tablec/__init__.py`; confirm `build`/`check` signatures, format whitelist, parser resolution, and the not-compiled fallback match the code and tests

## 3. Spec Hygiene — Validation Enforcement

- [ ] 3.1 Confirm every requirement contains at least one `#### Scenario:` with WHEN/THEN (4-hashtag scenarios exactly); fix any requirement lacking a scenario
- [ ] 3.2 Run `openspec validate --change init-specs --strict` and confirm it passes (no too-brief `## Purpose`, no 3-hashtag scenarios, no headers mismatched)
- [ ] 3.3 Verify each capability path uses kebab-case and no nested domain level was added beyond the flat layout
- [ ] 3.4 Confirm the proposal's New Capabilities list matches exactly the spec files under `specs/` (no extra, no missing)

## 4. Finishing

- [ ] 4.1 Optionally sync the spec baseline into the main specs via `/openspec-sync-specs` (or defer; document status in the session)
- [ ] 4.2 Summarize the baseline: 8 capability specs covering schema/parse, constraints, compilation, diagnostics, CLI, webui, plugins, and Python bindings, ready for future delta changes