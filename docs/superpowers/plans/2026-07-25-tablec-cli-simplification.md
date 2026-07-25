# tablec CLI Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop the unused `web` CLI command (and its `actix-web` + `tokio` deps), align `binding-python` JSON defaults with the CLI (minified default + `json-pretty` support), and sync stale references in `CLAUDE.md` and `README.md`.

**Architecture:** Three independent commits. Each task lives in its own crate (`tablec-cli`, `binding-python`, docs) and can be reverted without breaking the others.

**Tech Stack:**
- Rust 1.60+ (per `tablec/README.md`)
- `cargo` workspace at `/home/bot/workbench/repos/tablec/`
- Python 3.10+ with `maturin` for `binding-python` testing
- Existing crates: `tablec-cli`, `binding-python`

## Global Constraints

- Spec source: `docs/superpowers/specs/2026-07-25-tablec-cli-simplification-design.md` (commit `b12daa3` on `feat/cli-simplification`).
- All work happens on branch `feat/cli-simplification` from main `da4f5b5`. **Do not touch main directly.**
- Per-repo SSH for `tablec` is `core.sshCommand = ssh -i ~/.ssh/id_ed25519 -o IdentitiesOnly=yes` in `.git/config`. Pre-configured.
- Per-repo git author identity is `Claude <claude@anthropic.com>` (local override). Do NOT change.
- Public API of `tablec-core` MUST NOT change. Tests are not the public API surface, but additions are fine.
- Do NOT add new dependencies this round. We're removing deps (Task 1), aligning defaults (Task 2), editing docs (Task 3).
- Each task's final commit must leave green: `cargo build -p tablec-cli -p tablec-core -p binding-python` and `cargo test -p tablec-core --tests`.
- Task 2 also needs `binding-python` pytest to pass.

## File Structure

### Files to delete
- `tablec-cli/src/cmd/web.rs` — Task 1

### Files to modify
- `tablec-cli/src/cmd/mod.rs` — Task 1 (remove `pub mod web;`)
- `tablec-cli/src/cli.rs` — Task 1 (remove `WebCommand` import + `Web` variant)
- `tablec-cli/src/main.rs` — Task 1 (remove `Command::Web` arm + simplify main to sync)
- `tablec-cli/Cargo.toml` — Task 1 (remove `actix-web` and `tokio` deps)
- `binding-python/src/lib.rs` — Task 2 (json defaults + add `json-pretty`)
- `binding-python/tests/test_python_binding.py` — Task 2 (add 2 tests)
- `CLAUDE.md` — Task 3
- `README.md` — Task 3

### Files to create
- `binding-python/tests/fixtures/minimal.xlsx` — Task 2 (small fixture or generated in conftest; see Step 6 below)

---

## Task 1: Remove `web` command and its dependencies

**Files:**
- Delete: `tablec-cli/src/cmd/web.rs`
- Modify: `tablec-cli/src/cmd/mod.rs` (delete `pub mod web;` on line 3)
- Modify: `tablec-cli/src/cli.rs` (delete `pub use crate::cmd::web::WebCommand;` on line 5, delete `Web(WebCommand),` variant on line 25)
- Modify: `tablec-cli/src/main.rs` (delete `Command::Web(c) => { c.run().await?; }` and convert main to sync)
- Modify: `tablec-cli/Cargo.toml` (delete `actix-web` and `tokio` deps)

**Interfaces:**
- Produces: smaller `tablec-cli` crate (no actix-web, no tokio). CLI exposes only `build | check | example`.
- Consumes: nothing new.

- [ ] **Step 1: Baseline build/test**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  cargo build -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-core --tests 2>&1 | tail -10
```
Expected: green baseline. Note the build time / artifact size for comparison.

- [ ] **Step 2: Confirm no callers exist outside the 3 expected files**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  grep -rnE 'web::|WebCommand|cmd::web|actix' \
    tablec-cli/src tablec-cli/Cargo.toml 2>&1
```
Expected: only hits inside `tablec-cli/src/cmd/web.rs`, `tablec-cli/src/cmd/mod.rs`, `tablec-cli/src/cli.rs`, `tablec-cli/src/main.rs`, `tablec-cli/Cargo.toml`. **No other crate references.**

