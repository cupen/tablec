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
- **Python API**: `build()` and `check()` functions exposed

## Development Setup

1. Install Rust 1.60+ (per README.md)
2. For Python: `pip install maturin`
3. Build: `cargo build --release`
4. Test: `cargo test` or `pytest binding-python/tests/`

## File Structure

- `src/core/table/` - Core table data structures and validation
- `src/export/` - Format-specific exporters (JSON, MessagePack; protobuf is not implemented)
- `binding-python/` - Python extension module

## 开发进度管理
使用 beads 
- bd list                   # 查看当前任务
- bd create --title {标题} --description {详细描述}  # 创建任务



<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
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

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
