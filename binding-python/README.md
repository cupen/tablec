# tablec Python Bindings

Python bindings for the tablec (Table Compiler) library.

## Installation

### Development

```bash
# Install dependencies using uv
cd binding-python
uv sync

# Build the Rust extension
uv run maturin develop
```

### From wheel (future)

```bash
pip install tablec
```

## Usage

### Basic API

```python
import tablec

# Build Excel to JSON
tablec.build('input.xlsx', 'output.json', 'json')

# Build Excel to MessagePack
tablec.build('input.xlsx', 'output.msgpack', 'msgpack')

# Validate Excel file
tablec.check('input.xlsx')
```

### High-level API

```python
from tablec.core import Project

# Load a project from Excel
project = Project.load('project.xlsx')

# Access tables by name
table = project['TableName']

# Iterate over tables
for table_name in project:
    table = project[table_name]
    for row in table:
        print(row)

# Export project
project.export('output.json', 'json')
```

### Type System

```python
from tablec.types import parse

# Parse type strings
int_type = parse('int')
array_type = parse('int[]')
map_type = parse('map<string, int>')
struct_type = parse('{a:int, b:string}')

# Type checking
if array_type.is_array():
    print('This is an array type')
```

## Excel Format

Excel sheets should follow this format:

| Row 1    | Field names (e.g., id, name, score) |
| Row 2    | Field types (e.g., int, string, float) |
| Row 3    | Descriptions (use # for none) |
| Row 4    | Constraints (e.g., @unique, @seq) |
| Row 5+   | Data rows |

### Supported Types

- **Primitives**: `int`, `int8`, `int16`, `int32`, `int64`, `uint`, `uint8`, `uint16`, `uint32`, `uint64`, `float`, `float32`, `float64`, `string`
- **Arrays**: `int[]`, `string[]`, etc.
- **Maps**: `map<string, int>`, `map<int, string>`
- **Structs**: `{a:int, b:string}`, `struct{id:int, name:string}`

### Constraints

- `@unique` - Field must be unique
- `@unique(field1, field2)` - Combination of fields must be unique
- `@seq` - Sequential field (1, 2, 3, ...)
- `@seq(step)` - Sequential with step (1, 3, 5, ...)
- `@order` - Ordered ascending
- `@order(desc)` - Ordered descending

## Testing

```bash
# Run all tests
uv run pytest tests/ -v

# Run specific test file
uv run pytest tests/test_python_binding.py -v
```

## Building for Distribution

```bash
# Build release wheel
uv run maturin build --release

# Build for multiple Python versions
uv run maturin build --release --strip --compatibility manylinux
```

## Error Handling

```python
try:
    tablec.check('file.xlsx')
except Exception as e:
    print(f"Validation failed: {e}")
```

## Import name vs package name

The package published to PyPI is `tablec`. The Python import name is also `tablec`
(PEP 421 allows the two to differ, but here they match on purpose):

```bash
pip install tablec
python -c "import tablec; tablec.check('your.xlsx')"
```

If you `pip install` a checkout of this repo, the editable install uses the
project's `name` (`tablec`) — `import tablec` is the canonical import.

## License

Same as the main tablec project.
