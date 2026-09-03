# Tablec

[English](README.md) | [简体中文](docs/README.zh.md)

A **Table Compiler** for game development - compiles Excel/CSV/JSON table data into program-readable formats (JSON, MessagePack).

## Features

- **Table Schema**: Database-Like schema with field types and constraints
- **Blazing Fast**: Optimized Rust implementation
- **Rich Type System**: Support for basic types, array, map, and struct(aka custom type)
- **Multiple Export Formats**: JSON, MessagePack
- **CLI Tool**: Build, check, example commands
- **Python Bindings**: Native Python API via PyO3

## Installation

Requires Rust 1.80+:

```bash
cargo install --path .
tablec --version
```

## Quick Start

### Build

```bash
tablec build -i input.xlsx -o output.json

# build ./tablec.toml or all *.xlsx in cwd
tablec build
# same, against ./data
tablec build ./data
```

### Check for Errors

```bash
tablec check path/to/files
```

### Create Example Excel

```bash
tablec example -o example.xlsx -r 10
```

If the input directory contains `tablec.toml` (or `.tablec.toml`), it controls
include patterns, output name, format, and so on. `--config path/to/other.toml`
overrides auto-discovery.


## Data Format

Excel Sheet structure (first 5 rows are reserved for schema):

| Row | Content |
|-----|---------|
| 1 | Field names |
| 2 | Field types |
| 3 | Field comments |
| 4 | Constraints |
| 5 | Reserved |

## 自定义头布局

如果你的 Excel/CSV 头不是 5 行标准布局（行数不同、字段名/类型不同行），写一个 plugin 接管：

```rust
use tablec_core::core::schema::{Schema, SchemaParser, SchemaParseResult};

pub struct MyParser;
impl SchemaParser for MyParser {
    fn name(&self) -> &str { "my-parser" }
    fn parse_schema(&self, sheet_name: &str, sheet: &[Vec<String>]) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        // 你的解析逻辑
    }
}
```

详细文档见 `doc/design.md#插件机制`。

## Type System

### Basic Types
- **Integers**: `int8`, `int16`, `int32`, `int64`
- **Unsigned**: `uint8`, `uint16`, `uint32`, `uint64`
- **Floats**: `float32`, `float64`
- **String**: `string` or `str`
- **Boolean**: `bool`

### Array Types
```rust
int[]          // One-dimensional array
int[][]        // Two-dimensional array
string[]       // String array
```

### Map Types
```rust
map<int, string>          // Int key, string value
map<string, int[]>        // String key, int array value
```

### Struct Types
```rust
{x:int, y:float}                  // Point struct
struct{name:str, value:int[]}     // Named struct
```

## Constraints

- `@unique` - Field value must be unique
- `@unique(field1, field2)` - Composite unique constraint
- `@seq` - Sequential values (1, 2, 3, ...)
- `@seq(2)` - Sequential with step (1, 3, 5, ...)
- `@order` - Ascending order
- `@order(desc)` - Descending order

## Testing

### Run All Tests

```bash
cargo test --package tablec-core
```

### Run Scale Tests

```bash
# Small scale (10 tables x 10 rows)
cargo test --package tablec-core test_small_scale

# Medium scale (100 tables x 1000 rows)
cargo test --package tablec-core test_medium_scale -- --ignored

# Large scale (1000 tables x 10000 rows)
cargo test --package tablec-core test_large_scale -- --ignored
```

### Run Performance Benchmarks

```bash
cargo bench --package tablec-core
```

### Test Coverage

| Category | Tests |
|----------|-------|
| Type Coverage | Basic types, arrays, maps, structs |
| Constraints | @unique, @seq, @order |
| Scale Tests | 10/100/1000 tables, 10/1000/10000 rows |
| Performance | Parse time, export speed, memory usage |

## Python Bindings

See [README](binding-python/README.md)

## WebUI

For an interactive browser-based UI (preview Excel files, trigger builds,
inspect diagnostics), launch the built-in webui:

```bash
cargo run -- webui --dir ./data
# or, against a built binary:
./target/debug/tablec webui --dir ./data
```

The server binds to `127.0.0.1` on an OS-assigned port and auto-opens the
browser to the entry URL. Use `--no-browser` to skip the auto-open
(useful for CI / remote hosts) and `--port <N>` to pin the port.

Features exposed:

- Browse the configured data directory and preview every sheet in every
  `.xlsx` / `.xls` / `.xlsb` / `.ods` file (first 5 schema rows + first 100
  data rows)
- **Diff preview**: when the working directory is inside a git repository,
  the file list shows each spreadsheet's status vs the current branch HEAD
  (`modified` / `added` / `untracked` / `deleted` / `clean`) and a
  "Modified only" filter; the parsed preview colors per-cell changes —
  green for added, red for deleted, amber for modified. Outside a repo (or
  without a HEAD) everything reports clean and no colors are shown.
- Trigger a build with the chosen format (`json`, `json-pretty`, `msgpack`)
- Trigger a check (validates per-table constraints *and* cross-table `@ref`
  — the latter is a fix on top of the CLI `check` command, which currently
  skips it)
- Inspect diagnostics rendered with severity color-coding

**Note:** the data-validation feature (`/api/validate`) is **not yet
implemented** — the UI shows a TODO and the endpoint returns
`501 Not Implemented`. The CLI flag is `webui`; everything else is the
same shape as `build` / `check`.

## Development

```bash
git config core.hooksPath .githooks
```

Bypass for a single commit when importing pre-existing drift:

```bash
git commit --no-verify
```

CI also runs `cargo fmt --all --check` as a backstop.

## Architecture

```
tablec/
├── tablec-core/        # Core library (Rust)
│   ├── src/            # Source code
│   ├── tests/          # Integration tests
│   └── benches/        # Performance benchmarks
├── tablec-cli/         # CLI application
├── binding-python/     # Python bindings (PyO3)
└── doc/                # Documentation
```

## License

```
Copyright (c) 2023-2026 cupen<xcupen@gmail.com>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
