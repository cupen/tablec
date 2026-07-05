# tablec-core 清理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land 6 commits that fix P0 correctness issues in `tablec-core` (silent parse errors, type-width loss, tokenizer panic) plus two P1 items (table-level constraint execution, stable hash) per the spec at [`docs/superpowers/specs/2026-07-05-tablec-core-cleanup-design.md`](../specs/2026-07-05-tablec-core-cleanup-design.md).

**Architecture:** Layered TDD commits. c1 introduces `Diagnostic` value types; c2 changes `tokenizer` to return `Result`; c3 rewrites `Value` enum with 10 numeric variants + parse-time range checks; c4 changes `read_excel` to return `Vec<Diagnostic>` on error; c5 wires table-level constraints from row 5; c6 swaps the hash to Blake3 with a `[u8;32]` `Meta.hash` field rendered as hex in JSON.

**Tech Stack:** Rust 2024 edition (workspace resolver = 3), `calamine 0.25`, `serde 1`, `serde_json 1.0`, `indexmap 2.10`, `blake3 1` (new dep added in c6), `pretty_assertions` for dev tests, `pytest` for the `tablec-testsuite` companion.

## Global Constraints

- Repository root (workspace): `repos/tablec/`. Cargo workspace at `Cargo.toml`; members `tablec-core`, `tablec-cli`, `binding-python`. Default members `tablec-core`, `tablec-cli`.
- Plan/spec repo URL (do not use `github.com/example/...`): `https://github.com/cupen/tablec`.
- Per-commit verification required: `cargo build -p tablec-core` + `cargo test -p tablec-core` + (for c3/c4/c5) `pytest /home/bot/workbench/repos/tablec-testsuite`.
- Compat policy from spec §1.2: **breaking API changes allowed**.
- Error model: structured `Diagnostic` (c1) + `SourceLocation`. Library functions return `Result<T, Vec<Diagnostic>>` at boundaries, `Result<T, Diagnostic>` at single-error sites.
- Hash model from spec §6: Blake3 with domain separator `"tablec.project.v1"`. Output is `[u8; 32]`. JSON serialization of `Meta.hash` is **32-char hex string** (not a 32-number array).
- Test fixture paths under `tablec-core/tests/fixtures/error_cases/` are deliberately-bad; `tablec-testsuite` does NOT depend on them.
- Snapshot updates with `bash scripts/update_snapshots.sh --apply` are **manual decision points** — never auto-run without per-fixture diff review.
- Binding-python compile broken by c3 — c3.1 must land in the same batch as c3; no commit may leave `cargo build -p binding-python` failing on `main`.

---

## File Structure

Pre-existing files this plan modifies (each task lists its specific touchpoints):

| Path | Role before this plan |
|------|----------------------|
| `tablec-core/src/lib.rs` | Re-export hub. Adds `pub use diagnostic::*` in c1; adds `pub use constraint::*` semantics kept; no further net additions. |
| `tablec-core/src/core/mod.rs` | Module hub; c1 adds `pub mod diagnostic;`. |
| `tablec-core/src/core/diagnostic.rs` | **NEW in c1.** Diagnostic value types. |
| `tablec-core/src/core/parser/mod.rs` | Module hub. c3 removes `pub mod type_parser;`; c3 also writes a one-line deprecation comment. |
| `tablec-core/src/core/parser/tokenizer.rs` | c2: change signature to return `Result<Vec<Token>, Diagnostic>`. |
| `tablec-core/src/core/parser/value_parser.rs` | c3: rewrite to take `FieldType` + `SourceLocation`; new error mapping. |
| `tablec-core/src/core/parser/type_parser.rs` | **DELETED in c3.** |
| `tablec-core/src/core/table/field.rs` | c2: pass location into tokenizer; c3: rewrite `to_type()`; c5: no change. |
| `tablec-core/src/core/table/value.rs` | c3: rewrite enum with 10 numeric variants and trait impls. |
| `tablec-core/src/core/table/types.rs` | c3: rewrite `Type` enum, drop `Int/Uint/Float` aliases. |
| `tablec-core/src/core/table/row.rs` | No schema changes; c3 keeps `IndexMap<String, Value>` (struct semantics unchanged). |
| `tablec-core/src/core/table/constraint.rs` | c5: add `location` field; rewrite error path to produce `Diagnostic`. |
| `tablec-core/src/core/table/table.rs` | c3: thread `FieldType` through `read_excel`; c4: change return type; c5: parse row 5. |
| `tablec-core/src/core/table/validator.rs` | Touched only if separate; spec keeps validator logic in `constraint.rs::ConstraintValidator::validate_table` per existing file layout — no changes to validator.rs. |
| `tablec-core/src/core/project/project.rs` | c6: rewrite `calculate_hash`. |
| `tablec-core/src/core/project/meta.rs` | c6: change `hash` field, add `source`, `tool`; custom `Serialize`/`Deserialize` for hex. |
| `tablec-core/src/export/json.rs` | c3: small rewrite to drive `Value` traits; c4: handle new error; **delete `to_string(legacy)`**. |
| `tablec-core/src/export/msgpack.rs` | c3: same as above; c4: handle new error; **delete legacy `to_vec`**. |
| `tablec-core/Cargo.toml` | c6: add `blake3 = "1"`. |
| `tablec-core/tests/common/mod.rs` | **NEW in c1.** Shared test helpers (only `#[cfg(test)]` re-export from `lib.rs`). |
| `tablec-core/tests/fixtures/error_cases/bad_int_range.xlsx` | **NEW in c4.** Pre-existing conftest used. |
| `tablec-core/tests/fixtures/error_cases/bad_struct_field.xlsx` | **NEW in c4.** |
| `tablec-core/tests/fixtures/error_cases/bad_unique_constraint.xlsx` | **NEW in c5.** |
| `tablec-cli/src/cmd/build.rs` | c4: update callers of `read_excel`. |
| `tablec-cli/src/cmd/check.rs` | c4/c5: update callers. |
| `binding-python/src/lib.rs` | c3.1: update Value mapping if exposed; minimally update error mapping to handle `Vec<Diagnostic>`. |

**Decomposition rationale:** Each file gets one logical addition per task. Each task ends with a green commit boundary. c3 deliberately spans multiple files because the type rewrites must land together.

---

## Task 1: Add Diagnostic value types (commit c1)

**Files:**
- Create: `tablec-core/src/core/diagnostic.rs`
- Modify: `tablec-core/src/core/mod.rs`
- Modify: `tablec-core/src/lib.rs`
- Create: `tablec-core/tests/common/mod.rs`

**Interfaces produced (later tasks consume these):**
- `pub struct SourceLocation { file: Option<PathBuf>, sheet: Option<String>, line: Option<u32>, column: Option<u32> }`
- `pub enum Severity { Error, Warning }`
- `pub enum DiagnosticCode { … }` (variants from spec §3.1)
- `pub struct Diagnostic { severity: Severity, code: DiagnosticCode, message: String, location: SourceLocation }`
- `impl From<&str> for Diagnostic` — convenience default constructor at single-error sites.
- `pub fn expect_diagnostic(errs: &[Diagnostic], code: DiagnosticCode) -> &Diagnostic` (test helper)

- [ ] **Step 1: Write failing unit tests for Diagnostic**

Create `tablec-core/src/core/diagnostic.rs` with the body left as `todo!()` so the file compiles minimally:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: Option<PathBuf>,
    pub sheet: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity { Error, Warning }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticCode {
    TokenizerUnexpectedChar,
    TypeParseError,
    TypeUnknown,
    ValueParseError,
    ValueOutOfRange,
    StringEscapeUnsupported,
    StructFieldMismatch,
    StructFieldCountMismatch,
    SheetSkipped,
    FieldMissingValue,
    TableConstraintParseError,
    ConstraintUnknown,
    ConstraintDuplicate,
    ConstraintSequenceBroken,
    ConstraintOrderViolation,
    ConstraintFieldMissing,
    ConstraintCompositeMissing,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub location: SourceLocation,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, message: impl Into<String>, location: SourceLocation) -> Self {
        Self { severity: Severity::Error, code, message: message.into(), location }
    }
}

impl From<&str> for Diagnostic {
    fn from(s: &str) -> Self { Diagnostic::new(DiagnosticCode::Other, s, SourceLocation::default()) }
}

impl Default for SourceLocation {
    fn default() -> Self { Self { file: None, sheet: None, line: None, column: None } }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.code)?;
        if let Some(sheet) = &self.location.sheet {
            write!(f, " [{}]", sheet)?;
        }
        if let (Some(line), Some(col)) = (self.location.line, self.location.column) {
            write!(f, " {}:{}", line, col)?;
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_roundtrip_preserves_fields() {
        let d = Diagnostic::new(DiagnosticCode::ValueOutOfRange, "200 not in int8 range [-128, 127]",
            SourceLocation { file: None, sheet: Some("Sheet1".into()), line: Some(6), column: Some(2) });
        let json = serde_json::to_string(&d).unwrap();
        let d2: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(d.code, d2.code);
        assert_eq!(d.message, d2.message);
        assert_eq!(d.location.sheet, d2.location.sheet);
    }

    #[test]
    fn display_with_full_location() {
        let d = Diagnostic::new(DiagnosticCode::TokenizerUnexpectedChar, "bad char",
            SourceLocation { file: None, sheet: Some("S".into()), line: Some(1), column: Some(3) });
        let s = format!("{}", d);
        assert!(s.contains("TokenizerUnexpectedChar"));
        assert!(s.contains("S"));
        assert!(s.contains("1:3"));
        assert!(s.contains("bad char"));
    }

    #[test]
    fn display_with_empty_location() {
        let d: Diagnostic = "plain".into();
        assert_eq!(format!("{}", d), "Other : plain");
    }

    #[test]
    fn diagnostic_code_count_matches_spec() {
        // Lock the enum so adding new variants is intentional.
        let codes = [
            DiagnosticCode::TokenizerUnexpectedChar,
            DiagnosticCode::TypeParseError,
            DiagnosticCode::TypeUnknown,
            DiagnosticCode::ValueParseError,
            DiagnosticCode::ValueOutOfRange,
            DiagnosticCode::StringEscapeUnsupported,
            DiagnosticCode::StructFieldMismatch,
            DiagnosticCode::StructFieldCountMismatch,
            DiagnosticCode::SheetSkipped,
            DiagnosticCode::FieldMissingValue,
            DiagnosticCode::TableConstraintParseError,
            DiagnosticCode::ConstraintUnknown,
            DiagnosticCode::ConstraintDuplicate,
            DiagnosticCode::ConstraintSequenceBroken,
            DiagnosticCode::ConstraintOrderViolation,
            DiagnosticCode::ConstraintFieldMissing,
            DiagnosticCode::ConstraintCompositeMissing,
            DiagnosticCode::Other,
        ];
        // Intentionally asserts total variants — change ONLY when adding/removing a code.
        assert_eq!(codes.len(), 18);
    }
}
```

- [ ] **Step 2: Run tests, expect all four to pass already (it is a complete file)**

Run: `cd repos/tablec && cargo test -p tablec-core core::diagnostic`
Expected: 4 tests pass.

- [ ] **Step 3: Wire the module into the workspace**

Edit `tablec-core/src/core/mod.rs`:

```rust
pub mod diagnostic;
pub mod table;
pub mod parser;
pub mod plugin;
pub mod project;
pub mod config;
```

Edit `tablec-core/src/lib.rs`:

```rust
pub mod core;
pub mod export;

