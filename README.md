# Tablec

A **Table Compiler** for game development - compiles Excel/CSV/JSON table data into program-readable formats (JSON, MessagePack).

## Features

- **Table Schema**: MySQL-like schema with field types and constraints
- **Blazing Fast**: Optimized Rust implementation
- **Rich Type System**: Support for basic types, arrays, maps, and structs
- **Multiple Export Formats**: JSON, MessagePack
- **CLI Tool**: Build, check, example commands
- **Python Bindings**: Native Python API via PyO3

## Installation


Requires Rust 1.60+:

```bash
cargo install --path .
```

## Quick Start

### Build Excel to JSON

```bash
tablec build -i input.xlsx -o output.json
```

### Check Excel for Errors

```bash
tablec check path/to/files
```

### Create Example Excel

```bash
tablec example -o example.xlsx -r 10
```

## Data Format

Excel Sheet structure (first 5 rows are reserved for schema):

| Row | Content |
|-----|---------|
| 1 | Field names |
| 2 | Field types |
| 3 | Field comments |
| 4 | Constraints |
| 5 | Reserved |

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

```bash
cd binding-python
pip install maturin
maturin develop

python -c "import tablec; tablec.check('file.xlsx')"
```

`build(input, output, format)` accepts `json` (minified), `json-pretty` (indented), or `msgpack`. The `json` default matches the CLI's minified output.

## Development

### Auto-format on commit

This repo ships a git pre-commit hook that runs `cargo fmt --all` and stages the result before each commit (mirrors Go's `gofmt` workflow — format drift never reaches CI). Enable it once per clone:

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

[Your License]
