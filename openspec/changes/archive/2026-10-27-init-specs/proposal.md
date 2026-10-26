## Why

tablec is a functioning Rust-based table compiler (Excel/CSV/JSON → JSON/MessagePack) with a CLI, a webui, and Python bindings, but its behavior is only documented in prose (`docs/design.md`, README) and locked by unit tests. The OpenSpec root exists (`openspec/config.yaml`) with no specs, so there is no single source of truth for what the system must do, none of the behaviors are captured as testable requirements, and future changes cannot be planned as deltas against established capability specs. We are initializing the spec baseline from the existing, working implementation.

## What Changes

- Scaffold OpenSpec specs for the existing system as a **baseline** — each capability is captured as requirements with WHEN/THEN scenarios that reflect current behavior.
- Define capability boundaries that mirror the real module layering, so future changes map to a stable structure.
- Record known gaps/limitations (not-yet-implemented `@validator`/`/api/validate`, HTTP rejects `plugin_paths`, CLI `check` skips cross-table `@ref`) in the specs as explicit non-goals / open items — not as new features.
- No production code, no API, and no behavior changes. This change is documentation-only; `apply` will only add spec files under `openspec/specs/`.

## Capabilities

### New Capabilities

- `table-schema`: The 5-row Excel header layout, field types (`int8`…`string`, arrays, maps, structs), field metadata, tags, and the default-not-null schema semantics.
- `constraints`: The `@func(...)` constraint grammar, the named constraints (`@nullable`, `@range`, `@oneof`, `@maxlen`, `@pattern`, `@unique`, `@seq`, `@order`, `@ref`), layer semantics, and how violations map to `DiagnosticCode`s.
- `compilation`: Parsing Excel workbooks into typed `Table`/`Row`/`Value` data, the `Project` with `Meta` (version/hash/build_at/source/tool), and export to JSON / JSON-pretty / MessagePack.
- `diagnostics`: The `Diagnostic` model (severity, code, message, source location) and how errors are rendered across CLI, webui, and Python bindings.
- `cli`: The `build | check | example | webui` subcommands and their observable behavior (input resolution, config auto-discovery, format handling, exit codes, stdout/stderr output).
- `webui`: The HTTP backend behavior — endpoints (`/api/health`, `/api/state`, `/api/files`, `/api/sheets`, `/api/preview`, `/api/parsed_preview`, `/api/build`, `/api/check`, `/api/validate`), embedded static assets, and the security boundary that rejects `plugin_paths` from HTTP.
- `schema-parser-plugins`: The `SchemaParser` extension contract — static registration via registry, dynamic cdylib loading via `tablec_plugin_create_v1`/`tablec_plugin_drop_v1`, dependency on matching host/plugin Rust toolchains, and the CLI/config selection precedence.
- `python-bindings`: The PyO3 `tablec.build()` / `tablec.check()` API names, signatures, supported formats, error mapping, and the pure-Python `tablec` package surface.

### Modified Capabilities

- None — no existing specs to modify.

## Impact

- **Code**: None changed. Only `openspec/` planning artifacts are added.
- **APIs**: None — CLI flags, HTTP endpoints, Python API, and Rust public API are unchanged (documented as-is).
- **Dependencies**: None added.
- **Docs**: `openspec/specs/**` becomes the normative behavior reference; `docs/design.md` remains the design narrative and is out of scope here.
- **Note**: Duplicate spec sources may drift — future spec edits should treat `openspec/specs/**` as authoritative and update `docs/design.md`/README when they touch the same behaviors.