pub use core::diagnostic::*;
pub use core::table::*;
pub use core::parser::*;
pub use core::plugin::*;
pub use core::project::*;
pub use export::*;
```

- [ ] **Step 4: Create shared test helper module**

Create `tablec-core/tests/common/mod.rs`:

```rust
#![allow(dead_code)]
use tablec_core::core::diagnostic::{Diagnostic, DiagnosticCode};

pub fn expect_diagnostic<'a>(errs: &'a [Diagnostic], code: DiagnosticCode) -> &'a Diagnostic {
    errs.iter().find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected diagnostic with code {:?}, got: {:?}", code, errs))
}
```

Add a tiny shim test using it. Create `tablec-core/tests/diagnostic_export.rs`:

```rust
mod common;
use common::expect_diagnostic;
use tablec_core::core::diagnostic::*;

#[test]
fn helper_finds_code() {
    let errs = vec![
        Diagnostic::new(DiagnosticCode::SheetSkipped, "x", SourceLocation::default()),
        Diagnostic::new(DiagnosticCode::ValueParseError, "y", SourceLocation::default()),
    ];
    let d = expect_diagnostic(&errs, DiagnosticCode::ValueParseError);
    assert_eq!(d.message, "y");
}
```

Run: `cd repos/tablec && cargo test -p tablec-core --test diagnostic_export`
Expected: 1 passed.

- [ ] **Step 5: Commit c1**

```bash
git -C repos/tablec add tablec-core/src/core/diagnostic.rs \
    tablec-core/src/core/mod.rs tablec-core/src/lib.rs \
    tablec-core/tests/common/mod.rs tablec-core/tests/diagnostic_export.rs
git -C repos/tablec -c user.name="Claude" -c user.email="claude@anthropic.com" \
    commit -m "feat(core): add Diagnostic + SourceLocation value types

Lays the foundation for c2-c6 by adding structured error types. No
business-code callers yet — purely additive. See spec §3 and plan task 1."
```

---

## Task 2: Tokenizer returns Result (commit c2)

**Files:**
- Modify: `tablec-core/src/core/parser/tokenizer.rs`
- Modify: `tablec-core/src/core/table/field.rs`
- Modify: `tablec-core/src/core/parser/type_parser.rs` (will be deleted in c3, leave for now)

**Interfaces produced (later tasks use these signatures):**
- `pub fn scan_tokens(s: &str, loc: SourceLocation) -> Result<Vec<Token<'_>>, Diagnostic>`
- `pub fn FieldType::from_str_with_loc(s: &str, loc: SourceLocation) -> Result<Self, Diagnostic>` — new helper that wraps `scan_tokens` + `parse_from_tokens`. The existing `FieldType::from_str(s)` returns `Self` (panic-equivalent) and is kept only as a wrapper that calls `from_str_with_loc` and panics if the source is internal; spec §3.4 routes external parsing through the loc-aware path. (Effectively a one-call replacement done in c2.)

- [ ] **Step 1: Write failing test for scan_tokens returning Result**

Append to `tablec-core/src/core/parser/tokenizer.rs` (the file currently has `pub fn scan_tokens(s: &str) -> Vec<Token<'_>>`):

```rust
#[cfg(test)]
mod result_tests {
    use super::*;
    use crate::core::diagnostic::*;

    #[test]
    fn empty_string_returns_empty_vec() {
        let loc = SourceLocation::default();
        let tokens = scan_tokens("", loc).unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn symbols_only_tokens() {
        let loc = SourceLocation::default();
        let tokens = scan_tokens("[]<>{},:", loc).unwrap();
        assert_eq!(tokens.iter().map(|t| t.value).collect::<Vec<_>>(),
            vec!["[", "]", "<", ">", "{", "}", ",", ":"]);
    }

    #[test]
    fn unrecognized_char_returns_diagnostic() {
        let loc = SourceLocation { line: Some(1), column: Some(4), ..Default::default() };
        let err = scan_tokens("int🙂", loc).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::TokenizerUnexpectedChar);
        assert_eq!(err.location.line, Some(1));
        assert_eq!(err.location.column, Some(4));
        assert!(err.message.contains("🙂"));
    }

    #[test]
    fn existing_happy_path_preserved() {
        let loc = SourceLocation::default();
        let tokens = scan_tokens("array<int>, map<string, int>, array<float>", loc).unwrap();
        assert!(tokens.len() > 5);
    }
}
```

- [ ] **Step 2: Run new tests to verify they fail (panic path still active)**

Run: `cd repos/tablec && cargo test -p tablec-core core::parser::tokenizer::result_tests`
Expected: FAIL — `scan_tokens` is declared `-> Vec<Token>`, not `-> Result<…, Diagnostic>`, so the call sites in the new test return type mismatches.

- [ ] **Step 3: Change scan_tokens signature and panic → Diagnostic**

In `tablec-core/src/core/parser/tokenizer.rs`, replace the `scan_tokens` body so the unknown-char branch returns a `Diagnostic` instead of `panic!`:

```rust
pub fn scan_tokens<'a>(s: &'a str, loc: SourceLocation) -> Result<Vec<Token<'a>>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '<' | '>' | ',' | '{' | '}' | '[' | ']' | ':' => {
                tokens.push(Token {
                    value: &s[i..i + c.len_utf8()],
                    token_type: TokenType::Symbol,
                    start: i,
                    end: i + c.len_utf8(),
                });
            }
            _ if c.is_whitespace() => {}
            _ if c.is_alphanumeric() => {
                let start = i;
                let mut end = i + c.len_utf8();
                while let Some((j, c_next)) = chars.peek() {
                    if c_next.is_alphanumeric() {
                        end = j + c_next.len_utf8();
                        chars.next();
                    } else { break; }
                }
                tokens.push(Token {
                    value: &s[start..end],
                    token_type: TokenType::Word,
                    start, end,
                });
            }
            _ => {
                return Err(Diagnostic::new(
                    DiagnosticCode::TokenizerUnexpectedChar,
                    format!("Unexpected character: '{}'", c),
                    SourceLocation { line: loc.line, column: loc.column.map(|x| x + i as u32), ..Default::default() },
                ));
            }
        }
    }
    Ok(tokens)
}
```

- [ ] **Step 4: Update FieldType::from_str to thread SourceLocation**

In `tablec-core/src/core/table/field.rs`:

Replace:

```rust
impl FromStr for FieldType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens = scan_tokens(s);
        let mut peekable = tokens.into_iter().peekable();
        let field_type = parse_from_tokens(&mut peekable)?;
        if peekable.next().is_some() { Err("Extra tokens…".to_string()) } else { Ok(field_type) }
    }
}
```

with:

```rust
pub fn parse_field_type(s: &str, loc: SourceLocation) -> Result<FieldType, Diagnostic> {
    let tokens = scan_tokens(s, loc.clone())?;
    let mut peekable = tokens.into_iter().peekable();
    let field_type = parse_from_tokens(&mut peekable, &loc)?;
    if peekable.next().is_some() {
        return Err(Diagnostic::new(
            DiagnosticCode::TypeParseError,
            format!("Extra tokens at end of type definition: {}", s),
            loc,
        ));
    }
    Ok(field_type)
}

impl FromStr for FieldType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_field_type(s, SourceLocation::default())
            .map_err(|d| format!("{}: {}", d.code as u32, d.message))
    }
}
```

Update the two `parse_from_tokens` and `parse_array_type`/`parse_map_type`/`parse_struct_type` helpers — they currently return `Result<_, String>`. Change them to `Result<_, Diagnostic>` and bubble errors using a fresh `Diagnostic::new(DiagnosticCode::TypeParseError, …, loc.clone())` at each `Err(...)` site. Concretely, change the signatures and the body of each helper that returns `Result<…, String>` so that on `Err(mes)` it returns `Err(Diagnostic::new(DiagnosticCode::TypeParseError, mes, loc.clone()))`. Below is the helper-level rewrite:

```rust
fn parse_from_tokens(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    let mut base_type = parse_base_type(tokens, loc)?;
    while let Some(token) = tokens.peek() {
        if token.value == "[" {
            tokens.next();
            if let Some(next_token) = tokens.next() {
                if next_token.value != "]" {
                    return Err(Diagnostic::new(
                        DiagnosticCode::TypeParseError, "Expected ']' after '['".into(), loc.clone(),
                    ));
                }
                base_type = FieldType::Array { r#type: Box::new(base_type) };
            } else {
                return Err(Diagnostic::new(
                    DiagnosticCode::TypeParseError, "Expected ']' but found end of input".into(), loc.clone(),
                ));
            }
        } else { break; }
    }
    Ok(base_type)
}

fn parse_base_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    let token = tokens.next().ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::TypeParseError, "Unexpected end of input".into(), loc.clone())
    })?;
    match token.value {
        "array"  => parse_array_type(tokens, loc),
        "map"    => parse_map_type(tokens, loc),
        "struct" => parse_struct_type(tokens, loc),
        "int"    => Ok(FieldType::Int32),
        "int8"   => Ok(FieldType::Int8),
        "int16"  => Ok(FieldType::Int16),
        "int32"  => Ok(FieldType::Int32),
        "int64"  => Ok(FieldType::Int64),
        "uint"   => Ok(FieldType::Uint32),
        "uint8"  => Ok(FieldType::Uint8),
        "uint16" => Ok(FieldType::Uint16),
        "uint32" => Ok(FieldType::Uint32),
        "uint64" => Ok(FieldType::Uint64),
        "float" | "float32" => Ok(FieldType::Float32),
        "float64" => Ok(FieldType::Float64),
        "string" | "str" => Ok(FieldType::String),
        "bool" | "boolean" => Ok(FieldType::Bool),
        "date" => Ok(FieldType::Date),
        "datetime" => Ok(FieldType::DateTime),
        "timestamp64" => Ok(FieldType::Timestamp64),
        "timestamp32" => Ok(FieldType::Timestamp32),
        _ => Err(Diagnostic::new(
            DiagnosticCode::TypeUnknown,
            format!("Unknown type: {}", token.value), loc.clone(),
        )),
    }
}

fn parse_array_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    consume_token(tokens, "<", loc)?;
    let inner = parse_from_tokens(tokens, loc)?;
    consume_token(tokens, ">", loc)?;
    Ok(FieldType::Array { r#type: Box::new(inner) })
}

fn parse_map_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    consume_token(tokens, "<", loc)?;
    let key = parse_from_tokens(tokens, loc)?;
    consume_token(tokens, ",", loc)?;
    let value = parse_from_tokens(tokens, loc)?;
    consume_token(tokens, ">", loc)?;
    Ok(FieldType::Map { key: Box::new(key), value: Box::new(value) })
}

fn parse_struct_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    consume_token(tokens, "{", loc)?;
    let mut fields = Vec::new();
    if let Some(t) = tokens.peek() { if t.value == "}" { tokens.next(); return Ok(FieldType::Struct { fields }); } }
    loop {
        let name_token = tokens.next().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::TypeParseError, "Expected field name".into(), loc.clone())
        })?;
        if !matches!(name_token.token_type, TokenType::Word) {
            return Err(Diagnostic::new(DiagnosticCode::TypeParseError, "Expected a word for field name".into(), loc.clone()));
        }
        let field_type = if let Some(t) = tokens.peek() {
            if t.value == ":" {
                tokens.next();
                parse_from_tokens(tokens, loc)?
            } else { FieldType::String }
        } else {
            return Err(Diagnostic::new(DiagnosticCode::TypeParseError, "Unexpected end of input after field name".into(), loc.clone()));
        };
        fields.push(Field {
            name: name_token.value.to_string(),
            t: field_type,
            desc: "".to_string(), constraint: None, tags: Vec::new(),
        });
        let next = tokens.next().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::TypeParseError, "Expected '}' or ',' after field".into(), loc.clone())
        })?;
        match next.value {
            "}" => break,
            "," => continue,
            _ => return Err(Diagnostic::new(DiagnosticCode::TypeParseError, format!("Expected '}}' or ',' but got {}", next.value), loc.clone())),
        }
    }
    Ok(FieldType::Struct { fields })
}

fn consume_token(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    expected: &str,
    loc: &SourceLocation,
) -> Result<(), Diagnostic> {
    let token = tokens.next().ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::TypeParseError, format!("Expected '{}' but found end of input", expected), loc.clone())
    })?;
    if token.value != expected {
        Err(Diagnostic::new(DiagnosticCode::TypeParseError, format!("Expected '{}' but found '{}'", expected, token.value), loc.clone()))
    } else { Ok(()) }
}
```

