# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`tablec` is a Rust-based table compiler for gamedev that processes Excel/CSV/JSON files into structured data formats. It includes both a CLI tool and Python bindings.

## Architecture

- **Core**: Excel parsing and table schema validation (`src/core/`)
- **CLI**: Three commands - build, check, example (`src/cmd/`)
- **Export**: JSON and MessagePack formats (`src/export/`)
- **Python**: Maturin-based bindings in `binding-python/`

## Build Commands

### Rust CLI
```bash
cargo build --release                    # Build CLI
target/release/tablec --help            # Run CLI help
target/release/tablec build -i input.xlsx -o output.json
target/release/tablec check path/to/files
target/release/tablec example -o example.xlsx -r 10
target/release/tablec build [path]                # build a directory (defaults to cwd); auto-discovers tablec.toml
```

### Python Bindings
python 开发环境可以用 uv 创建 venv
```bash
cd binding-python
maturin develop                         # Build Python bindings
python -c "import tablec; tablec.check('file.xlsx')"
pytest                                  # Run Python tests
```

## Key Components

- **Excel parser**: Uses `calamine` crate for Excel parsing
- **Table schema**: Defines constraints and validation rules
- **Export formats**: JSON (default), MessagePack (Protobuf is not implemented)
- **Example generator**: Creates sample Excel files with tablec format
- **Python API**: `build()` and `check()` functions exposed; `build()` accepts `json` (minified) / `json-pretty` (indented) / `msgpack`

## Development Setup

1. Install Rust 1.60+ (per README.md)
2. For Python: `pip install maturin`
3. Build: `cargo build --release`
4. Test: `cargo test` or `pytest binding-python/tests/`
5. Enable `cargo fmt` pre-commit hook once: `git config core.hooksPath .githooks`

## File Structure

- `src/core/table/` - Core table data structures and validation
- `src/export/` - Format-specific exporters (JSON, MessagePack; protobuf is not implemented)
- `binding-python/` - Python extension module

## 开发进度管理
使用 beads 
- bd list                   # 查看当前任务
- bd create --title {标题} --description {详细描述}  # 创建任务



<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