- [ ] **Step 3: Delete `web.rs`**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  git rm tablec-cli/src/cmd/web.rs
```

- [ ] **Step 4: Update `cmd/mod.rs`**

Edit `tablec-cli/src/cmd/mod.rs`. Delete the `pub mod web;` line. The file becomes:

```rust
pub mod build;
pub mod check;
pub mod example;
```

- [ ] **Step 5: Update `cli.rs`**

Edit `tablec-cli/src/cli.rs`. Two changes:

a) Delete the line `pub use crate::cmd::web::WebCommand;` (currently line 5).

b) Delete the entire `Web` variant from the `Command` enum (currently lines 23-25):
```rust
    /// Start a web server
    Web(WebCommand),
```

The resulting `Command` enum:
```rust
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build data from Excel files
    Build(BuildCommand),
    /// Check Excel files for errors
    Check(CheckCommand),
    /// Create an example Excel file
    Example(ExampleCommand),
}
```

- [ ] **Step 6: Update `main.rs` to be synchronous**

Read `tablec-cli/src/main.rs` first. The current file should look like:

```rust
use tablec_cli::cli;
use cli::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::parse_args();
    match args.command {
        Command::Build(c) => {
            c.run()?;
        }
        Command::Check(c) => {
            c.run()?;
        }
        Command::Web(c) => {
            c.run().await?;
        }
        Command::Example(c) => {
            c.run()?;
        }
    }
    Ok(())
}
```

Replace with:

```rust
use tablec_cli::cli;
use cli::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::parse_args();
    match args.command {
        Command::Build(c) => {
            c.run()?;
        }
        Command::Check(c) => {
            c.run()?;
        }
        Command::Example(c) => {
            c.run()?;
        }
    }
    Ok(())
}
```

The `#[tokio::main]` attribute is removed, the function becomes sync, the `Web` arm is gone.

- [ ] **Step 7: Update `Cargo.toml`**

Edit `tablec-cli/Cargo.toml`. Delete the two lines:

```toml
actix-web = "4.0"
tokio = { version = "1", features = ["full"] }
```

The `[dependencies]` section should now read:

```toml
[dependencies]
tablec-core = { path = "../tablec-core" }
clap = { version = "4.5.13", features = ["derive"] }
tempfile = "3.10.1"
rust_xlsxwriter = "0.90.1"
calamine = "0.25.0"
rand = "0.9.2"
glob = "0.3"
```

- [ ] **Step 8: Build, test, and CLI smoke**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  cargo build -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-core --tests 2>&1 | tail -10
```
Expected: green. Build should be visibly faster than Step 1.

Then smoke-test the CLI:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  cargo run -p tablec-cli -- --help 2>&1 | tail -15
```
Expected: shows only `Build`, `Check`, `Example` subcommands. No `Web`.

Also verify the binary still launches:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  cargo run -p tablec-cli -- example --help 2>&1 | tail -10
```
Expected: example subcommand help renders correctly.

- [ ] **Step 9: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  git add tablec-cli/src/cmd/web.rs tablec-cli/src/cmd/mod.rs tablec-cli/src/cli.rs tablec-cli/src/main.rs tablec-cli/Cargo.toml && \
  git commit -m "chore(cli): remove web command and its dependencies

The web command (actix-web HttpServer serving a hello + health endpoint)
has no callers in any production path; it pulls actix-web and tokio
(full features) into every build for a feature that does not fit the
project's 'table compiler for game data' scope. Drop the command, the
module, the dep footprint, and the async main wrapper.

Per spec 2026-07-25 §2. CLI surface is now build | check | example."
```

---

## Task 2: Sync binding-python with CLI JSON defaults

**Files:**
- Modify: `binding-python/src/lib.rs` (json defaults + add `json-pretty`)
- Modify: `binding-python/tests/test_python_binding.py` (add 2 tests)
- Create: `binding-python/tests/fixtures/minimal.xlsx` OR use `openpyxl` in conftest

**Interfaces:**
- Produces:
  - `build(input, output, "json")` writes minified JSON (matches CLI default)
  - `build(input, output, "json-pretty")` writes indented JSON (new)
  - `build(input, output, "msgpack")` unchanged
  - Format error message: `"Unsupported format '<x>'. Use one of: json, json-pretty, msgpack."`
- Consumes: nothing new.

- [ ] **Step 1: Baseline**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification/binding-python && \
  cargo build 2>&1 | tail -5
