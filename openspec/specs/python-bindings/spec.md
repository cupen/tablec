# python-bindings Specification

## Purpose

Defines the Python API surface exposed by the `tablec` PyO3 extension and the pure-Python package: `build()` and `check()` entry points, supported formats, and error mapping.

## Requirements

### Requirement: Native build function
The native extension SHALL expose `build(input, output=None, format=None, parser=None)`. `output` and `format` MUST be provided; omitting either SHALL raise a Python `ValueError`. The function SHALL resolve the parser (default `"standard"`), parse the input workbook, build a `Project` named after the input file's stem, and write the export to `output`. Supported formats SHALL be `json`, `json-pretty`, and `msgpack`; any other format SHALL raise a `ValueError` listing the supported formats. Parse failures SHALL raise a `ValueError` with the joined diagnostic text.

#### Scenario: Build a workbook to JSON
- **WHEN** `tablec.build("input.xlsx", "out.json", "json")` is called on a valid workbook
- **THEN** `out.json` is written containing the sheet data

#### Scenario: Missing output argument
- **WHEN** `tablec.build("input.xlsx", format="json")` is called without `output`
- **THEN** a `ValueError` is raised stating that output is required

#### Scenario: Unsupported format
- **WHEN** `tablec.build(..., format="yaml")` is called
- **THEN** a `ValueError` is raised listing `json`, `json-pretty`, `msgpack`

### Requirement: Native check function
The native extension SHALL expose `check(input, parser=None)`, resolve the parser (default `"standard"`), parse the workbook, and run per-table constraint validation on each table. Any violation SHALL raise a `ValueError` with the joined diagnostic text; otherwise the call SHALL return `None`.

#### Scenario: Check passes
- **WHEN** `tablec.check("valid.xlsx")` is called on a valid workbook
- **THEN** the call returns `None`

#### Scenario: Check fails
- **WHEN** `tablec.check("invalid.xlsx")` is called on a workbook with a constraint violation
- **THEN** a `ValueError` is raised containing the violation text

### Requirement: Parser selection in the bindings
The `parser` argument SHALL be resolved against a registry containing the standard parser. An unknown parser name SHALL raise a `ValueError` stating the parser is not registered. The bindings SHALL load only statically registered parsers (no plugin paths are accepted).

#### Scenario: Unknown parser name
- **WHEN** `tablec.build(..., parser="does-not-exist")` is called
- **THEN** a `ValueError` is raised stating the parser is not registered

### Requirement: Pure-Python package surface
The `tablec` Python package SHALL expose `Project`, `Table`, `parse_type`, `FieldType`, `ArrayType`, `MapType`, `StructType`, `ValidationError`, `ParseError`, and `ExportError`, alongside the native `build`/`check`. When the native module is unavailable, `build`/`check` SHALL raise a `RuntimeError` instructing the user to run `maturin develop`.

#### Scenario: Native module not compiled
- **WHEN** the native module is missing and `tablec.build(...)` is called
- **THEN** a `RuntimeError` is raised explaining the binding is not compiled