# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`tablec` is a Rust-based table compiler for gamedev that processes Excel/CSV/JSON files into structured data formats. It includes both a CLI tool and Python bindings.

## Architecture

- **Core**: Excel parsing and table schema validation (`src/core/`)
- **CLI**: Four commands - build, check, example, and web server (`src/cmd/`)
- **Export**: JSON, MessagePack, and Protobuf formats (`src/export/`)
- **Python**: Maturin-based bindings in `pybinding/` directory

## Build Commands

### Rust CLI
```bash
cargo build --release                    # Build CLI
target/release/tablec --help            # Run CLI help
target/release/tablec build -i input.xlsx -o output.json
target/release/tablec check path/to/files
target/release/tablec example -o example.xlsx -r 10
target/release/tablec web --listen 127.0.0.1:8080
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
- **Export formats**: JSON (default), MessagePack, Protobuf
- **Example generator**: Creates sample Excel files with tablec format
- **Web server**: Basic Actix-web server with hello endpoint
- **Python API**: `build()` and `check()` functions exposed

## Development Setup

1. Install Rust 1.60+ (per README.md)
2. For Python: `pip install maturin`
3. Build: `cargo build --release`
4. Test: `cargo test` or `pytest pybinding/tests/`

## File Structure

- `src/core/table/` - Core table data structures and validation
- `src/export/` - Format-specific exporters
- `pybinding/` - Python extension module
- `proto/` - Protocol buffer definitions

## 开发进度管理
使用 beads 
- bd list                   # 查看当前任务
- bd create --title {标题} --description {详细描述}  # 创建任务

