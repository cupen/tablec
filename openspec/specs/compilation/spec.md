# compilation Specification

## Purpose

Defines how tablec compiles a workbook directory or file into a typed, structured project and exports it to JSON, JSON-pretty, or MessagePack, including the content hash stamped into the output metadata.

## Requirements

### Requirement: Table value parsing
Each data cell SHALL be parsed according to its declared field type. Numeric cells parse to their typed width (`int8`..`int64`, `uint8`..`uint64`, `float32`, `float64`); out-of-range integers produce `ValueOutOfRange`; non-numeric content produces `ValueParseError`. Boolean cells accept `true`/`1` and `false`/`0` case-insensitively. String cells trim surrounding whitespace and strip a single pair of surrounding `'` or `"` quotes. Array cells parse `[a, b, c]` recursively; map cells parse `k:v, k:v`; struct cells parse `{name: value, ...}` matching field names against the declared struct fields.

#### Scenario: Out-of-range integer
- **WHEN** a field of type `int8` holds the cell value `200`
- **THEN** a `ValueOutOfRange` diagnostic is produced

#### Scenario: Non-numeric integer cell
- **WHEN** a field of type `int32` holds the cell value `abc`
- **THEN** a `ValueParseError` diagnostic is produced

#### Scenario: Struct by name
- **WHEN** a struct field `{a:int, b:str}` holds `{b: hi, a: 7}`
- **THEN** the value parses to `a = 7` and `b = "hi"` regardless of text order

#### Scenario: Struct missing declared field
- **WHEN** a struct field `{a:int, b:int}` holds `{a: 1}`
- **THEN** a `StructFieldCountMismatch` diagnostic is produced

### Requirement: Row and table assembly
A parsed sheet produces a `Table` with a name (the sheet name), a schema (fields and table constraints), and data rows. Each data row SHALL preserve column order in its field map. Rows whose cells are all empty MUST be skipped. A parse error in a cell produces a diagnostic and that row omits the offending field, while remaining rows still parse.

#### Scenario: All-empty data row skipped
- **WHEN** a data row has every cell empty
- **THEN** the row is not added to the table's data

#### Scenario: Cell parse error drops that field
- **WHEN** a cell fails to parse for its declared type
- **THEN** a diagnostic is produced and the row is emitted without that field

### Requirement: Project model
A `Project` SHALL aggregate tables by name with the project name, source file list, and `Meta`. `Meta` SHALL carry `version`, a 32-byte content `hash`, Unix-seconds `build_at`, the list of source file paths, and tool version info. The hash MUST be deterministic for identical content, sensitive to data row changes and to which sheets exist, and independent of field declaration order.

#### Scenario: Deterministic hash
- **WHEN** two projects are built from identical content
- **THEN** their `Meta.hash` values are equal

#### Scenario: Hash changes with data
- **WHEN** a single data value differs between two projects
- **THEN** their `Meta.hash` values differ

### Requirement: JSON export shape
JSON export SHALL produce an object `{ "name": <project name>, "meta": <Meta>, "tables": [ { "name": <table name>, "data": [ <row objects> ], "fields": [ <fields> ]? } ] }`. The `fields` array SHALL be included only when `include_fields` is enabled. Row objects SHALL serialize numeric values as JSON numbers (not strings), maps as JSON objects keyed by stringified keys, structs as JSON objects, and `Null` as JSON `null`. Map keys MUST be simple scalar values.

#### Scenario: Compact JSON contains name, meta, and tables
- **WHEN** a project is exported as compact JSON
- **THEN** the output is valid JSON containing the project name, `meta`, and a `tables` array

#### Scenario: Row keys follow sheet column order
- **WHEN** a row is exported to JSON with `include_fields` off and on
- **THEN** the row object's keys appear in sheet column order in both cases

#### Scenario: Fields array gated by include_fields
- **WHEN** `include_fields` is false
- **THEN** no `fields` key appears in the table objects

### Requirement: MessagePack export
MessagePack export SHALL serialize the project with `rmp-serde`, producing a MessagePack binary stream whose content mirrors the project structure (including `meta` as a map).

#### Scenario: Msgpack output round-trips to project shape
- **WHEN** a project is exported to MessagePack and read back with a MessagePack reader
- **THEN** the decoded structure contains the project name, meta fields, tables, and row data

### Requirement: Export writes files with parent creation
The JSON and MessagePack exporters SHALL create the output file's parent directories when absent and write bytes to the requested path.

#### Scenario: Export into a new directory
- **WHEN** an export targets a path whose parent directory does not exist
- **THEN** the parent directory is created and the output file is written