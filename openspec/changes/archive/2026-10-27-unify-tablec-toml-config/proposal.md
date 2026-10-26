# Proposal: unify-tablec-toml-config

## Why

`tablec.toml` is consumed by three entry points (CLI `build`, CLI `check`, webui) with three conflicting resolution rules and path semantics: CLI `build` ignores the configured `data.input_dir` entirely, `export.pretty` is a dead field no consumer reads, a hand-written config must spell out four required fields before it parses, relative paths drift with the process cwd instead of the config file location, and a typo'd key (`output_dirr`) fails silently. The config layer is the foundation under every entry point; flatten it before adding new capabilities on top.

## What Changes

- **BREAKING** All config sections/fields become optional with serde defaults; an empty `tablec.toml` or one containing only `[parser]` parses successfully.
- **BREAKING** Remove the dead `export.pretty` field; indentation stays expressed by `format = "json-pretty"`.
- **BREAKING** Unify config discovery for `build` and `check`: `--config` > `tablec.toml`/`.tablec.toml` in the process cwd > `tablec.toml` in the target input directory > all defaults. (This reverses `build`'s current input-dir-first discovery.)
- **BREAKING** `build` consumes `data.input_dir`: an explicit `-i` wins; when omitted, input comes from the resolved config.
- **BREAKING** Config-relative paths (`data.input_dir`, `export.output_dir`, `plugins[].path`) resolve relative to the config file's directory; absolute paths pass through. CLI arguments stay relative to the cwd.
- Unknown config keys are rejected with an error naming the key (`deny_unknown_fields`).
- **BREAKING** Zero-config directory mode derives the output as `{output_dir}/{project.name}.{ext}` instead of demanding `-o`; the error case moves to "no matching spreadsheet files".
- CLI `build`/`check` and webui resolve config through one shared resolution layer in `tablec-core` (single source of truth); webui drops its private `input_dir` special-casing.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `cli`: the "Tablec toml config" requirement is rewritten — full defaults, `pretty` removed, unknown keys rejected, path base = config file directory, unified cwd-first discovery; "build subcommand" directory-mode scenarios updated accordingly (input_dir consumption, zero-config output derivation).
- `webui`: adds a config-resolution requirement — the webui uses the same discovery order and path base as the CLI instead of its private resolution.

## Impact

- `tablec-core/src/core/config.rs` — schema rework (defaults, `deny_unknown_fields`, path-resolution helpers, shared discovery function).
- `tablec-cli/src/cmd/build.rs`, `check.rs` — adopt the shared resolution layer; `build` gains `input_dir` consumption and derived output.
- `tablec-webui/src/state.rs`, `handlers.rs` — switch to the shared resolver; remove the `input_dir = "."` override.
- Test fixtures and docs referencing required fields, `pretty`, or discovery order (`AGENTS.md`, `README.md`, `docs/`, existing spec scenarios) need synchronized updates.
- Existing configs that set `pretty` or rely on input-dir-first discovery break (project is early; fixtures are the only known consumers).
- Python bindings are unaffected (they do not go through `Config`).