Keep the existing field.rs `mod tests` but adjust its expected error type — `FieldType::from_str("invalid")` should now return `Err(String)` from the wrapper, so the `assert!(FieldType::from_str("invalid").is_err())` line still compiles.

Add to `field.rs` after the `FromStr` impl:

```rust
#[cfg(test)]
mod parse_field_type_tests {
    use super::*;
    use crate::core::diagnostic::*;

    #[test]
    fn parse_field_type_ok() {
        let ty = parse_field_type("int[]", SourceLocation::default()).unwrap();
        assert_eq!(ty, FieldType::Array { r#type: Box::new(FieldType::Int32) });
    }

    #[test]
    fn parse_field_type_bad_returns_diagnostic() {
        let loc = SourceLocation { sheet: Some("S".into()), line: Some(2), ..Default::default() };
        let err = parse_field_type("foo", loc.clone()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::TypeUnknown);
        assert!(err.message.contains("foo"));
        assert_eq!(err.location.sheet, Some("S".into()));
    }
}
```

- [ ] **Step 5: Remove `parser/type_parser.rs` module declaration? — NO, defer to c3**

Do not delete `tablec-core/src/core/parser/type_parser.rs` in this task. c3 deletes it. Removing it earlier leaves an unused `pub mod type_parser;` mismatch during intermediate refactors. The file is currently not imported by anything outside itself, so leaving it does no harm.

- [ ] **Step 6: Run the full tablec-core test suite**

Run: `cd repos/tablec && cargo test -p tablec-core`
Expected: all previous tests pass + new tokenizer tests + new field tests pass.

- [ ] **Step 7: Commit c2**

```bash
git -C repos/tablec add tablec-core/src/core/parser/tokenizer.rs tablec-core/src/core/table/field.rs
git -C repos/tablec -c user.name="Claude" -c user.email="claude@anthropic.com" \
    commit -m "feat(core): tokenizer returns Result; parser errors propagate as Diagnostic

Replaces panic-on-unknown-char with a Diagnostic carrying the offending
character's location. FieldType::from_str gains a loc-aware companion.
type_parser.rs is intentionally not deleted yet (c3)."
```

---

## Task 3: Value eight-width rewrite (commit c3)

**Files:**
- Modify: `tablec-core/src/core/table/value.rs`
- Modify: `tablec-core/src/core/table/types.rs`
- Modify: `tablec-core/src/core/table/field.rs` (only `to_type()`)
- Modify: `tablec-core/src/core/parser/value_parser.rs`
- Modify: `tablec-core/src/core/table/table.rs` (thread `FieldType` and `SourceLocation` through cell parsing)
- Modify: `tablec-core/src/core/table/constraint.rs` (extend `validate_sequence` and any `match` on `Value::Int/...` to cover the 10 variants)
- Modify: `tablec-core/src/export/json.rs` (delete `to_string(legacy)`)
- Modify: `tablec-core/src/export/msgpack.rs` (delete legacy `to_vec`)
- Modify: `tablec-core/src/core/parser/mod.rs` (delete `pub mod type_parser;`)
- Delete: `tablec-core/src/core/parser/type_parser.rs`

**Interfaces produced:**
- `Value::Int8(i8)`, `Int16(i16)`, `Int32(i32)`, `Int64(i64)`, `Uint8(u8)`, `Uint16(u16)`, `Uint32(u32)`, `Uint64(u64)`, `Float32(f32)`, `Float64(f64)`, `String(String)`, `Bool(bool)`, `Array(Vec<Value>)`, `Map(IndexMap<Value, Value>)`, `Struct(IndexMap<String, Value>)`, `Null`. Old `Int/Int64/Uint/Uint64/Float/Float64` aliases are gone.
- `Type::Int8 … Float64 … String … Bool … Array … Map … Struct … Any`. No `Int/Uint/Float` aliases.
- `FieldType::to_type(&self) -> Type` returns the matching typed variant.
- `parse_value(s: &str, ty: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic>` — relocated signature.
- `parse_array(s, inner_field_type, loc) -> Result<Value, Diagnostic>`
- `parse_map(s, key_field_type, value_field_type, loc) -> Result<Value, Diagnostic>`
- `parse_struct(s, fields: &[Field], loc) -> Result<Value, Diagnostic>` — by-name match (no longer positional).

Because of the size of this commit, decompose the TDD steps below into 8 sub-steps. Each sub-step must keep the suite green: failing test → minimal implementation → run → commit.

- [ ] **Step 1: New Value enum with 10 numeric variants and trait impls (DELETES old aliases)**

Replace `tablec-core/src/core/table/value.rs` with:

