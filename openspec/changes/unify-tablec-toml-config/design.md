# Design: unify-tablec-toml-config

## Context

`Config` lives in `tablec-core/src/core/config.rs` as a serde/TOML struct with four required fields (`project.name`, `data.input_dir`, `export.format`, `export.output_dir`) and one dead field (`export.pretty`). Three consumers each roll their own resolution:

- CLI `build` (`tablec-cli/src/cmd/build.rs`): `--config` > `tablec.toml` in the **input directory**; ignores `cfg.data.input_dir` (uses `-i` or cwd); derives output as `{output_dir}/{project.name}.{ext}` only when a config was found, else demands `-o`.
- CLI `check` (`tablec-cli/src/cmd/check.rs`): `--config` > `tablec.toml` in the **cwd**; consumes `input_dir` when no path arg.
- webui (`tablec-webui/src/state.rs`): `--config` > `tablec.toml` in `--dir`; resolves `input_dir` against `--dir`; overrides default `input_dir` to `"."`.

Relative config paths are joined against the process cwd everywhere, so behavior drifts with where the command is run from. See proposal.md for the motivation.

## Goals / Non-Goals

**Goals:**

- One config schema and one resolution function in `tablec-core` that all three entry points call.
- Minimal-config ergonomics: every field defaults; strict rejection of unknown keys.
- Config-relative paths anchored to the config file so runs are cwd-independent.

**Non-Goals:**

- Per-table / split output files, output name templating, new formats.
- `@validator` config (unimplemented feature, separate change).
- Python bindings (do not go through `Config`).
- Changing glob semantics of `include`/`exclude` (patterns stay relative to the resolved input dir, `**` recurses, exclude wins).

## Decisions

### D1. Schema: all-optional fields, `#[serde(default)]`, per-struct `deny_unknown_fields`

Every section and field gets `#[serde(default)]` so an empty or partial `tablec.toml` parses. Defaults (via `Default` impls on each section struct): `project.name = "default"`, `data.input_dir = "."`, `data.include = ["*.xlsx"]`, `export.format = "json"`, `export.output_dir = "output"`, `export.include_fields = false`.

`export.pretty` is **removed** — indentation stays encoded in `format = "json-pretty"`. (Alternative rejected: reviving `pretty` as an orthogonal `json|msgpack × pretty` switch — more orthogonal on paper, but it rewrites every existing `json-pretty` usage across CLI/webui/tests for no new capability, exceeding the repair scope.)

Unknown keys are rejected by `#[serde(deny_unknown_fields)]`. Note serde caveat: the attribute does **not** propagate to nested structs, so it must be applied to `ProjectConfig`, `DataConfig`, `ExportConfig`, `ParserConfig`, and `PluginConfig` individually — a single test asserting a typo in each section fails loudly guards against regressions.

### D2. Two-layer shape: raw `Config` (pure data) + `ResolvedConfig` (absolute paths)

`Config` stays a pure deserialization struct; a new `ResolvedConfig` wraps it with the resolution results:

```rust
struct ResolvedConfig {
    config: Config,          // raw values as written
    config_path: Option<PathBuf>, // None = all defaults
    base_dir: PathBuf,       // config file's dir, or cwd when no file
    input_dir: PathBuf,      // base_dir + data.input_dir  (absolute)
    output_dir: PathBuf,     // base_dir + export.output_dir (absolute)
    plugin_paths: Vec<PathBuf>, // base_dir + each plugins[].path
}
```

Consumers never join paths themselves; they take `input_dir`/`output_dir`/`plugin_paths` from `ResolvedConfig`. Absolute config values pass through unchanged. (Alternative rejected: mutating `Config` fields in place after load — loses the as-written values and makes serialization round-trips lie.)

### D3. One discovery function, cwd-first

`tablec-core` exposes a single resolution entry, e.g. `ResolvedConfig::resolve(explicit: Option<&Path>, target_dir: &Path)`, implementing: `--config` > `tablec.toml`/`.tablec.toml` in cwd > `tablec.toml`/`.tablec.toml` in `target_dir` > defaults (base_dir = cwd).

`build` and `check` pass their input/target path as `target_dir`; webui passes `--dir`. This **reverses** build's current input-dir-first discovery (user decision: "run from project root" convention wins; the alternative — target-dir-first — was rejected because the same command run from a different cwd resolving a different config is the more surprising failure mode).

Single-file `build` mode: the file's parent directory is `target_dir`, matching directory-mode behavior; `-o` remains required for single-file input.

### D4. `build` consumes `input_dir`; zero-config output derivation

Resolution order for build input: `-i` wins (relative to cwd, as any CLI arg) → else `ResolvedConfig.input_dir`. Directory-mode output: `-o` wins → else `{output_dir}/{project.name}.{ext}` — unconditionally, defaults included, so bare `tablec build` scans the cwd and writes `output/default.json`. The former "missing -o" error is replaced by the existing "no matching spreadsheet files" error.

### D5. Assumptions pending review (not user-ratified; veto at review flips the noted spec scenario)

- **A. `data.input_dir` default `"."`** (not the current `Config::default`'s `"data"`): bare `tablec build` keeps scanning the cwd exactly as today, and webui's `input_dir = "."` special-case override becomes deletable. Flipping to `"data"` changes the "Zero-config directory build" scenario to scan `./data`.
- **B. Zero-config output derivation allowed** (no `-o` requirement): implements "minimal config runs" fully; the cost is that bare `tablec build` silently creates `output/`. The stderr progress line already names the resolved output path. Rejecting this restores the old error and removes the "Zero-config directory build derives output" scenario.
- C. Glob semantics of `include`/`exclude` are unchanged (only the base against which patterns join changes — from cwd to the resolved absolute `input_dir`).
- D. `parser.name` unset falls back to `"standard"` (current behavior, now also the schema default).

## Risks / Trade-offs

- [Breaking change to every existing `tablec.toml` that sets `pretty`] → one-pass fixture/test update; early project, no shim.
- [`deny_unknown_fields` rejects forward-declared keys users preemptively write] → acceptable: typos are the more common failure; version pinning of the config schema is out of scope.
- [Bare `build` now writes `output/` where it previously errored] → stderr prints the resolved output path before writing; documented in proposal as BREAKING.
- [cwd-first discovery makes `build <dir>` results depend on cwd when both directories hold a config] → inherent to the chosen precedence; spec scenario pins it so the behavior is at least deliberate.
- [webui startup now fails loudly on invalid config where it previously started with defaults] → deliberate (spec scenario); error message names the config path and key.

## Migration Plan

Single release, no compat shim: land schema + resolver + consumers + fixture/doc updates in one change (tasks.md sequences it). Rollback = revert the commit; no persistent state involved.

## Open Questions

None — the two genuine unknowns (assumptions A and B in D5) are decisions with recorded veto points, not deferrable unknowns.