```
Expected: green.

If `maturin` is installed and pytest is reachable, also run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification/binding-python && \
  pytest -v 2>&1 | tail -15
```
Expected: existing tests pass. If pytest setup isn't working locally, skip — but Task 2's commit message should record the test outcome.

- [ ] **Step 2: Read existing `test_python_binding.py` to understand the pattern**

Read `binding-python/tests/test_python_binding.py`. Identify:
- How the existing test loads a fixture (conftest or inline)
- How `tablec.build(...)` is currently invoked
- How the output is verified

If existing tests don't use a fixture file but generate xlsx inline (e.g. via `openpyxl`), follow that pattern. Otherwise add a `tests/fixtures/minimal.xlsx` as a committed binary.

- [ ] **Step 3: Write the failing tests (TDD)**

Append to `binding-python/tests/test_python_binding.py`:

```python
def test_build_json_is_minified_by_default(tmp_path):
    """`json` format produces single-line minified output (matches CLI default)."""
    # Use whatever fixture-loading pattern Step 2 identified.
    # Example shape (replace with actual fixture):
    src = Path(__file__).parent / "fixtures" / "minimal.xlsx"
    dst = tmp_path / "out.json"
    tablec.build(str(src), str(dst), "json")
    text = dst.read_text()
    assert "\n" not in text, f"minified JSON should have no newlines, got: {text!r}"
    # It is still valid JSON
    json.loads(text)


def test_build_json_pretty_has_indentation(tmp_path):
    """`json-pretty` format produces multi-line indented output."""
    src = Path(__file__).parent / "fixtures" / "minimal.xlsx"
    dst = tmp_path / "out_pretty.json"
    tablec.build(str(src), str(dst), "json-pretty")
    text = dst.read_text()
    assert text.count("\n") >= 2, f"pretty JSON should span multiple lines, got: {text!r}"
    # Spot-check indentation: at least one line starts with 4 spaces.
    assert any(line.startswith("    ") for line in text.splitlines()), \
        f"pretty JSON should contain indented lines, got: {text!r}"
    json.loads(text)
```

If `Path` and `json` are not imported at the top of the test file, add them.

- [ ] **Step 4: Run new tests, confirm both FAIL**

```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification/binding-python && \
  pytest -v tests/test_python_binding.py::test_build_json_is_minified_by_default \
         tests/test_python_binding.py::test_build_json_pretty_has_indentation 2>&1 | tail -20
```
Expected: both FAIL. `test_build_json_pretty_has_indentation` will fail with "Unsupported format 'json-pretty'" (the binding doesn't yet accept it). `test_build_json_is_minified_by_default` will fail because current impl writes pretty=true so the output contains newlines.

If pytest can't run because the binding isn't built, document this in the report and move to Step 5 anyway (will be verified at Step 8).

- [ ] **Step 5: Update `binding-python/src/lib.rs`**

Edit `binding-python/src/lib.rs`. Replace the `match format { ... }` block (currently lines 30-44) with:

```rust
    let bytes: Vec<u8> = match format {
        "json" => Json { pretty: false, include_fields: false }
            .to_vec(&project)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        "json-pretty" => Json { pretty: true, include_fields: false }
            .to_vec(&project)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        "msgpack" => Msgpack
            .to_vec(&project)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
                other
            )));
        }
    };
    std::fs::write(output, bytes)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
```

Then replace the original three `std::fs::write(output, bytes)` lines that appeared inside each match arm (lines 34, 39, 44 originally) — they should now appear once after the match.

Verify the final shape of the `build` function:
- Top: `read_excel_or_pyerr(input)?` (unchanged)
- Construct `Project` (unchanged)
- The single `match format { ... bytes: Vec<u8> = ... }` shown above
- Single `std::fs::write(output, bytes)` after the match
- `Ok(())` at end

- [ ] **Step 6: Build the binding**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification/binding-python && \
  cargo build 2>&1 | tail -5
```
Expected: green.

- [ ] **Step 7: Run tests, confirm pass**

If pytest is runnable:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification/binding-python && \
  pytest -v 2>&1 | tail -20
```
Expected: all tests pass, including the 2 new ones.

If pytest isn't runnable locally, document this fact in the report — the CI runs pytest on every PR; this is a documented deviation only if local pytest cannot be invoked.

- [ ] **Step 8: Confirm full repo still builds**

```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  cargo build -p tablec-cli -p tablec-core -p binding-python 2>&1 | tail -5 && \
  cargo test -p tablec-core --tests 2>&1 | tail -10
```
Expected: green.

- [ ] **Step 9: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  git add binding-python/src/lib.rs binding-python/tests/test_python_binding.py binding-python/tests/fixtures/minimal.xlsx 2>&1 && \
  git commit -m "feat(binding-python): align json defaults with CLI

The CLI's 'json' format produces minified output (commit 7b57636); the
Python binding was still writing pretty-printed JSON. This commit:

- default 'json' to pretty=false (minified)
- add new format 'json-pretty' for indented output
- extend format error message to list all valid options

Tests added in test_python_binding.py cover both formats."
```

(If you didn't create a fixture file, omit it from `git add`.)

---

## Task 3: Sync CLAUDE.md and README.md with current state

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Interfaces:**
- Produces: docs that reflect current code state
- Consumes: nothing new

- [ ] **Step 1: Baseline grep — capture all stale references**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  grep -nE 'actix|web server|proto|pybinding' CLAUDE.md README.md 2>&1
```
Expected: shows the current stale references (will become the patch list).

- [ ] **Step 2: Update `CLAUDE.md`**

Edit `CLAUDE.md`. Make the following replacements (line numbers approximate; match by content):

a) Architecture / CLI line, change:
```
- **CLI**: Four commands - build, check, example, and web server (`src/cmd/`)
```
to:
```
- **CLI**: Three commands - build, check, example (`src/cmd/`)
```

