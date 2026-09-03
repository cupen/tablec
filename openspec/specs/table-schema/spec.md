# table-schema Specification

## Purpose

Defines how tablec discovers and exposes workbooks' tables: the 5-row Excel header contract, the field type system (primitives, arrays, maps, structs), field metadata, and the schema-level default-not-null semantics.

## Requirements

### Requirement: Workbook loading scans every sheet
The system SHALL open a workbook (`.xlsx` / `.xls` / `.xlsb` / `.ods`) via the configured parser and produce one `Table` per parsed sheet. Sheets whose name starts with `#` MUST be skipped. Sheets whose first row's first cell starts with `#` MUST be skipped by the standard parser.

#### Scenario: Parsing a workbook with mixed sheets
- **WHEN** a workbook contains a normal sheet `Items` and a sheet named `#comment`
- **THEN** the result contains a table named `Items` and no table whose name starts with `#`

#### Scenario: Opening a missing or corrupt workbook
- **WHEN** the workbook cannot be opened
- **THEN** a `Diagnostic` is produced describing the failed open, and parsing stops with errors

### Requirement: Five-row schema header layout
The standard parser SHALL interpret the first five rows of a sheet as: row 1 field names, row 2 field type strings, row 3 field comments, row 4 per-field constraints, row 5 table-level constraints. Data rows start at row 6. The declared `data_start_row` for a standard-layout sheet SHALL be 5.

#### Scenario: Standard 5-row sheet with data
- **WHEN** a sheet has `id, name` in row 1, `int, string` in row 2, comments in row 3, a `@unique` constraint in the `id` cell of row 4, empty row 5, and data from row 6
- **THEN** the parsed table has two fields, `id` of type `int`, `name` of type `string`, the `id` field carries the `@unique` constraint, and the field-name comment is preserved

#### Scenario: Sheet with fewer than five rows
- **WHEN** a sheet has fewer than five rows
- **THEN** the parser treats all rows as header and produces a table with zero data rows rather than failing on out-of-bounds access

#### Scenario: First cell starts with a hash
- **WHEN** the first row's first cell starts with `#`
- **THEN** the standard parser returns a skip result and no table is emitted for that sheet

### Requirement: Field metadata with tags
A field name MAY carry tags in `name[tag1,tag2]` form; the parsed field name SHALL be the part before `[`, and the bracketed comma-separated tokens SHALL become the field's tags. Fields with an empty name or a name starting with `#` MUST be omitted. Unparseable type strings MUST fall back to `string`. Exact field-name duplicates within a sheet MUST produce a `SchemaFieldOverlap` diagnostic and the sheet is not emitted.

#### Scenario: Tagged field name
- **WHEN** a field name cell contains `id[client,key]`
- **THEN** the field is named `id` with tags `["client", "key"]`

#### Scenario: Unknown type falls back to string
- **WHEN** a field type cell contains `not_a_type`
- **THEN** the field is typed as `string`

#### Scenario: Duplicate field names
- **WHEN** two columns declare the same field name
- **THEN** a `SchemaFieldOverlap` diagnostic is produced and the sheet is not included in the result

### Requirement: Type system
The system SHALL support the field types: `int`/`int8`/`int16`/`int32`/`int64`, `uint`/`uint8`/`uint16`/`uint32`/`uint64`, `float`/`float32`/`float64`, `string`/`str`, `bool`/`boolean`, `date`, `datetime`, `timestamp32`, `timestamp64`, arrays (`type[]` and `array<type>`, nestable), maps (`map<keyType, type>` where keyType is integer or string), and structs (`struct{name:type,...}` or `{name:type,...}` with up to 32 fields, omitted type defaulting to `string`). Type strings are tokenized; an unknown type token MUST produce a `TypeUnknown` diagnostic.

#### Scenario: Each type family parses
- **WHEN** the type strings `int`, `uint16`, `float64`, `string`, `bool`, `int[][]`, `array<array<int>>`, `map<string, int[]>`, and `struct{a:int, b:string[]}` are parsed
- **THEN** each parses to its corresponding structured field type

#### Scenario: Struct with omitted type
- **WHEN** a struct type `{foo, bar:int}` is parsed
- **THEN** `foo` is typed as `string` and `bar` as `int`

#### Scenario: Unknown type token
- **WHEN** a type string contains an unknown token such as `foo`
- **THEN** a `TypeUnknown` diagnostic is produced

### Requirement: Schema-level default not-null
Absent an explicit `@nullable` on a field, every data-row cell of that field MUST be non-empty: an empty string, a missing cell, or a `Value::Null` SHALL raise a `ConstraintNullNotAllowed` diagnostic before any other per-field constraint runs on that cell. A field marked `@nullable` MUST opt out of this pre-check for itself.

#### Scenario: Empty cell without @nullable
- **WHEN** a field has no `@nullable` and a data row contains an empty cell
- **THEN** a `ConstraintNullNotAllowed` diagnostic is produced for that cell

#### Scenario: Empty cell with @nullable
- **WHEN** a field is marked `@nullable` and a data row contains an empty cell
- **THEN** no not-null diagnostic is produced for that cell

### Requirement: Date and time types are string-valued
The `date`, `datetime`, `timestamp32`, and `timestamp64` field types SHALL be parsed and stored as string values.

#### Scenario: Date cell value
- **WHEN** a field of type `date` has the cell value `2026-01-01`
- **THEN** the stored value is the string `2026-01-01`