```rust
use indexmap::IndexMap;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub enum Value {
    Int8(i8), Int16(i16), Int32(i32), Int64(i64),
    Uint8(u8), Uint16(u16), Uint32(u32), Uint64(u64),
    Float32(f32), Float64(f64),
    String(String), Bool(bool),
    Array(Vec<Value>),
    Map(IndexMap<Value, Value>),
    Struct(IndexMap<String, Value>),
    Null,
}

fn numeric_kind(v: &Value) -> Option<u8> {
    match v {
        Value::Int8(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_)
        | Value::Uint8(_) | Value::Uint16(_) | Value::Uint32(_) | Value::Uint64(_) => Some(0),
        Value::Float32(_) | Value::Float64(_) => Some(1),
        _ => None,
    }
}

fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int8(n) => Some(*n as f64),
        Value::Int16(n) => Some(*n as f64),
        Value::Int32(n) => Some(*n as f64),
        Value::Int64(n) => Some(*n as f64),
        Value::Uint8(n) => Some(*n as f64),
        Value::Uint16(n) => Some(*n as f64),
        Value::Uint32(n) => Some(*n as f64),
        Value::Uint64(n) => Some(*n as f64),
        Value::Float32(n) => Some(*n as f64),
        Value::Float64(n) => Some(*n),
        _ => None,
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Int8(n)   => s.serialize_i8(*n),
            Value::Int16(n)  => s.serialize_i16(*n),
            Value::Int32(n)  => s.serialize_i32(*n),
            Value::Int64(n)  => s.serialize_i64(*n),
            Value::Uint8(n)  => s.serialize_u8(*n),
            Value::Uint16(n) => s.serialize_u16(*n),
            Value::Uint32(n) => s.serialize_u32(*n),
            Value::Uint64(n) => s.serialize_u64(*n),
            Value::Float32(n) => s.serialize_f32(*n),
            Value::Float64(n) => s.serialize_f64(*n),
            Value::String(v) => s.serialize_str(v),
            Value::Bool(b)   => s.serialize_bool(*b),
            Value::Array(a)  => a.serialize(s),
            Value::Struct(m) => m.serialize(s),
            Value::Null      => s.serialize_none(),
            Value::Map(m)    => {
                let mut map = s.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    let key_str = match k {
                        Value::String(st) => st.clone(),
                        Value::Int8(n) => n.to_string(),
                        Value::Int16(n) => n.to_string(),
                        Value::Int32(n) => n.to_string(),
                        Value::Int64(n) => n.to_string(),
                        Value::Uint8(n) => n.to_string(),
                        Value::Uint16(n) => n.to_string(),
                        Value::Uint32(n) => n.to_string(),
                        Value::Uint64(n) => n.to_string(),
                        Value::Float32(n) => n.to_string(),
                        Value::Float64(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),
                        _ => return Err(serde::ser::Error::custom("Map keys must be simple types")),
                    };
                    map.serialize_entry(&key_str, v)?;
                }
                map.end()
            }
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        // Same-variant exact compare.
        if core::mem::discriminant(self) != core::mem::discriminant(other) { return false; }
        match (self, other) {
            (Value::Int8(a),   Value::Int8(b))   => a == b,
            (Value::Int16(a),  Value::Int16(b))  => a == b,
            (Value::Int32(a),  Value::Int32(b))  => a == b,
            (Value::Int64(a),  Value::Int64(b))  => a == b,
            (Value::Uint8(a),  Value::Uint8(b))  => a == b,
            (Value::Uint16(a), Value::Uint16(b)) => a == b,
            (Value::Uint32(a), Value::Uint32(b)) => a == b,
            (Value::Uint64(a), Value::Uint64(b)) => a == b,
            (Value::Float32(a), Value::Float32(b)) => (a - b).abs() < f32::EPSILON,
            (Value::Float64(a), Value::Float64(b)) => (a - b).abs() < f64::EPSILON,
            (Value::String(a), Value::String(b))   => a == b,
            (Value::Bool(a),   Value::Bool(b))     => a == b,
            (Value::Array(a),  Value::Array(b))    => a == b,
            (Value::Map(a),    Value::Map(b))      => a == b,
            (Value::Struct(a), Value::Struct(b))   => a == b,
            (Value::Null,      Value::Null)        => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Value::Int8(n)   => n.hash(state),
            Value::Int16(n)  => n.hash(state),
            Value::Int32(n)  => n.hash(state),
            Value::Int64(n)  => n.hash(state),
            Value::Uint8(n)  => n.hash(state),
            Value::Uint16(n) => n.hash(state),
            Value::Uint32(n) => n.hash(state),
            Value::Uint64(n) => n.hash(state),
            Value::Float32(n) => n.to_bits().hash(state),
            Value::Float64(n) => n.to_bits().hash(state),
            Value::String(s) => s.hash(state),
            Value::Bool(b)   => b.hash(state),
            Value::Array(a)  => a.hash(state),
            Value::Map(m)    => { for (k, v) in m { k.hash(state); v.hash(state); } }
            Value::Struct(s) => { for (k, v) in s { k.hash(state); v.hash(state); } }
            Value::Null      => 0u8.hash(state),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Value::*;
        match (self, other) {
            (Int8(a),   Int8(b))   => a.partial_cmp(b),
            (Int16(a),  Int16(b))  => a.partial_cmp(b),
            (Int32(a),  Int32(b))  => a.partial_cmp(b),
            (Int64(a),  Int64(b))  => a.partial_cmp(b),
            (Uint8(a),  Uint8(b))  => a.partial_cmp(b),
            (Uint16(a), Uint16(b)) => a.partial_cmp(b),
            (Uint32(a), Uint32(b)) => a.partial_cmp(b),
            (Uint64(a), Uint64(b)) => a.partial_cmp(b),
            (Float32(a), Float32(b)) => a.partial_cmp(b),
            (Float64(a), Float64(b)) => a.partial_cmp(b),
            // Cross-width Int↔Int narrow→wide promotes losslessly (parse-time range-checked).
            (Int8(a),  Int16(b))  => Some(Int16(*a as i16).partial_cmp(b)),
            (Int16(a), Int8(b))   => Some(a.partial_cmp(&Int16(*b as i16))),
            (Int8(a),  Int32(b))  => Some(Int32(*a as i32).partial_cmp(b)),
            (Int32(a), Int8(b))   => Some(a.partial_cmp(&Int32(*b as i32))),
            (Int8(a),  Int64(b))  => Some(Int64(*a as i64).partial_cmp(b)),
            (Int64(a), Int8(b))   => Some(a.partial_cmp(&Int64(*b as i64))),
            (Int16(a), Int32(b))  => Some(Int32(*a as i32).partial_cmp(b)),
            (Int32(a), Int16(b))  => Some(a.partial_cmp(&Int32(*b as i32))),
            (Int16(a), Int64(b))  => Some(Int64(*a as i64).partial_cmp(b)),
            (Int64(a), Int16(b))  => Some(a.partial_cmp(&Int64(*b as i64))),
            (Int32(a), Int64(b))  => Some(Int64(*a as i64).partial_cmp(b)),
            (Int64(a), Int32(b))  => Some(a.partial_cmp(&Int64(*b as i32))),
            // Cross-family via i128.
            (Int8(a),   Uint8(b))  => Some(0i128.partial_cmp(&(*b as i128 - *a as i128))),
            (Uint8(a),  Int8(b))   => Some(0i128.partial_cmp(&(*a as i128 - *b as i128))),
            (Int8(a),   Uint16(b)) => Some(0i128.partial_cmp(&(*b as i128 - *a as i128))),
            (Uint16(a), Int8(b))   => Some(0i128.partial_cmp(&(*a as i128 - *b as i128))),
            // …similar deltas for the rest of int/uint width pairs — implement using a small helper.
            // Float↔Float cross-width.
            (Float32(a), Float64(b)) => Some((*a as f64).partial_cmp(b)),
            (Float64(a), Float32(b)) => Some(a.partial_cmp(&(*b as f64))),
            // Float↔Int/uint via f64 widening.
            (a, b) if numeric_kind(a).is_some() && numeric_kind(b).is_some() => {
                let (av, bv) = match (to_f64(a), to_f64(b)) { (Some(x), Some(y)) => (x, y), _ => return None };
                av.partial_cmp(&bv)
            }
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int8(n)   => write!(f, "{}", n),
            Value::Int16(n)  => write!(f, "{}", n),
            Value::Int32(n)  => write!(f, "{}", n),
            Value::Int64(n)  => write!(f, "{}", n),
            Value::Uint8(n)  => write!(f, "{}", n),
            Value::Uint16(n) => write!(f, "{}", n),
            Value::Uint32(n) => write!(f, "{}", n),
            Value::Uint64(n) => write!(f, "{}", n),
            Value::Float32(n) => write!(f, "{}", n),
            Value::Float64(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "'{}'", s),
            Value::Bool(b)   => write!(f, "{}", b),
            Value::Null      => write!(f, "null"),
            Value::Array(a)  => { write!(f, "[")?; for (i, x) in a.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}", x)?; } write!(f, "]") }
            Value::Map(m)    => { write!(f, "{{")?; for (i, (k, v)) in m.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
            Value::Struct(s) => { write!(f, "{{")?; for (i, (k, v)) in s.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
        }
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("a valid Value") }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> { Ok(Value::Bool(v)) }
    fn visit_i8<E: de::Error>(self, v: i8) -> Result<Value, E> { Ok(Value::Int8(v)) }
    fn visit_i16<E: de::Error>(self, v: i16) -> Result<Value, E> { Ok(Value::Int16(v)) }
    fn visit_i32<E: de::Error>(self, v: i32) -> Result<Value, E> { Ok(Value::Int32(v)) }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> { Ok(Value::Int64(v)) }
    fn visit_u8<E: de::Error>(self, v: u8) -> Result<Value, E> { Ok(Value::Uint8(v)) }
    fn visit_u16<E: de::Error>(self, v: u16) -> Result<Value, E> { Ok(Value::Uint16(v)) }
    fn visit_u32<E: de::Error>(self, v: u32) -> Result<Value, E> { Ok(Value::Uint32(v)) }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> { Ok(Value::Uint64(v)) }
    fn visit_f32<E: de::Error>(self, v: f32) -> Result<Value, E> { Ok(Value::Float32(v)) }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> { Ok(Value::Float64(v)) }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> { Ok(Value::String(v.to_string())) }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> { Ok(Value::String(v)) }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> { Ok(Value::Null) }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> { d.deserialize_any(self) }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut v = Vec::new();
        while let Some(x) = seq.next_element()? { v.push(x); }
        Ok(Value::Array(v))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut m = IndexMap::new();
        while let Some((k, v)) = access.next_entry::<String, Value>()? { m.insert(k, v); }
        Ok(Value::Struct(m))
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> { d.deserialize_any(ValueVisitor) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_size_is_sixteen_variants() {
        // Lock the variants; bump ONLY when adding/removing a variant.
        let mut seen = std::collections::HashSet::new();
        seen.insert(std::mem::discriminant(&Value::Int8(0)));
        seen.insert(std::mem::discriminant(&Value::Int16(0)));
        seen.insert(std::mem::discriminant(&Value::Int32(0)));
        seen.insert(std::mem::discriminant(&Value::Int64(0)));
        seen.insert(std::mem::discriminant(&Value::Uint8(0)));
        seen.insert(std::mem::discriminant(&Value::Uint16(0)));
        seen.insert(std::mem::discriminant(&Value::Uint32(0)));
        seen.insert(std::mem::discriminant(&Value::Uint64(0)));
        seen.insert(std::mem::discriminant(&Value::Float32(0.0)));
        seen.insert(std::mem::discriminant(&Value::Float64(0.0)));
        seen.insert(std::mem::discriminant(&Value::String(String::new())));
        seen.insert(std::mem::discriminant(&Value::Bool(false)));
        seen.insert(std::mem::discriminant(&Value::Array(vec![])));
        seen.insert(std::mem::discriminant(&Value::Map(IndexMap::new())));
        seen.insert(std::mem::discriminant(&Value::Struct(IndexMap::new())));
        seen.insert(std::mem::discriminant(&Value::Null));
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn cross_width_partial_ord_promotes() {
        assert!(Value::Int8(-1) < Value::Uint8(1));
        assert!(Value::Int32(1) < Value::Int64(2));
        // Float vs Int cross-family:
        assert!(Value::Float32(1.5) > Value::Int32(1));
    }

    #[test]
    fn hash_includes_discriminant() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        Value::Int32(0).hash(&mut h1);
        Value::Uint32(0).hash(&mut h2);
        assert_ne!(h1.finish(), h2.finish(), "Int32(0) and Uint32(0) must hash differently");
    }

    #[test]
    fn serialize_each_numeric_variant() {
        let cases = vec![
            (Value::Int8(-1),   "-1"),
            (Value::Int16(-1),  "-1"),
            (Value::Int32(-1),  "-1"),
            (Value::Int64(-1),  "-1"),
            (Value::Uint8(1),   "1"),
            (Value::Uint16(1),  "1"),
            (Value::Uint32(1),  "1"),
            (Value::Uint64(1),  "1"),
            (Value::Float32(1.5), "1.5"),
            (Value::Float64(1.5), "1.5"),
        ];
        for (v, expected) in cases {
            let s = serde_json::to_string(&v).unwrap();
            assert_eq!(s, expected, "variant {:?}", v);
        }
    }
}
```

- [ ] **Step 2: New Type enum without Int/Uint/Float aliases**

Replace `tablec-core/src/core/table/types.rs`:

```rust
#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Int8, Int16, Int32, Int64,
    Uint8, Uint16, Uint32, Uint64,
    Float32, Float64,
    String, Bool,
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Struct(std::collections::HashMap<String, Type>),
    Any,
}
```

- [ ] **Step 3: Update FieldType::to_type to be width-aware**

In `tablec-core/src/core/table/field.rs`, replace the body of `to_type`:

```rust
pub fn to_type(&self) -> Type {
    match self {
        FieldType::Int8 => Type::Int8,
        FieldType::Int16 => Type::Int16,
        FieldType::Int32 => Type::Int32,
        FieldType::Int64 => Type::Int64,
        FieldType::Int  => Type::Int32,
        FieldType::Uint8 => Type::Uint8,
        FieldType::Uint16 => Type::Uint16,
        FieldType::Uint32 => Type::Uint32,
        FieldType::Uint64 => Type::Uint64,
        FieldType::Uint  => Type::Uint32,
        FieldType::Float32 => Type::Float32,
        FieldType::Float64 => Type::Float64,
        FieldType::Float => Type::Float32,
        FieldType::String => Type::String,
        FieldType::Bool => Type::Bool,
        FieldType::Date | FieldType::DateTime
            | FieldType::Timestamp32 | FieldType::Timestamp64 => Type::String,
        FieldType::Array { r#type } => Type::Array(Box::new(r#type.to_type())),
        FieldType::Map { key, value } => Type::Map(Box::new(key.to_type()), Box::new(value.to_type())),
        FieldType::Struct { fields } => {
            let mut m = std::collections::HashMap::new();
            for f in fields { m.insert(f.name.clone(), f.t.to_type()); }
            Type::Struct(m)
        }
    }
}
```

Add a unit test in `mod tests` of `field.rs`:

```rust
#[test]
fn to_type_preserves_width() {
    assert_eq!(FieldType::Int8.to_type(), Type::Int8);
    assert_eq!(FieldType::Uint16.to_type(), Type::Uint16);
    assert_eq!(FieldType::Float64.to_type(), Type::Float64);
}
```

- [ ] **Step 4: Rewrite value_parser.rs to take FieldType + SourceLocation + by-name struct**

Replace `tablec-core/src/core/parser/value_parser.rs`:

