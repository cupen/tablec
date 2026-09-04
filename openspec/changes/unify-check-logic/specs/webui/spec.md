## MODIFIED Requirements

### Requirement: Check endpoint
`POST /api/check` SHALL accept `{ dir, parser?, plugin_paths? }` and validate the configured input directory through the same shared check pipeline as the CLI `check` subcommand: files enumerated with the configured include/exclude globs, parsed with the selected parser (request `parser` or server override, resolved via the parser registry), validated per table, and validated once at project level across all parsed tables so cross-table `@ref` constraints are enforced. It SHALL return `{ diagnostics, duration_ms, sheets_checked }`, with each project-level diagnostic reported exactly once. A missing resolved input directory SHALL be rejected as 400 with a hint about `tablec.toml`.

The check endpoint SHALL NOT itself change the diff baseline; the git HEAD baseline is the reference for change detection and is not mutated by build or check actions.

#### Scenario: Check runs @ref validation
- **WHEN** the parsed tables contain a violating `@ref` whose target lives in another file
- **THEN** the returned `diagnostics` includes the foreign-key violation exactly once

#### Scenario: Missing input directory
- **WHEN** the config points at a nonexistent `data` directory
- **THEN** the response is 400 with a message mentioning the input directory

#### Scenario: Diff baseline is not a build cache
- **WHEN** a build or check runs after a diff was displayed
- **THEN** the diff statuses remain relative to git HEAD, not to the last build/check result

#### Scenario: Check matches CLI semantics
- **WHEN** the same input directory (same parser selection) is checked via `tablec check` and `POST /api/check`
- **THEN** both report the same set of diagnostics
