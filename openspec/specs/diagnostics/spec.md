# diagnostics Specification

## Purpose

Defines the diagnostic model shared by core, CLI, webui, and Python bindings: how errors carry severity, code, message, and source location, and how they are rendered to users.

## Requirements

### Requirement: Diagnostic structure
Every failure SHALL be represented by a `Diagnostic` with a `Severity` (Error or Warning), a `DiagnosticCode`, a human-readable `message`, and a `SourceLocation` carrying optional file path, sheet name, line, and column. Constructing a diagnostic without explicit severity SHALL default to Error.

#### Scenario: Diagnostic carries code, message, and location
- **WHEN** a value fails to parse at a known sheet, line, and column
- **THEN** the diagnostic has the parse-error code, a message describing the failure, and a location holding the sheet, line, and column

### Requirement: Diagnostic codes are stable and countable
The code set SHALL be a fixed, non-exhaustive enum that serializes by name. Adding or removing a code REQUIRES updating the locked count test.

#### Scenario: Code set is locked
- **WHEN** the diagnostic code enum is enumerated
- **THEN** it contains exactly the documented set of 26 codes

### Requirement: Diagnostic rendering
The rendered form of a diagnostic SHALL be `CODE [sheet] line:col: message`, omitting whichever location parts are absent: the `[sheet]` block only when no sheet is set, and the `line:col` block only when either line or column is missing. The file path is metadata only and MUST NOT appear in the rendered text.

#### Scenario: Full location rendering
- **WHEN** a diagnostic has sheet `S`, line 1, column 3, and a message
- **THEN** rendering contains the code, `[S]`, `1:3`, and the message

#### Scenario: No sheet block when sheet absent
- **WHEN** a diagnostic has line and column but no sheet
- **THEN** rendering contains no `[` `]` block

#### Scenario: No line:col block when line or column absent
- **WHEN** a diagnostic has a sheet but no line or column
- **THEN** rendering contains the sheet block and message but no `<line>:<col>` segment

#### Scenario: File path never rendered
- **WHEN** a diagnostic has a file path set
- **THEN** the rendered output does not contain the path

### Requirement: Diagnostic serialization
A `Diagnostic` SHALL serialize and deserialize through serde, preserving severity, code, message, and location, for both JSON and MessagePack transport.

#### Scenario: JSON round-trip
- **WHEN** a diagnostic is serialized to JSON and deserialized back
- **THEN** the fields are preserved

### Requirement: Aggregate validation surfaces all diagnostics
Table validation and project validation SHALL return a list of all diagnostics for the input rather than stopping at the first failure.

#### Scenario: Multiple violations reported together
- **WHEN** a table violates several constraints at once
- **THEN** validation returns a diagnostic for each violation