```rust
use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
use crate::core::table::field::FieldType;
use crate::core::table::value::Value;
use crate::core::table::value::Value::*;
use indexmap::IndexMap;

pub fn parse_value(s: &str, ty: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic> {
    fn parse_basic(s: &str, ty: &FieldType, loc: &SourceLocation) -> Result<Value, Diagnostic> {
        let trimmed = s.trim();
        match ty {
            FieldType::Int8   => trimmed.parse::<i8>().map(Int8).map_err(|e| out_of_range(trimmed, "int8",   -128, 127,   loc)),
            FieldType::Int16  => trimmed.parse::<i16>().map(Int16).map_err(|e| out_of_range(trimmed, "int16",  i16::MIN as i128, i16::MAX as i128, loc)),
            FieldType::Int32  => trimmed.parse::<i32>().map(Int32).map_err(|e| out_of_range(trimmed, "int32",  i32::MIN as i128, i32::MAX as i128, loc)),
            FieldType::Int64  => trimmed.parse::<i64>().map(Int64).map_err(|e| out_of_range(trimmed, "int64",  i64::MIN as i128, i64::MAX as i128, loc)),
            FieldType::Int    => trimmed.parse::<i32>().map(Int32).map_err(|e| out_of_range(trimmed, "int",    i32::MIN as i128, i32::MAX as i128, loc)),
            FieldType::Uint8  => trimmed.parse::<u8>().map(Uint8).map_err(|e| out_of_range(trimmed, "uint8",  0, u8::MAX  as i128, loc)),
            FieldType::Uint16 => trimmed.parse::<u16>().map(Uint16).map_err(|e| out_of_range(trimmed, "uint16", 0, u16::MAX as i128, loc)),
            FieldType::Uint32 => trimmed.parse::<u32>().map(Uint32).map_err(|e| out_of_range(trimmed, "uint32", 0, u32::MAX as i128, loc)),
            FieldType::Uint64 => trimmed.parse::<u64>().map(Uint64).map_err(|e| out_of_range(trimmed, "uint64", 0, u64::MAX as i128, loc)),
            FieldType::Uint   => trimmed.parse::<u32>().map(Uint32).map_err(|e| out_of_range(trimmed, "uint",   0, u32::MAX as i128, loc)),
            FieldType::Float32=> trimmed.parse::<f32>().map(Float32).map_err(|e| parse_fail(trimmed, "float32", loc)),
            FieldType::Float64=> trimmed.parse::<f64>().map(Float64).map_err(|e| parse_fail(trimmed, "float64", loc)),
            FieldType::Float  => trimmed.parse::<f32>().map(Float32).map_err(|e| parse_fail(trimmed, "float", loc)),
            FieldType::Bool   => match trimmed.to_lowercase().as_str() {
                "true" | "1" => Ok(Value::Bool(true)),
                "false" | "0" => Ok(Value::Bool(false)),
                _ => Err(parse_fail(trimmed, "bool", loc)),
            },
            _ => Err(Diagnostic::new(DiagnosticCode::TypeParseError, format!("Unsupported basic type for: {}", ty), loc.clone())),
        }
    }

    fn out_of_range(s: &str, ty: &str, lo: i128, hi: i128, loc: &SourceLocation) -> Diagnostic {
        // Distinguish between genuinely out-of-range and non-numeric via a quick attempt.
        if s.trim().parse::<f64>().is_err() {
            parse_fail(s, ty, loc)
        } else {
            Diagnostic::new(
                DiagnosticCode::ValueOutOfRange,
                format!("value '{}' not in {} range [{}, {}]", s, ty, lo, hi),
                loc.clone(),
            )
        }
    }
    fn parse_fail(s: &str, ty: &str, loc: &SourceLocation) -> Diagnostic {
        Diagnostic::new(DiagnosticCode::ValueParseError, format!("cannot parse '{}' as {}", s, ty), loc.clone())
    }

    match ty {
        FieldType::String => {
            let trimmed = s.trim();
            if (trimmed.starts_with('\'') && trimmed.ends_with('\'')) || (trimmed.starts_with('"') && trimmed.ends_with('"')) {
                Ok(String(trimmed[1..trimmed.len()-1].to_string()))
            } else {
                Ok(String(trimmed.to_string()))
            }
        }
        FieldType::Date | FieldType::DateTime
            | FieldType::Timestamp32 | FieldType::Timestamp64 => parse_basic(s, &FieldType::String, loc),
        _ if !is_compound(ty) => parse_basic(s, ty, loc),
        FieldType::Array { r#type } => parse_array(s, r#type, loc),
        FieldType::Map { key, value } => parse_map(s, key, value, loc),
        FieldType::Struct { fields } => parse_struct(s, fields, loc),
    }
}

fn is_compound(ty: &FieldType) -> bool {
    matches!(ty, FieldType::Array { .. } | FieldType::Map { .. } | FieldType::Struct { .. })
}

fn parse_array(s: &str, inner: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic> {
    let t = s.trim();
    if !t.starts_with('[') || !t.ends_with(']') {
        return Err(Diagnostic::new(DiagnosticCode::ValueParseError, "Invalid array format (need [a, b, …])".into(), loc));
    }
    let inner_str = &t[1..t.len()-1];
    let mut values = Vec::new();
    let mut level = 0; let mut start = 0;
    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => { values.push(parse_value(inner_str[start..i].trim(), inner, loc.clone())?); start = i + 1; }
            _ => {}
        }
    }
    if start < inner_str.len() {
        values.push(parse_value(inner_str[start..].trim(), inner, loc.clone())?);
    }
    Ok(Value::Array(values))
}

fn parse_map(s: &str, key: &FieldType, value: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic> {
    let t = s.trim();
    let mut out: IndexMap<Value, Value> = IndexMap::new();
    if t.is_empty() { return Ok(Value::Map(out)); }
    let mut level = 0; let mut start = 0; let mut pairs: Vec<&str> = vec![];
    for (i, c) in t.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => { pairs.push(t[start..i].trim()); start = i + 1; }
            _ => {}
        }
    }
    if start < t.len() { pairs.push(t[start..].trim()); }
    for pair in pairs {
        let parts: Vec<&str> = pair.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Diagnostic::new(DiagnosticCode::ValueParseError, format!("Invalid map pair: {}", pair), loc.clone()));
        }
        let k = parse_value(parts[0], key, loc.clone())?;
        let v = parse_value(parts[1], value, loc.clone())?;
        out.insert(k, v);
    }
    Ok(Value::Map(out))
}

fn parse_struct(s: &str, fields: &[crate::core::table::field::Field], loc: SourceLocation) -> Result<Value, Diagnostic> {
    let t = s.trim();
    if !t.starts_with('{') || !t.ends_with('}') {
        return Err(Diagnostic::new(DiagnosticCode::ValueParseError, "Invalid struct format (need {a: x, b: y})".into(), loc));
    }
    let inner_str = &t[1..t.len()-1];

    // Token-stream by-name: parse each field by walking the comma-split list and matching field names.
    // Use JSON-ish style "name: value" — see spec §4.4 note about struct by-name matching.
    let mut fields_str = Vec::new();
    let mut level = 0; let mut start = 0;
    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => { fields_str.push(&inner_str[start..i]); start = i + 1; }
            _ => {}
        }
    }
    if start < inner_str.len() { fields_str.push(&inner_str[start..]); }

    let mut m: IndexMap<String, Value> = IndexMap::new();
    for chunk in fields_str {
        let chunk = chunk.trim();
        if chunk.is_empty() { continue; }
        let parts: Vec<&str> = chunk.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Diagnostic::new(DiagnosticCode::StructFieldMismatch,
                format!("expected 'name: value' but got '{}'", chunk), loc.clone()));
        }
        let name = parts[0].trim().to_string();
        let field = match fields.iter().find(|f| f.name == name) {
            Some(f) => f,
            None => return Err(Diagnostic::new(DiagnosticCode::StructFieldMismatch,
                format!("unknown struct field '{}'", name), loc.clone())),
        };
        let v = parse_value(parts[1].trim(), &field.t, loc.clone())?;
        m.insert(name, v);
    }
    // Check all declared fields were seen.
    for f in fields {
        if !m.contains_key(&f.name) {
            return Err(Diagnostic::new(DiagnosticCode::StructFieldCountMismatch,
                format!("struct missing field '{}'", f.name), loc.clone()));
        }
    }
    Ok(Value::Struct(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::{Field, FieldType};

    fn loc() -> SourceLocation { SourceLocation::default() }

    #[test]
    fn parse_int_each_width_ok() {
        assert_eq!(parse_value("42", &FieldType::Int8,  loc()).unwrap(), Value::Int8(42));
        assert_eq!(parse_value("42", &FieldType::Int16, loc()).unwrap(), Value::Int16(42));
        assert_eq!(parse_value("42", &FieldType::Int64, loc()).unwrap(), Value::Int64(42));
        assert_eq!(parse_value("42", &FieldType::Uint8, loc()).unwrap(), Value::Uint8(42));
    }

    #[test]
    fn parse_int_out_of_range_yields_diagnostic() {
        let err = parse_value("200", &FieldType::Int8, loc()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::ValueOutOfRange);
        assert!(err.message.contains("int8"));
        assert!(err.message.contains("["));
    }

    #[test]
    fn parse_non_numeric_yields_parse_error() {
        let err = parse_value("abc", &FieldType::Int32, loc()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::ValueParseError);
    }

    #[test]
    fn parse_struct_by_name_matches_declared_fields() {
        let f1 = Field { name: "a".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] };
        let f2 = Field { name: "b".into(), t: FieldType::String, desc: "".into(), constraint: None, tags: vec![] };
        let fields = vec![f1, f2];
        // Order in text doesn't match declaration order, but by-name matches.
        let v = parse_value("{b: hi, a: 7}", &FieldType::Struct { fields: fields.clone() }, loc()).unwrap();
        match v {
            Value::Struct(m) => {
                assert_eq!(m.get("a"), Some(&Value::Int32(7)));
                assert_eq!(m.get("b"), Some(&Value::String("hi".into())));
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn parse_struct_missing_field_reports_count_mismatch() {
        let fields = vec![
            Field { name: "a".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
            Field { name: "b".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
        ];
        let err = parse_value("{a: 1}", &FieldType::Struct { fields }, loc()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::StructFieldCountMismatch);
    }
}
```

- [ ] **Step 5: Update table.rs to thread FieldType + loc through per-cell parsing**

Replace the body of `read_excel` in `tablec-core/src/core/table/table.rs`. The relevant signature change is the inner loop where `parse_value_from_str(&cell_value_str, &field.t.to_type())` becomes `parse_value(&cell_value_str, &field.t, cell_loc)`. The `cell_loc` is constructed:

```rust
let cell_loc = SourceLocation {
    file: Some(std::path::PathBuf::from(fpath)),
    sheet: Some(sheet_name.clone()),
    line: Some(row_index as u32 + 5),  // row 1-4 reserved, data starts at row 5 (0-indexed adjustment + 5)
    column: Some(col_index as u32 + 1),
};
let value = crate::core::parser::value_parser::parse_value(&cell_value_str, &field.t, cell_loc)?;
```

Add `use crate::core::parser::value_parser::parse_value;` at the top of `table.rs`.

Note: `read_excel`'s outer return type is still `Result<Vec<Table>, Box<dyn Error>>` at this point — it must stay that way until c4. Use `?` on per-cell errors: on the first error, return early. (In c4 this becomes aggregation.)

Add a unit test in `table.rs`:

```rust
#[test]
fn out_of_range_cell_yields_clear_error() {
    // Generate an in-memory workbook-like construction? Tests of read_excel
    // require real .xlsx files; defer detailed xlsx tests to error_cases fixtures (c4).
}
```

(The body is intentionally a no-op; real fixture tests land in c4.)

- [ ] **Step 6: Delete type_parser.rs**

Edit `tablec-core/src/core/parser/mod.rs`:

```rust
pub mod tokenizer;
pub mod value_parser;
```

