## Context

The current system is a working Rust table compiler (`tablec-core` + `tablec-cli` + `tablec-webui` + `binding-python`). Its behavior is locked by an extensive unit/integration test suite and narrated in `docs/design.md` and the README, but there is no OpenSpec baseline — `openspec/specs/` is empty. This change converts that existing, tested behavior into normative capability specs without touching any code (see proposal.md — Why).

Constraints that shape the design of the spec baseline:

- Rust **edition 2024** locked; `tablec-core` requires unit tests with **coverage ≥ 95%**.
- `docs/design.md` documents the current 5-row layout, constraint layers, and plugin mechanism — but it is design narrative, not a requirements contract.
- The webui is a separate crate with its own endpoints, embedded `dist/`, and a deliberate security boundary (HTTP rejects `plugin_paths`).
- Some capabilities are intentionally incomplete (data validation, `@no_ref` missing) — the baseline records them as-is rather than inventing behavior.

## Goals / Non-Goals

**Goals:**

- Produce one capability spec per stable behavioral area, each requirement with at least one WHEN/THEN scenario that mirrors an existing tested behavior.
- Organize capability paths to match the real module layering so future changes can target stable, discoverable specs.
- Keep every scenario grounded in observable behavior (CLI output, HTTP responses, Python exceptions, exported bytes) and existing tests.

**Non-Goals:**

- No code changes; no behavior changes; no new features.
- Not a rewrite of `docs/design.md` — the design narrative stays where it is.
- No coverage of unimplemented features (`@validator`, `/api/validate` payload semantics, `@no_ref` pluralization) beyond noting them.
- The Rust public API surface (`pub struct`/`pub fn` signatures) is not specified as an API contract — only behavior is.

## Decisions

### D1: Spec capabilities mirror crate/module boundaries
Specs are split into `table-schema`, `constraints`, `compilation`, `diagnostics`, `cli`, `webui`, `schema-parser-plugins`, `python-bindings`.

Rationale: each maps to a coherent set of existing tests and source modules (schema/parse in `tablec-core/src/core/schema` + `parser`; constraints in `tablec-core/src/core/table/constraint.rs`; compilation in `read_excel` + `Project` + `export`; diagnostics in `core/diagnostic.rs`; CLI in `tablec-cli`; webui in `tablec-webui`; plugins in `core/schema/{mod,dynamic}.rs`; bindings in `binding-python`).

Alternatives considered: a single monolithic `core` spec (too coarse — future deltas would churn unrelated requirements) and a per-directory 1:1 spec (too fine — e.g. `export/` and `project/` belong to one "compilation" capability).

### D2: Baseline captures current behavior, including limits
SQL-style semantics (empty-key rows skipped by `@unique`), nullable FK skip in `@ref`, date types stored as strings, the 26-code locked diagnostic set, and the `cli`/`webui` split on cross-table `@ref` are captured exactly as the code behaves today.

Rationale: a baseline must be descriptive, not aspirational; the apply phase validates the specs against the existing test suite. Discrepancies surface as issues, not silent fixes.

### D3: Specs describe behavior, not Rust APIs
No `pub struct` signatures appear in the specs; scenarios are phrased in terms of the observable surface each crate exposes (CLI exit behavior, HTTP status codes and bodies, Python exceptions, exported JSON shape, diagnostics).

Rationale: OpenSpec specs are a behavior contract; the design doc and source remain the API reference. This keeps refactors of internals from churning the specs.

### D4: Known gaps are recorded as non-goals, not requirements
The unimplemented `/api/validate` returns 501 with a `todo` field and is specified as a requirement that it is not implemented; HTTP `plugin_paths` rejection is a requirement. `@no_ref` (referenced in code comments) is not given its own requirement because the public machinery routes it through `@ref`'s validator.

Rationale: surfacing the boundary explicitly prevents a future validate implementation from thinking the baseline made promises it did not; the security boundary is a real, tested behavior that must not be lost in the baseline.

### D5: Spec organization is flat, capability-per-area
No extra domain nesting (`specs/constraints/` rather than `specs/core/constraints/`), matching the repo's existing flat doc layout and the OpenSpec guidance that says not to add a new domain level when the project uses a flat layout.

## Risks / Trade-offs

- [Specs could drift from the narrative `docs/design.md`] → The proposal's Impact section names `openspec/specs/**` as authoritative; future spec edits should update the corresponding README/design sections in the same change.
- [Baseline may encode a behavior that is actually a bug] → The apply phase runs the existing test suite against the scenarios; any failed scenario is investigated as a discrepancy, and the spec (or a bug issue) is updated deliberately — not silently.
- [A capability boundary later proves wrong] → Splitting/merging specs is cheap (archive + new delta); no migration of code is involved because the specs are additive this change.
- [The 26-code diagnostic set is locked by a unit test] → The spec mirrors the code; changing the count later is a deliberate MODIFIED delta plus a test update.

## Migration Plan

No deployment or rollback applies — this change adds planning documents only. The corresponding "migration" for the repo is that any future behavior change must now be proposed as a delta against these baseline specs instead of editing `docs/design.md` first.

## Open Questions

None — the capability split, the flat layout, and the behavior-first phrasing were settled here; remaining unknowns (e.g. whether a future validate feature reuses these specs) are deferrable without changing the baseline, the design, or the tasks.