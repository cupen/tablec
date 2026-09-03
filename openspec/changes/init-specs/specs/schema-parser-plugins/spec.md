## Purpose

Defines how tablec can be extended with custom schema parsers: the `SchemaParser` contract, static registration, dynamic cdylib loading with its matching-toolchain requirement, and how CLI/config select the active parser.

## ADDED Requirements

### Requirement: SchemaParser contract
A schema parser SHALL implement `name()` returning its identifier and `parse_schema(sheet_name, sheet)` returning either a `Schema` (with fields, table constraints, and `data_start_row`) or a `Skip` result, or a list of diagnostics on failure. The standard parser's name SHALL be `"standard"`.

#### Scenario: Parser dispatches by name
- **WHEN** a parser implements `name()` as `"my-parser"`
- **THEN** the registry returns that parser when requested by `"my-parser"`

### Requirement: Static parser registration
A registry SHALL support registering parsers in-process; the registry SHALL reject duplicate names with a panic. The standard parser SHALL be pre-registered.

#### Scenario: Registering again panics
- **WHEN** a parser with an already-registered name is registered
- **THEN** registration panics with a message naming the duplicate

### Requirement: Dynamic plugin loading
A dynamic plugin SHALL be a cdylib exporting `tablec_plugin_create_v1` (returning a `*mut SchemaParser`) and `tablec_plugin_drop_v1`. Loading SHALL resolve both symbols, reject null pointers, and surface load/symbol errors with hints that host and plugin must be compiled with the same Rust toolchain. A panicking plugin's `parse_schema` SHALL be caught and converted to a `HeaderParserError` diagnostic. Loading a plugin whose name duplicates a registered parser SHALL error.

#### Scenario: Missing plugin file
- **WHEN** a plugin path does not exist
- **THEN** loading fails with a message hinting at the `.so` load and matching Rust toolchain

#### Scenario: Missing ABI symbols
- **WHEN** a plugin does not export the required symbols
- **THEN** loading fails with a message naming the expected `create_v1` / `drop_v1` symbols

#### Scenario: Plugin panic is contained
- **WHEN** a loaded plugin panics while parsing a sheet
- **THEN** the caller receives a `HeaderParserError` diagnostic instead of a crash

### Requirement: Parser resolution precedence
The active parser SHALL resolve in the order: CLI `--parser` flag, then config `[parser] name`, then `"standard"`. Plugin paths SHALL be taken from config `[[plugins]] path` plus CLI `--plugin-path` flags. An unregistered resolved name, an empty name, or a failing plugin load SHALL fail the command.

#### Scenario: CLI overrides config parser
- **WHEN** config names parser `"bogus"` but CLI passes `--parser standard`
- **THEN** the standard parser is used

#### Scenario: Unknown parser name fails
- **WHEN** the resolved parser name is not registered
- **THEN** the command fails with a message that the parser is not registered

### Requirement: Plugin loading exercises arbitrary code (trust boundary)
Dynamic plugin loading runs arbitrary native initializer code; the loading API documents that callers must supply trusted, ABI-compatible plugin paths, and that host and plugin must use the same Rust toolchain. This requirement exists so future callers (including the webui) treat cdylib loading as a privileged operation.

#### Scenario: Loader documents trust requirement
- **WHEN** a caller loads a dynamic plugin
- **THEN** the load is performed under the documented assumption that the path is trusted and ABI-compatible