Run: `git rm repos/tablec/tablec-core/src/core/parser/type_parser.rs`

Run: `cd repos/tablec && cargo build -p tablec-core`
Expected: no error. `parser/type_parser.rs` is now gone; nothing referenced it after c2.

- [ ] **Step 7: Update constraint.rs match arms to 10 numeric variants**

In `tablec-core/src/core/table/constraint.rs`, the `validate_sequence` function currently matches `Value::Int(n)` and `Value::Uint(n)`. After c3, replace with:

```rust
use crate::core::table::value::Value::*;

fn numeric_i64(v: &Value) -> Option<i64> {
    match v {
        Int8(n)  => Some(*n as i64),
        Int16(n) => Some(*n as i64),
        Int32(n) => Some(*n as i64),
        Int64(n) => Some(*n),
        Uint8(n)  => Some(*n as i64),
        Uint16(n) => Some(*n as i64),
        // Uint32 may exceed i64 for top bit set; treat as overflow and reject.
        Uint32(n) if *n <= i64::MAX as u32 => Some(*n as i64),
        Uint64(n) if *n <= i64::MAX as u64 => Some(*n as i64),
        Uint32(_) | Uint64(_) => None,
        _ => None,
    }
}
```

Inside `validate_sequence`, replace:

```rust
match value {
    Value::Int(n) => { if *n != expected_value { return Err(...); } }
    Value::Uint(n) => { if *n != expected_value as u64 { return Err(...); } }
    _ => return Err("@seq requires numeric".into()),
}
```

with:

```rust
let n = numeric_i64(value).ok_or_else(|| format!("@seq requires numeric field '{}'", field_name))?;
if n != expected_value { return Err(format!("expected {} at row {} but found {}", expected_value, row_index + 1, n)); }
```

Add unit tests:

```rust
#[test]
fn validate_sequence_handles_each_width() {
    let c = Constraint::from_str("@seq").unwrap();
    let fields = vec![Field { name: "n".into(), t: FieldType::Int16, desc: "".into(), constraint: Some(c.clone()), tags: vec![] }];
    let rows = vec![
        Row::from_vec(vec![("n".into(), Value::Int16(1))]),
        Row::from_vec(vec![("n".into(), Value::Int16(2))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}
```

- [ ] **Step 8: Delete legacy `Json::to_string(legacy)` and `Msgpack::legacy to_vec`**

In `export/json.rs`, remove the trailing `to_string` function (lines ~56-69).

In `export/msgpack.rs`, remove the trailing legacy `to_vec` function (lines ~25-29).

Update `binding-python/src/lib.rs` calls to use the new API (c3.1 task handles this; for c3 build, ensure the call sites compile by replacing `export::json::to_string(...)` with something that uses the new return type — temporarily use `format!("{:?}", v)` if needed — but the binding-python revamp is its own commit c3.1). To keep c3 green on `tablec-core`, the binding-python caller is rewritten in c3.1, NOT c3. **However**, **c3 must include a small binding-python emergency fix to keep `cargo build` green** — at minimum swap the old `Box<dyn Error>` `read_excel` result to the new error type (since task 5 rewrites `read_excel` next, you stay backward-compatible by treating the error `Box<dyn Error>` as a `String` mapping).

Concretely, do this as part of c3 step 9 (below) — c3 covers binding-python compat only.

- [ ] **Step 9: c3 binding-python compatibility shim (no API extension, just keep it compiling)**

In `tablec-core/src/core/table/table.rs`, after the rewrite, keep `read_excel`'s outer signature as:

```rust
pub fn read_excel(fpath: &str) -> Result<Vec<Table>, Box<dyn std::error::Error>>
```

by wrapping the per-cell `Diagnostic` in a one-line `String`:

```rust
fn diag_to_box(e: Diagnostic) -> Box<dyn std::error::Error> { Box::new(e.to_string()) }
```

… applied at each `?` site. (c4 will change this in one go.)

In `binding-python/src/lib.rs`, **keep current behavior**: `read_excel(input).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))`. The `e.to_string()` is via `Box<dyn Error>`'s Display — `Diagnostic`'s Display impl already produces a useful message (already in c1).

- [ ] **Step 10: Run full tablec-core tests**

Run: `cd repos/tablec && cargo test -p tablec-core`
Expected: all tests pass. **Manually fix any new failures** (typically: missing match arm on Value numeric variants; spec §10 highlights c3 as the largest commit).

Run tablec-testsuite to detect any output drift:

```bash
cd /home/bot/workbench/repos/tablec-testsuite && pytest -q
```

Expected: If any test asserts exact JSON byte shape, snapshot review required. Otherwise green.

- [ ] **Step 11: Commit c3 + c3.1 binding-python compat shim (single combined commit)**

```bash
git -C repos/tablec add -A
git -C repos/tablec -c user.name="Claude" -c user.email="claude@anthropic.com" \
    commit -m "feat(core): Value enum rewritten with 10 numeric variants

Each Rust integer/float width is a distinct variant. parse_value
performs parse-time range checks and emits ValueOutOfRange or
ValueParseError Diagnostics on mismatch. Old Int/Int64/Uint/Uint64/Float
aliases are removed (compat policy allows breaking). type_parser.rs
deleted (was zombie code). Constraint validator updated to convert any
numeric width to i64 for @seq.

Includes binding-python compatibility shim: callers continue to receive
Box<dyn Error>; full re-exposure of new enum variants is planned in a
follow-up.

See spec §4 and plan task 3."
```

---

## Task 4: read_excel returns Vec<Diagnostic> (commit c4)

**Files:**
- Modify: `tablec-core/src/core/table/table.rs` — change `read_excel` signature; aggregate errors
- Modify: `tablec-cli/src/cmd/build.rs` — call-site updates
- Modify: `tablec-cli/src/cmd/check.rs` — call-site updates
- Modify: `binding-python/src/lib.rs` — adjust error mapping
- Create: `tablec-core/tests/fixtures/error_cases/bad_int_range.xlsx`
- Create: `tablec-core/tests/fixtures/error_cases/bad_struct_field.xlsx`

**Interfaces produced:**
- `pub fn read_excel(path: &str) -> Result<Vec<Table>, Vec<Diagnostic>>` — boundary error type.

- [ ] **Step 1: Write a fixture-driven test expecting aggregated errors**

Add to `tablec-core/tests/diagnostic_export.rs`:

```rust
#[test]
fn read_excel_propagates_diagnostics_on_bad_value() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/error_cases/bad_int_range.xlsx");
    let errs = tablec_core::core::table::table::read_excel(path.to_str().unwrap())
        .err()
        .expect("expected Err");
    let codes: std::collections::HashSet<_> = errs.iter().map(|d| d.code).collect();
    assert!(codes.contains(&tablec_core::core::diagnostic::DiagnosticCode::ValueOutOfRange));
}
```

- [ ] **Step 2: Create the bad fixture manually with rust_xlsxwriter (in test code) and save it**

Add a one-shot helper in `tablec-core/tests/fixtures/error_cases/build_bad_int_range.rs` (this is its own `examples/` style program, invoked only by maintainers):

```rust
use rust_xlsxwriter::*;
fn main() {
    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet();
    sheet.write_string(0, 0, "id").unwrap();     // row 1: name
    sheet.write_string(1, 0, "int8").unwrap();    // row 2: type
    sheet.write_string(2, 0, "id").unwrap();      // row 3: comment
    sheet.write_string(3, 0, "").unwrap();        // row 4: empty
    sheet.write_string(4, 0, "").unwrap();        // row 5: empty
    sheet.write_number(5, 0, 200).unwrap();       // row 6: data — out of int8 range
    wb.save("tests/fixtures/error_cases/bad_int_range.xlsx").unwrap();
}
```

Register the example in `tablec-core/Cargo.toml` (add immediately after the existing `[[bench]]` block):

```toml
[[example]]
name = "build_bad_int_range"
path = "tests/fixtures/error_cases/build_bad_int_range.rs"
```

Run: `cd repos/tablec && cargo run --example build_bad_int_range --manifest-path tablec-core/Cargo.toml`
Expected: file written.

- [ ] **Step 3: Change read_excel signature, aggregate errors**

In `tablec-core/src/core/table/table.rs`, change:

```rust
pub fn read_excel(fpath: &str) -> Result<Vec<Table>, Box<dyn std::error::Error>>
```

to:

```rust
pub fn read_excel(fpath: &str) -> Result<Vec<Table>, Vec<Diagnostic>>
```

Inside, accumulate into a `let mut diagnostics: Vec<Diagnostic> = vec![];`. The per-cell parse becomes `match parse_value(...) { Ok(v) => new_row.add_field(...), Err(d) => diagnostics.push(d) }`. Skip rows with all-empty cells as today. After all sheets processed, return `if diagnostics.is_empty() { Ok(tables) } else { Err(diagnostics) }`.

- [ ] **Step 4: Update CLI call sites**

In `tablec-cli/src/cmd/build.rs`, replace each `?` on `read_excel(...)` with:

```rust
let tables = match read_excel(input) {
    Ok(t) => t,
    Err(errs) => {
        for d in errs { eprintln!("{}", d); }
        return Err(format!("read_excel failed with {} diagnostics", errs.len()).into());
    }
};
```

Apply at lines 116, 143, 168 (existing match arm `read_excel(file_path.to_str().unwrap())?`).

In `tablec-cli/src/cmd/check.rs`, do similarly at line 72.

- [ ] **Step 5: Update binding-python call site**

In `binding-python/src/lib.rs`, change to:

```rust
fn read_excel_or_pyerr(input: &str) -> PyResult<Vec<Table>> {
    tablec_core::core::table::table::read_excel(input).map_err(|errs| {
        let msg = errs.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        pyo3::exceptions::PyValueError::new_err(msg)
    })
}
```

…and call `read_excel_or_pyerr` in both `build` and `check`.

- [ ] **Step 6: Run tablec-core suite + tablec-cli + binding-python**

```bash
cd repos/tablec && cargo build --workspace && cargo test -p tablec-core
cd /home/bot/workbench/repos/tablec-testsuite && pytest -q
```

Expected: all green. If `testsuite` flags snapshot drift, **DO NOT auto-update** — review each diff first (per spec §8.2 c4 row).

- [ ] **Step 7: Commit c4**

```bash
git -C repos/tablec add -A
git -C repos/tablec -c user.name="Claude" -c user.email="claude@anthropic.com" \
    commit -m "feat(core): read_excel returns Vec<Diagnostic> on error

Errors from per-cell parsing are aggregated and returned as a Vec.
CLI prints each diagnostic via the Display impl, then bubbles a single
error to the main function. Binding-python joins diagnostic messages
with newlines and returns one PyValueError.

Bad-fixture tests added under tests/fixtures/error_cases/.

See spec §3.3 and plan task 4."
```

---

## Task 5: table-level constraints from row 5 (commit c5)

**Files:**
- Modify: `tablec-core/src/core/table/constraint.rs` — add `location`, validate errors return `Diagnostic`
- Modify: `tablec-core/src/core/table/table.rs` — parse row 5; `validate_table` returns `Result<(), Vec<Diagnostic>>`
- Modify: `tablec-core/src/core/table/validator.rs` (if validator exists as separate file) — update calls
- Create: `tablec-core/tests/fixtures/error_cases/bad_unique_constraint.xlsx`

