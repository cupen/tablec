## 1. Core schema rework (tablec-core/src/core/config.rs)

- [x] 1.1 Rework `Config` and its section structs: `#[serde(default)]` on every section and field, per-struct `#[serde(deny_unknown_fields)]`, remove `export.pretty`, set default `data.input_dir = "."`; add `Default` impls per section; verify unit tests parse an empty TOML and a `[parser]`-only TOML, and that a typo'd key in each of the five sections fails with the key named
- [x] 1.2 Add `ResolvedConfig` (config, config_path, base_dir, input_dir, output_dir, plugin_paths) with `resolve(explicit, target_dir)` implementing `--config` > cwd `tablec.toml`/`.tablec.toml` > target-dir `tablec.toml`/`.tablec.toml` > defaults, joining relative paths against the config file's dir (absolute paths pass through; no file → base is cwd); verify unit tests cover all four precedence tiers and both path polarities per resolved field
- [x] 1.3 Verify `tablec-core` unit-test coverage stays ≥ 95% (`cargo llvm-cov` per `.claude/rules/how-to.md`) for the config module

## 2. CLI build adoption (tablec-cli/src/cmd/build.rs)

- [x] 2.1 Replace build's private discovery (`find_tablec_toml` + `Config::load`) with `ResolvedConfig::resolve`, using the input path's directory as `target_dir` (single-file mode: file's parent); verify `find_tablec_toml` is deleted and unit tests for the old discovery are replaced by cwd-first ones
- [x] 2.2 Consume `input_dir`: `-i` wins, else `ResolvedConfig.input_dir`; directory-mode output resolves as `-o` > `{output_dir}/{project.name}.{ext}` unconditionally; verify unit tests cover bare-build zero-config (writes `output/default.json`), `input_dir` from config, and the no-matching-files error replacing the missing-`-o` error
- [x] 2.3 Update `tablec-cli` tests asserting the old "missing -o" message and input-dir-first discovery; verify `cargo test -p tablec-cli` passes

## 3. CLI check adoption (tablec-cli/src/cmd/check.rs)

- [x] 3.1 Switch `check` to `ResolvedConfig::resolve` with its path argument as `target_dir` (defaults to resolved `input_dir` when omitted); verify unit tests cover cwd-beats-target-dir config selection and unchanged per-file/directory check behavior
- [x] 3.2 Verify `cargo test -p tablec-cli` passes end to end

## 4. Webui adoption (tablec-webui/src/state.rs, handlers.rs)

- [x] 4.1 Switch webui state to `ResolvedConfig::resolve(config_override, --dir)`; delete the `input_dir = "."` default override and private `resolve_input_dir` joining (consumers take `ResolvedConfig.input_dir`); verify state unit tests cover cwd-beats---dir precedence and input_dir anchored to the config file's dir
- [x] 4.2 Surface config load errors at startup with the config path and offending key instead of falling back to defaults; verify the handler/state tests for invalid config expect a loud failure
- [x] 4.3 Run existing webui handler tests and update fixtures that embed required-fields-only or `pretty`-bearing TOML; verify `cargo test -p tablec-webui` passes

## 5. Fixtures, docs, and specs sync

- [x] 5.1 Update embedded TOML fixtures across `tablec-cli`, `tablec-webui`, and `tests/` (drop `pretty`, required-fields-only files now minimal); verify `cargo test` (workspace) is green
- [x] 5.2 Update `AGENTS.md`, `README.md`, `docs/README.zh.md`, and `docs/design.md` to document the unified discovery order, config-relative path base, and defaults; verify doc examples parse as valid minimal configs
- [x] 5.3 Re-read `openspec/specs/cli/spec.md` scenarios affected by this change (config discovery, zero-config build) and confirm behavior matches the delta specs; record any divergence found

## 6. Integration verification

- [x] 6.1 End-to-end smoke from a temp project root: bare `tablec build` writes `output/default.json`; `tablec build` from a foreign cwd with a target-dir config picks the cwd config; `tablec check` uses config `input_dir`; unknown-key config fails with the key named; verify each observed behavior matches the delta specs
