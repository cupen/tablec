# constraints Specification

## Purpose

Defines the table compiler's constraint system: the `@func(...)` grammar for declaring constraints in the header, the named constraints available at field level, table level, and project level, how arguments are parsed, and how each violation maps to a `DiagnosticCode`.

## Requirements

### Requirement: Constraint declaration grammar
A constraint cell MUST start with `@`, contain a function name, and any arguments in parentheses: `@func(arg1, arg2, ...)`. Arguments are separated by top-level commas; commas inside double quotes are literal; quoted strings support only the `\"` and `\\` escapes, and any other `\X` sequence MUST be rejected. Malformed constraints (missing `@`, missing closing parenthesis, unterminated string, unsupported escape, empty function name, or a `"` mid-argument) MUST produce a `TableConstraintParseError` diagnostic. A bare `@func` without parentheses is valid when the name is a single token.

#### Scenario: Valid multi-argument constraint
- **WHEN** the cell contains `@oneof("a,b", c)`
- **THEN** the constraint parses with function `oneof` and arguments `["a,b", "c"]`

#### Scenario: Malformed constraint
- **WHEN** the cell contains `@func(a, b` (missing closing parenthesis)
- **THEN** a `TableConstraintParseError` diagnostic is produced

#### Scenario: Unsupported escape in quoted argument
- **WHEN** the cell contains `@pattern("\n")`
- **THEN** a `TableConstraintParseError` diagnostic is produced because `\n` is not an allowed escape

### Requirement: @nullable
`@nullable` marks a field as allowing empty cells, opting out of the schema-level default not-null pre-check. It takes no arguments; supplying arguments to `@nullable` MUST be rejected.

#### Scenario: Nullable field accepts empty cell
- **WHEN** a field declared `@nullable` has an empty cell
- **THEN** no not-null diagnostic is produced and inner constraints (e.g. `@oneof`) skip that row

### Requirement: @range
`@range(lo, hi)` requires each cell of a single integer field to be an integer in the closed interval `[lo, hi]`. Arguments MUST be integers; `lo` MUST NOT exceed `hi`. A violation produces `ConstraintValueViolation`.

#### Scenario: Boundary values accepted
- **WHEN** a field declared `@range(1, 10)` holds values `1`, `5`, and `10`
- **THEN** validation passes

#### Scenario: Out-of-range value
- **WHEN** a field declared `@range(1, 10)` holds value `0` or `11`
- **THEN** a `ConstraintValueViolation` diagnostic is produced

#### Scenario: Inverted range rejected
- **WHEN** a constraint is declared as `@range(10, 1)`
- **THEN** validation fails with an error and produces a diagnostic

### Requirement: @oneof
`@oneof(v1, v2, ...)` requires each cell to match one of the allowed values. Values that parse as `i64` go into a numeric bucket and are compared against numeric cells; all other values are treated as strings and compared against string cells. A cell that matches neither bucket produces `ConstraintNotInSet`.

#### Scenario: String enum matches
- **WHEN** a field declared `@oneof(red, green, blue)` holds `green`
- **THEN** validation passes

#### Scenario: Non-allowed value
- **WHEN** a field declared `@oneof(red, green, blue)` holds `yellow`
- **THEN** a `ConstraintNotInSet` diagnostic is produced

#### Scenario: Mixed numeric and string buckets
- **WHEN** a field declared `@oneof("1", "2")` holds integer `2`
- **THEN** validation passes; when it holds integer `4`, a `ConstraintNotInSet` diagnostic is produced

### Requirement: @maxlen
`@maxlen(n)` requires each cell of a single string field to have at most `n` characters counted as UTF-8 chars. A negative `n` MUST be rejected. A violation produces `ConstraintValueViolation`.

#### Scenario: Length at the bound
- **WHEN** a field declared `@maxlen(5)` holds `abcde`
- **THEN** validation passes

#### Scenario: Length over the bound
- **WHEN** a field declared `@maxlen(5)` holds `abcdef`
- **THEN** a `ConstraintValueViolation` diagnostic is produced

### Requirement: @pattern
`@pattern("regex")` requires each cell of a single string field to match the regex. The regex argument MUST be double-quoted. An invalid regex MUST be rejected. A non-matching value produces `ConstraintPatternMismatch`.

#### Scenario: Value matches pattern
- **WHEN** a field declared `@pattern("^[a-z]+@[a-z]+$")` holds `alice@example`
- **THEN** validation passes

#### Scenario: Value does not match pattern
- **WHEN** a field declared `@pattern("^[a-z]+@[a-z]+$")` holds `no-at-symbol`
- **THEN** a `ConstraintPatternMismatch` diagnostic is produced