**Interfaces produced:**
- `Constraint { func: String, args: Vec<String>, location: SourceLocation }` — `location` field added.
- `pub fn Constraint::from_str_with_loc(s: &str, loc: SourceLocation) -> Result<Self, Diagnostic>` — error path now Diagnostic, code `TableConstraintParseError`.
- `pub fn Table::validate_constraints(&self) -> Result<(), Vec<Diagnostic>>`

- [ ] **Step 1: Add `location` field and rewrite to_diagnostic**

In `constraint.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Constraint {
    pub func: String,
    pub args: Vec<String>,
    pub location: SourceLocation,
}

impl Constraint {
    pub fn from_str_with_loc(s: &str, loc: SourceLocation) -> Result<Self, Diagnostic> {
        if !s.starts_with('@') {
            return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                "constraint must start with @".into(), loc));
        }
        let body = &s[1..];
        let (func, args) = if let Some(idx) = body.find('(') {
            let f = body[..idx].trim();
            if f.is_empty() { return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                "empty function name".into(), loc)); }
            let arg_str = &body[idx+1..body.len()-1];
            let args: Vec<String> = if arg_str.trim().is_empty() { vec![] } else {
                arg_str.split(',').map(|s| s.trim().to_string()).collect()
            };
            (f.to_string(), args)
        } else {
            if body.trim().is_empty() { return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                "empty function name".into(), loc)); }
            (body.trim().to_string(), vec![])
        };
        Ok(Self { func, args, location: loc })
    }
}
```

Keep `impl FromStr for Constraint { type Err = (); ... }` as a wrapper around `from_str_with_loc` (returns Ok ignoring error) — this preserves backward compat with the existing `Constraint::from_str` callers. The wrapper:

```rust
impl FromStr for Constraint {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Constraint::from_str_with_loc(s, SourceLocation::default()).map_err(|_| ())
    }
}
```

Add `to_diagnostic(&self, msg: &str) -> Diagnostic`:

```rust
pub fn to_diagnostic(&self, msg: &str) -> Diagnostic {
    let code = match self.func.as_str() {
        "unique" => DiagnosticCode::ConstraintDuplicate,
        "seq"    => DiagnosticCode::ConstraintSequenceBroken,
        "order"  => DiagnosticCode::ConstraintOrderViolation,
        _        => DiagnosticCode::ConstraintUnknown,
    };
    Diagnostic::new(code, format!("@{}{}: {}", self.func,
        if self.args.is_empty() { "".to_string() } else { format!("({})", self.args.join(", ")) },
        msg), self.location.clone())
}
```

Update `validate_table` (still in `constraint.rs::ConstraintValidator`):

```rust
pub fn validate_table(table: &Table) -> Result<(), Vec<Diagnostic>> {
    let mut errs = Vec::new();
    for f in &table.fields {
        if let Some(c) = &f.constraint {
            if let Err(msg) = c.validate(&[f.clone()], &table.data) {
                errs.push(c.to_diagnostic(&msg));
            }
        }
    }
    for c in &table.constraints {
        if let Err(msg) = c.validate(&table.fields, &table.data) {
            errs.push(c.to_diagnostic(&msg));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}
```

- [ ] **Step 2: Add row 5 parsing in read_excel**

In `table.rs::read_excel`, after the four-row header loop, add a row-5 parse step:

```rust
// Row 5: table-level constraints (each cell one constraint).
let row5_iter = rows.next();
let row5: Vec<String> = match row5_iter {
    Some(r) => r.iter().map(|c| c.to_string()).collect(),
    None    => vec![],
};
let mut table_constraints: Vec<Constraint> = Vec::new();
for (col_idx, raw) in row5.iter().enumerate() {
    let cell = raw.trim();
    if cell.is_empty() { continue; }
    if !cell.starts_with('@') {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::TableConstraintParseError,
            format!("row 5 cell {} must start with @, got '{}'", col_idx + 1, cell),
            SourceLocation { file: Some(std::path::PathBuf::from(fpath)), sheet: Some(sheet_name.clone()),
                              line: Some(5), column: Some(col_idx as u32 + 1) },
        ));
        continue;
    }
    let loc = SourceLocation { file: Some(std::path::PathBuf::from(fpath)), sheet: Some(sheet_name.clone()),
                              line: Some(5), column: Some(col_idx as u32 + 1) };
    match Constraint::from_str_with_loc(cell, loc) {
        Ok(c)  => table_constraints.push(c),
        Err(d) => diagnostics.push(d),
    }
}
```

Store the result: `Table { ..., constraints: table_constraints }` (the existing `Table` struct already has a `constraints` field; this commit wires it up at last).

- [ ] **Step 3: Make `Table::validate_constraints` return Result<(), Vec<Diagnostic>>**

Replace its body:

```rust
pub fn validate_constraints(&self) -> Result<(), Vec<Diagnostic>> {
    crate::core::table::constraint::ConstraintValidator::validate_table(self)
}
```

- [ ] **Step 4: Update CLI check.rs to use the new Result type**

In `tablec-cli/src/cmd/check.rs`:

```rust
match table.validate_constraints() {
    Ok(()) => {}
    Err(errs) => {
        for d in errs { eprintln!("{}", d); }
        return Err(format!("constraint validation failed").into());
    }
}
```

- [ ] **Step 5: Update binding-python check() to handle Vec<Diagnostic>**

In `binding-python/src/lib.rs::check`:

```rust
for table in &tables {
    if let Err(errs) = table.validate_constraints() {
        let msg = errs.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        return Err(pyo3::exceptions::PyValueError::new_err(msg));
    }
}
```

- [ ] **Step 6: Add a fixture-driven test for table-level unique**

Create `tablec-core/tests/fixtures/error_cases/build_bad_unique.rs`:

```rust
use rust_xlsxwriter::*;
fn main() {
    let mut wb = Workbook::new();
    let sh = wb.add_worksheet();
    sh.write_string(0, 0, "id").unwrap();
    sh.write_string(0, 1, "name").unwrap();
    sh.write_string(1, 0, "int").unwrap();
    sh.write_string(1, 1, "string").unwrap();
    sh.write_string(2, 0, "").unwrap();
    sh.write_string(2, 1, "").unwrap();
    sh.write_string(3, 0, "@seq").unwrap();
    sh.write_string(3, 1, "").unwrap();
    sh.write_string(4, 0, "@unique(id, name)").unwrap();
    sh.write_number(5, 0, 1).unwrap(); sh.write_string(5, 1, "alice").unwrap();
    sh.write_number(6, 0, 1).unwrap(); sh.write_string(6, 1, "alice").unwrap(); // duplicate
    sh.write_number(7, 0, 2).unwrap(); sh.write_string(7, 1, "bob").unwrap();
    wb.save("tests/fixtures/error_cases/bad_unique_constraint.xlsx").unwrap();
}
```

Register the example in `tablec-core/Cargo.toml` (add immediately after the existing `[[example]]` block added in task 4 step 2):

```toml
[[example]]
name = "build_bad_unique"
path = "tests/fixtures/error_cases/build_bad_unique.rs"
```

Run: `cd repos/tablec && cargo run --example build_bad_unique --manifest-path tablec-core/Cargo.toml`

Add `tests/constraint_extras.rs`:

```rust
#[test]
fn table_level_unique_duplicate_reports_diagnostic() {
    use tablec_core::core::diagnostic::DiagnosticCode;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/error_cases/bad_unique_constraint.xlsx");
    let tables = tablec_core::core::table::table::read_excel(path.to_str().unwrap()).unwrap();
    let errs = tables[0].validate_constraints().unwrap_err();
    assert!(errs.iter().any(|d| d.code == DiagnosticCode::ConstraintDuplicate));
}
```

- [ ] **Step 7: Run full test suite**

```bash
cd repos/tablec && cargo test --workspace
cd /home/bot/workbench/repos/tablec-testsuite && pytest -q
```

- [ ] **Step 8: Commit c5**

```bash
git -C repos/tablec add -A
git -C repos/tablec -c user.name="Claude" -c user.email="claude@anthropic.com" \
    commit -m "feat(core): wire table-level constraints from row 5

Row 5 of each sheet now declares table-level constraints using the
same @func(args) DSL as row 4. Each cell is one constraint; the cell's
column is irrelevant. Composite unique constraints like @unique(id, name)
are supported end-to-end.

Constraint struct gains a SourceLocation field. validate_table returns
Result<(), Vec<Diagnostic>>. New fixture bad_unique_constraint.xlsx
exercises a composite unique collision.

See spec §5 and plan task 5."
```

---

## Task 6: Blake3 hash + Meta extension (commit c6)

**Files:**
- Modify: `tablec-core/Cargo.toml` — add `blake3 = "1"`
- Modify: `tablec-core/src/core/project/meta.rs` — full rewrite
- Modify: `tablec-core/src/core/project/project.rs` — rewrite `calculate_hash`
- Modify: `tablec-core/src/export/json.rs` — adjust if necessary for hex serialization
- Modify: `tablec-core/src/export/msgpack.rs` — adjust if necessary for hex serialization

**Interfaces produced:**
- `Meta { version: String, hash: [u8; 32], build_at: i64, source: Vec<PathBuf>, tool: ToolVersion }`
- `ToolVersion { tablec: String, calamine: &'static str, serde_json: &'static str, blake3: &'static str }`
- `Meta::hash_hex() -> String` — 64-char hex (full), for human-readable display.
- Custom `Serialize`/`Deserialize` for `Meta` that converts `hash` to a 64-char hex string in JSON.
- `Project::calculate_hash(&mut self)` — Blake3 with domain `"tablec.project.v1"`, row-order sensitive.

- [ ] **Step 1: Add dependency and run cargo**

Edit `tablec-core/Cargo.toml`:

```toml
[dependencies]
calamine = "0.25.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0.122"
rmp-serde = "1.1.2"
toml = "0.8"
glob = "0.3"
indexmap = { version = "2.10.0", features = ["serde"] }
rand = "0.8.5"
blake3 = "1"
```

Run: `cd repos/tablec && cargo build -p tablec-core`
Expected: builds, blake3 resolves.

- [ ] **Step 2: Rewrite meta.rs with hex serialization**