b) Architecture / Python line, change:
```
- **Python**: Maturin-based bindings in `pybinding/` directory
```
to:
```
- **Python**: Maturin-based bindings in `binding-python/`
```

c) Key Components, delete the entire "Web server" line:
```
- **Web server**: Basic Actix-web server with hello endpoint
```

d) Build Commands, delete the `tablec web` line:
```
tablec web --listen 127.0.0.1:8080
```

e) File Structure, change:
```
- `src/core/table/` - Core table data structures and validation
- `src/export/` - Format-specific exporters
- `pybinding/` - Python extension module
- `proto/` - Protocol buffer definitions
```
to:
```
- `src/core/table/` - Core table data structures and validation
- `src/export/` - Format-specific exporters (JSON, MessagePack; protobuf is not implemented)
- `binding-python/` - Python extension module
```

- [ ] **Step 3: Update `README.md`**

Edit `README.md`. Make these replacements:

a) Features list, find the "CLI Tool" bullet. If it mentions "web server", remove it:
```
- **CLI Tool**: Build, check, example, and web server commands
```
→
```
- **CLI Tool**: Build, check, example commands
```

b) Quick Start section, delete the `tablec web --listen 127.0.0.1:8080` example entirely (the heading "## Start Web Server" and its block, if present).

- [ ] **Step 4: Verify no stale references remain**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  grep -nE 'actix|web server|proto|pybinding' CLAUDE.md README.md 2>&1
```
Expected: 0 matches (the "开发进度管理" / "Use bd" sections don't contain any of these tokens).

- [ ] **Step 5: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec/.worktrees/feat-cli-simplification && \
  git add CLAUDE.md README.md && \
  git commit -m "doc: sync CLAUDE.md and README.md with current state

Removes references to the now-deleted web command (actix-web server),
the never-implemented proto/ directory, and the wrong pybinding/ path
(real path is binding-python/). No code changes.

Per spec 2026-07-25 §4."
```

---

## Self-Review

**Spec coverage:**
- Spec §2 → Task 1 ✓
- Spec §3 → Task 2 ✓
- Spec §4 → Task 3 ✓
- Spec §5 (out of scope) → no task touches those ✓
- Spec §6 (rhythm) → 4 commits total (spec + 3 impl); each task maps to one ✓

**Placeholder scan:** No "TBD" / "TODO" / "implement later" markers. All Step code is concrete. Step 2 in Task 2 references "Step 2 identified" without re-pasting the pattern; that's by design (the engineer should read the file once and apply the pattern).

**Type consistency:** `Json { pretty: false, ... }` and `Json { pretty: true, ... }` are the only forms used; matches `binding-python/src/lib.rs` existing usage. Format strings `"json" | "json-pretty" | "msgpack"` consistent with CLI's `build.rs`.

**Risk to flag:** Task 1 Step 6 silently assumes `main.rs` matches the listed body. If a parallel commit has already restructured it, the engineer must adapt — but the spec and CLAUDE.md both point to the current state.