#### Scenario: Invalid regex
- **WHEN** a constraint declares an unparseable regex such as `@pattern("([unclosed")`
- **THEN** validation fails with an error

### Requirement: @unique
`@unique` on a single field, or `@unique(a, b, ...)` on a set of fields, requires the row-key formed by the named fields to be unique across all data rows. Rows whose key values are all empty (empty string or null) MUST be skipped (SQL-style NULL semantics); any non-empty key forces the row into the duplicate set. A duplicate produces `ConstraintDuplicate`.

#### Scenario: Unique single-field values
- **WHEN** a field declared `@unique` holds `1, 2, 3`
- **THEN** validation passes

#### Scenario: Duplicate single-field value
- **WHEN** a field declared `@unique` holds `1, 1`
- **THEN** a `ConstraintDuplicate` diagnostic is produced

#### Scenario: Composite unique key
- **WHEN** a table declares `@unique(a, b)` and two rows share both `a` and `b`
- **THEN** a `ConstraintDuplicate` diagnostic is produced

#### Scenario: Empty keys are skipped
- **WHEN** a field declared `@unique` holds empty values in multiple rows
- **THEN** validation passes because all-empty rows are not compared

### Requirement: @seq
`@seq` requires a single integer field's values to follow the sequence `1, 1+step, 1+2*step, ...`. `@seq` uses step `1`; `@seq(step)` uses the given (possibly negative) integer step. The step MUST be an integer. A break produces `ConstraintSequenceBroken`.

#### Scenario: Default sequence
- **WHEN** a field declared `@seq` holds `1, 2, 3`
- **THEN** validation passes

#### Scenario: Stepped sequence
- **WHEN** a field declared `@seq(2)` holds `1, 3`
- **THEN** validation passes

#### Scenario: Broken sequence
- **WHEN** a field declared `@seq` holds `1, 3`
- **THEN** a `ConstraintSequenceBroken` diagnostic is produced

### Requirement: @order
`@order` (ascending) or `@order(desc)` requires a single field's values to be monotonic in the declared direction: ascending forbids `prev > current`, descending forbids `prev < current`. Values that cannot be compared SHALL produce an error. An unrecognized direction argument MUST be rejected. A violation produces `ConstraintOrderViolation`.

#### Scenario: Ascending values
- **WHEN** a field declared `@order` holds `1, 2, 3`
- **THEN** validation passes

#### Scenario: Descending violation
- **WHEN** a field declared `@order(desc)` holds `1, 5`
- **THEN** a `ConstraintOrderViolation` diagnostic is produced

#### Scenario: Invalid direction argument
- **WHEN** a field declares `@order(sideways)`
- **THEN** validation fails with an error

### Requirement: @ref foreign keys
`@ref("T.c")` (field-level, host is the field itself) or `@ref(host, "T.c")` (table-level, host is the named field) requires every non-empty host value in the table to exist in column `c` of table `T`. A host cell that is empty or null MUST be skipped (SQL-style nullable foreign key). A missing target table, a missing target column, or a host value absent from the target column MUST produce `ConstraintForeignKeyViolation` with a message distinguishing the three cases.

#### Scenario: Referenced value exists
- **WHEN** table `Drop` declares `@ref("Item.id")` on `item_id` and every `item_id` value exists in `Item.id`
- **THEN** project validation passes

#### Scenario: Referenced value missing
- **WHEN** a `Drop.item_id` value of `99` is not present in `Item.id`
- **THEN** a `ConstraintForeignKeyViolation` diagnostic is produced

#### Scenario: Empty host cell allowed
- **WHEN** a `@ref` host cell is empty or null
- **THEN** that row is skipped and no foreign-key violation is produced

#### Scenario: Missing target table or column
- **WHEN** `@ref` names a table or column that does not exist
- **THEN** a `ConstraintForeignKeyViolation` diagnostic is produced with a message identifying the missing target

### Requirement: Constraint layering and execution
Validation SHALL run as a pre-check for default not-null, then field-level constraints (row 4), then table-level constraints (row 5). Cross-table `@ref` SHALL be deferred to project validation, which runs per-table validation for every table and then resolves all `@ref` constraints against the full set of tables. Unknown constraint functions MUST produce a `ConstraintUnknown` diagnostic.

#### Scenario: Unknown constraint function
- **WHEN** a cell declares `@totally_unknown(1)`
- **THEN** validation fails with a `ConstraintUnknown` diagnostic

#### Scenario: Project validation runs per-table then cross-table
- **WHEN** a project contains a table violating a single-table constraint and another violating `@ref`
- **THEN** `validate_project` returns diagnostics for both