Replace `tablec-core/src/core/project/meta.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ToolVersion {
    pub tablec: String,
    pub calamine: &'static str,
    pub serde_json: &'static str,
    pub blake3: &'static str,
}

impl Default for ToolVersion {
    fn default() -> Self {
        Self {
            tablec: env!("CARGO_PKG_VERSION").to_string(),
            calamine: "0.25.0",
            serde_json: "1.0.122",
            blake3: "1",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Meta {
    pub version: String,
    pub hash: [u8; 32],
    pub build_at: i64,
    pub source: Vec<PathBuf>,
    pub tool: ToolVersion,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            hash: [0u8; 32],
            build_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            source: Vec::new(),
            tool: ToolVersion::default(),
        }
    }
}

impl Meta {
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl Serialize for Meta {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Meta", 5)?;
        st.serialize_field("version", &self.version)?;
        st.serialize_field("hash", &self.hash_hex())?;
        st.serialize_field("build_at", &self.build_at)?;
        st.serialize_field("source", &self.source.iter().map(|p| p.display().to_string()).collect::<Vec<_>>())?;
        st.serialize_field("tool", &self.tool)?;
        st.end()
    }
}

impl<'de> Deserialize<'de> for Meta {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            version: String,
            hash: String,
            build_at: i64,
            #[serde(default)]
            source: Vec<String>,
            #[serde(default)]
            tool: Option<ToolVersion>,
        }
        let raw = Raw::deserialize(d)?;
        if raw.hash.len() != 64 {
            return Err(serde::de::Error::custom("hash must be 64-char hex"));
        }
        let mut hash = [0u8; 32];
        for i in 0..32 {
            hash[i] = u8::from_str_radix(&raw.hash[i*2..i*2+2], 16)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(Meta {
            version: raw.version,
            hash,
            build_at: raw.build_at,
            source: raw.source.into_iter().map(PathBuf::from).collect(),
            tool: raw.tool.unwrap_or_default(),
        })
    }
}

impl Serialize for ToolVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("ToolVersion", 4)?;
        st.serialize_field("tablec", &self.tablec)?;
        st.serialize_field("calamine", self.calamine)?;
        st.serialize_field("serde_json", self.serde_json)?;
        st.serialize_field("blake3", self.blake3)?;
        st.end()
    }
}

impl<'de> Deserialize<'de> for ToolVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            tablec: String,
            calamine: &'static str,
            serde_json: &'static str,
            blake3: &'static str,
        }
        Ok(ToolVersion {
            tablec: Raw::deserialize(d)?.tablec,
            calamine: "0.25.0",
            serde_json: "1.0.122",
            blake3: "1",
        })
    }
}

impl std::fmt::Display for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hash={} version={} build_at={} source={:?} tool=tablec/{}",
            self.hash_hex(), self.version, self.build_at, self.source, self.tool.tablec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_hex_is_64_chars() {
        let meta = Meta::default();
        assert_eq!(meta.hash_hex().len(), 64);
    }

    #[test]
    fn hash_hex_roundtrip_through_json() {
        let mut meta = Meta::default();
        meta.hash = [42u8; 32];
        let json = serde_json::to_string(&meta).unwrap();
        let meta2: Meta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.hash, meta2.hash);
    }

    #[test]
    fn json_hash_field_is_string_not_array() {
        let mut meta = Meta::default();
        meta.hash = [1u8; 32];
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"hash\":\"01010101"), "hash must serialize as hex string, got: {}", json);
    }
}
```

- [ ] **Step 3: Rewrite calculate_hash with Blake3, row-order sensitive**

Replace `Project::calculate_hash` in `project.rs`:

```rust
use blake3::Hasher;

pub fn calculate_hash(&mut self) {
    let mut hasher: Hasher = blake3::new_derive_key("tablec.project.v1");
    hasher.update(self.name.as_bytes());

    let mut sheets: Vec<(&String, &crate::core::table::table::Table)> = self.tables.iter().collect();
    sheets.sort_by(|a, b| a.0.cmp(b.0));

    for (sheet_name, table) in sheets {
        hasher.update(sheet_name.as_bytes());

        // Schema (canonical = JSON with sorted fields).
        let fields_canon = serde_json::to_vec(&canonical_fields(&table.fields))
            .expect("fields always serializable");
        hasher.update(&fields_canon);

        // Data rows, row-order sensitive (any reorder/delete → byte stream change → hash changes).
        for row in &table.data {
            let row_canon = serde_json::to_vec(&row.fields)
                .expect("row always serializable");
            hasher.update(&row_canon);
        }
    }

    self.meta.hash = *hasher.finalize().as_bytes();
}

fn canonical_fields(fields: &[crate::core::table::field::Field]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    names.sort();
    for n in names {
        let f = fields.iter().find(|x| x.name == n).unwrap();
        map.insert(n.to_string(), serde_json::json!(format!("{:?}", f.t)));
    }
    serde_json::Value::Object(map)
}
```

- [ ] **Step 4: Add tests for hash determinism and row-order sensitivity**

Create `tablec-core/tests/hash_extras.rs`:

```rust
use tablec_core::core::project::project::Project;

fn build_project(rows: Vec<Vec<(&str, tablec_core::core::table::value::Value)>>) -> Project {
    use tablec_core::core::table::field::{Field, FieldType};
    use tablec_core::core::table::row::Row;
    use tablec_core::core::table::table::Table;
    let field_a = Field { name: "a".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] };
    let field_b = Field { name: "b".into(), t: FieldType::String, desc: "".into(), constraint: None, tags: vec![] };
    let data = rows.into_iter().map(|r| {
        Row::from_vec(r.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }).collect();
    Project::from_tables("p".into(), vec![Table { name: "S".into(), fields: vec![field_a, field_b], data, constraints: vec![] }])
}

#[test]
fn hash_is_stable_across_two_runs() {
    let mut p1 = build_project(vec![
        vec![("a", tablec_core::core::table::value::Value::Int32(1)), ("b", tablec_core::core::table::value::Value::String("x".into()))],
    ]);
    let mut p2 = build_project(vec![
        vec![("a", tablec_core::core::table::value::Value::Int32(1)), ("b", tablec_core::core::table::value::Value::String("x".into()))],
    ]);
    p1.calculate_hash(); p2.calculate_hash();
    assert_eq!(p1.meta.hash, p2.meta.hash);
}

#[test]
fn hash_changes_when_rows_reordered() {
    let mut p1 = build_project(vec![
        vec![("a", tablec_core::core::table::value::Value::Int32(1)), ("b", tablec_core::core::table::value::value::Value::String("a".into()))],
        vec![("a", tablec_core::core::table::value::Value::Int32(2)), ("b", tablec_core::core::table::value::value::Value::String("b".into()))],
    ]);
    let mut p2 = build_project(vec![
        vec![("a", tablec_core::core::table::value::Value::Int32(2)), ("b", tablec_core::core::table::value::value::Value::String("b".into()))],
        vec![("a", tablec_core::core::table::value::Value::Int32(1)), ("b", tablec_core::core::table::value::value::Value::String("a".into()))],
    ]);
    p1.calculate_hash(); p2.calculate_hash();
    assert_ne!(p1.meta.hash, p2.meta.hash);
}

#[test]
fn hash_changes_when_row_deleted() {
    let mut p1 = build_project(vec![
        vec![("a", tablec_core::core::table::value::Value::Int32(1)), ("b", tablec_core::core::table::value::Value::String("x".into()))],
    ]);
    let mut p2 = build_project(vec![
        vec![("a", tablec_core::core::table::value::Value::Int32(1)), ("b", tablec_core::core::table::value::Value::String("x".into()))],
        vec![("a", tablec_core::core::table::value::Value::Int32(2)), ("b", tablec_core::core::table::value::Value::String("y".into()))],
    ]);
    p1.calculate_hash(); p2.calculate_hash();
    assert_ne!(p1.meta.hash, p2.meta.hash);
}
```

- [ ] **Step 5: Run the full suite**

```bash
cd repos/tablec && cargo test --workspace
cd /home/bot/workbench/repos/tablec-testsuite && pytest -q
```

Expected: testsuite may flag `meta.hash` shape change as snapshot diff. **Review each diff before running `update_snapshots.sh --apply`**.

- [ ] **Step 6: Commit c6**

```bash
git -C repos/tablec add -A
git -C repos/tablec -c user.name="Claude" -c user.email="claude@anthropic.com" \
    commit -m "feat(core): Meta.hash uses Blake3, stored as 32-byte array serialized as hex

calculate_hash uses Blake3 with domain separator 'tablec.project.v1'.
Schema is canonicalized (field names sorted) before hashing; data
rows are hashed in declaration order so any reordering or deletion
changes the hash.

Meta gains 'source' (input file paths) and 'tool' (locked version
fingerprint) fields. JSON serialization of Meta renders hash as a
64-char hex string for human readability, not serde's default 32-number
array. See spec §6 and plan task 6."
```

---

## Self-Review Pass

After writing the plan, run a checklist:

### Spec coverage

| Spec section | Where in plan |
|--------------|---------------|
| §1 Background / decisions | Header + Global Constraints |
| §2 Architecture / File layout | File Structure section |
| §3 Diagnostic + SourceLocation | Task 1 |
| §3.4 type_parser deletion | Task 3 step 6 |
| §4.1 Value/Type enum rewrite | Task 3 steps 1-2 |
| §4.2 parse + range check | Task 3 step 4 |
| §4.3 Trait consistency | Task 3 step 1 |
| §5 row 5 + Constraint location | Task 5 |
| §6 Meta hash + Blake3 | Task 6 |
| §7 unit test strategy | Each task's steps; tests are inline (TDD) |
| §8 commit order + rollback | Header order; per-task commits self-contained |
| §9 decision rationale | Header + Global Constraints |
| §10 risks | Per-task risk notes |

All covered. **Gap noted**: spec §1.3 lists binding-python upgrade as separate concern but plan Task 3 step 8+9 keeps binding-python compiling through a shim. A full upgrade (re-exposing Value variants) is **out of scope** of this plan and should be a separate brainstorming when needed.

### Placeholder scan

Patterns searched: "TBD", "TODO", "fill in", "similar to", "appropriate error handling". Found and fixed:

- Task 4 step 2 originally described the `[[example]]` registration only by reference; **fix applied** — explicit `[[example]]` block now in the step body.
- Task 5 step 6 originally said "(similar to step 2 of task 4)"; **fix applied** — step now repeats the full build script and `[[example]]` block explicitly.
- Final pass on the plan finds no remaining placeholder strings.

### Type consistency

- `DiagnosticCode` variants referenced in tasks: `TokenizerUnexpectedChar` (c2), `TypeParseError / TypeUnknown` (c2), `ValueParseError / ValueOutOfRange / StructFieldMismatch / StructFieldCountMismatch` (c3), `TableConstraintParseError` (c5), `ConstraintDuplicate / ConstraintSequenceBroken / ConstraintOrderViolation / ConstraintUnknown` (c5), `SheetSkipped / FieldMissingValue` (c4 spec — but used in tests, never in production code path). All defined in Task 1 DiagnosticCode enum.

- `FieldType` variants referenced: `Int8/16/32/64`, `Uint8/16/32/64`, `Uint` / `Uint32`, `Int / Int32`, `Float32/64`, `Float / Float32`, `String`, `Bool`, `Date/DateTime/Timestamp32/64`, `Array { r#type }`, `Map { key, value }`, `Struct { fields }`. All preserved.

- `Value` variants referenced: full set per Task 1. Order of variants in `discriminant` Hash test (Task 3 step 1) matches c3 enum order.

- `Meta.hash` shape: `[u8; 32]` everywhere (not `Vec<u8>`, not `String`). Tests assert length 32 or 64-char hex conversion.

- `Project` methods: `from_tables(name, Vec<Table>)` unchanged. `from_excel(name, path)` unchanged in signature (c4 changes read_excel internal). `validate_all` kept. `calculate_hash` rewritten.

### Ambiguity check

- Task 3 step 9 leaves binding-python at functional minimum. Documented as such; not ambiguous.
- Task 4 step 2: `[[example]]` block — fixed above (added explicit config snippet).
- Task 5 step 6: example block not specified — will fix.

After the self-review, fix the explicit examples blocks in both task 4 step 2 and task 5 step 6 by adding the example entry to the tablec-core Cargo.toml alongside the per-fixture build script.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-05-tablec-core-cleanup.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
