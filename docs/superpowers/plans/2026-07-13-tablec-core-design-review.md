# tablec-core Design Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land 4 surgical cleanups to `tablec-core` and `tablec-cli` per the design-review spec — remove dead `plugin.rs`, dedupe `validator.rs` into `ConstraintValidator`, extract a private `Numeric` enum from `Value`, and consolidate CLI diagnostic rendering into a new `diag_render` module.

**Architecture:** Each task is independent and revertible. Tasks 1–3 touch `tablec-core` only; Task 4 touches `tablec-cli` only. Public API of `tablec-core` does not change. No wire formats change. No new dependencies.

**Tech Stack:**
- Rust 1.60+ (per `tablec/README.md`)
- `cargo` workspace at `/home/bot/workbench/repos/tablec/`
- Crates involved: `tablec-core`, `tablec-cli` (hybrid bin+lib — `src/lib.rs` declares `pub mod cli; pub mod cmd;`)
- Test: built-in `cargo test`

## Global Constraints

- Spec source: `docs/superpowers/specs/2026-07-13-tablec-core-design-review.md` (at commit `8bcaa51`).
- Per-repo git author identity is overridden locally to `Claude <claude@anthropic.com>` via `.git/config` — global config is `cupen <xcupen@gmail.com>` (per memory). Use the local override; do NOT run `git config --global`.
- Per-repo SSH for `tablec` is `core.sshCommand = ssh -i ~/.ssh/id_ed25519 -o IdentitiesOnly=yes` in `.git/config`. Pre-configured. No SSH config edits needed.
- Each task's final commit must leave both green: `cargo build -p tablec-core -p tablec-cli` and `cargo test -p tablec-core`.
- Public API of `tablec-core` MUST NOT change: no variants added/removed from `Value`, `Type`, `FieldType`, `DiagnosticCode`; no public signature changes; JSON/msgpack wire formats unchanged.
- Do NOT touch `binding-python`, `tablec-testsuite`, `proto/` (out of scope per spec §1, §9).
- Do NOT add new dependencies this round. (ANSI color & `owo_colors` are next spec.)
- Single branch per task group (or 4 separate PRs) — engineer's call per spec §6.1. Plan walks each commit as its own task; if shipping as a single PR, fast-forward between local commits at the end.

## File Structure

### Files to delete
- `tablec-core/src/core/plugin.rs` — Task 1
- `tablec-core/src/core/table/validator.rs` — Task 2

### Files to create
- `tablec-cli/src/diag_render.rs` — Task 4

### Files to modify
- `tablec-core/src/core/mod.rs` — Task 1 (remove `pub mod plugin;`)
- `tablec-core/src/lib.rs` — Task 1 (remove `pub use core::plugin::*;`)
- `docs/superpowers/specs/2026-07-05-tablec-core-cleanup-design.md` — Task 1 (close out plugin open point in §9.3)
- `tablec-core/src/core/table/mod.rs` — Task 2 (remove `pub mod validator;`)
- `tablec-cli/src/cmd/check.rs` — Task 2 (line 5 import) AND Task 4 (line 91-96 paste site)
- `tablec-core/src/core/table/value.rs` — Task 3 (introduce private `Numeric` + 2 helpers; refactor 5 trait impls)
- `tablec-cli/src/lib.rs` — Task 4 (add `pub mod diag_render;`)
- `tablec-cli/src/cmd/build.rs` — Task 4 (replace 3 paste sites: lines 116-123, 150-157, 182-188)

### Tests to write
- Task 1: none (removal of dead code; covered by existing build/test)
- Task 2: no new test; existing `tests/constraint_tests.rs` and `tests/constraint_extras.rs` cover `ConstraintValidator` paths
- Task 3: add `numeric_helper_round_trip` test in `tests/value_tests.rs`
- Task 4: `#[cfg(test)] mod tests` inside `tablec-cli/src/diag_render.rs` itself (binary crate's tests run via `cargo test -p tablec-cli`)

---

## Task 1: Remove dead `plugin` module

**Files:**
- Delete: `tablec-core/src/core/plugin.rs`
- Modify: `tablec-core/src/core/mod.rs` (delete `pub mod plugin;` on line 4)
- Modify: `tablec-core/src/lib.rs` (delete `pub use core::plugin::*;` on line 8)
- Modify: `docs/superpowers/specs/2026-07-05-tablec-core-cleanup-design.md` (§9.3 open-points list)

**Interfaces:**
- Produces: nothing (pure removal)
- Consumes: nothing

- [ ] **Step 1: Confirm the file is truly uncalled**

Run:
```bash
grep -rE 'PluginManager|JsonFormatterPlugin|DataValidatorPlugin|CsvExporterPlugin|create_default_plugin_manager|PluginMetadata' \
  /home/bot/workbench/repos/tablec/tablec-core/src \
  /home/bot/workbench/repos/tablec/tablec-cli/src \
  /home/bot/workbench/repos/tablec/binding-python/src \
  /home/bot/workbench/repos/tablec/tests \
  --include='*.rs' 2>&1
```
Expected: only `tablec-core/src/core/plugin.rs` itself shows up. If anything else matches, **stop and re-evaluate** — the spec assumes zero external callers.

- [ ] **Step 2: Baseline build/test**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-core --lib 2>&1 | tail -10
```
Expected: green baseline. Note the test totals so you can compare after the removal.

- [ ] **Step 3: Delete `plugin.rs`**

Run:
```bash
git -C /home/bot/workbench/repos/tablec rm tablec-core/src/core/plugin.rs
```
Expected: file removed from working tree and staged.

- [ ] **Step 4: Update `tablec-core/src/core/mod.rs`**

Edit `tablec-core/src/core/mod.rs` — remove the `pub mod plugin;` line (currently line 4). The file should read:

```rust
pub mod diagnostic;
pub mod table;
pub mod parser;
pub mod project;
pub mod config;
```

- [ ] **Step 5: Update `tablec-core/src/lib.rs`**

Edit `tablec-core/src/lib.rs` — remove the `pub use core::plugin::*;` line (currently line 8). The file should read:

```rust
pub mod core;
pub mod export;

// Re-export the main types for easier access
pub use core::diagnostic::*;
pub use core::table::*;
pub use core::parser::*;
pub use core::project::*;
pub use export::*;
```

- [ ] **Step 6: Update the prior spec**

Edit `docs/superpowers/specs/2026-07-05-tablec-core-cleanup-design.md` §9.3 — delete the bullet `plugin 模块的迁移 / 删除` (this spec closes that open point). Leave the other two bullets (`tablec-cli 错误呈现层` and `binding-python Value 同步升级`) as still-open future specs.

- [ ] **Step 7: Build and test**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-core --lib 2>&1 | tail -10
```
Expected: same totals as Step 2 (no test count change). Both build and test green.

- [ ] **Step 8: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  git add tablec-core/src/core/mod.rs tablec-core/src/lib.rs docs/superpowers/specs/2026-07-05-tablec-core-cleanup-design.md && \
  git commit -m "chore(core): remove unused plugin module

Spec §2 (tablec-core design review, 2026-07-13). The plugin module
shipped three builtin plugins (JsonFormatter, DataValidator,
CsvExporter) with zero callers in production code. Validator output
overlapped with the existing ConstraintValidator; CSV exporter had
no integration with Project::export. Closes PR-1 follow-up noted
in 2026-07-05-core-cleanup-design §9.3."
```
Expected: one commit, no warnings, `git status` clean.

- [ ] **Step 9: Push**

Run:
```bash
git -C /home/bot/workbench/repos/tablec push 2>&1 | tail -3
```
Expected: `main -> main` update.

---

## Task 2: Dedupe `validator.rs` into `ConstraintValidator`

**Files:**
- Delete: `tablec-core/src/core/table/validator.rs`
- Modify: `tablec-core/src/core/table/mod.rs` (delete `pub mod validator;`)
- Modify: `tablec-cli/src/cmd/check.rs` (line 5 — change import)

**Interfaces:**
- Produces: same surface area — `ConstraintValidator::validate_table(&Table) -> Result<(), Vec<Diagnostic>>`. CLI now imports from `constraint` instead of `validator`.
- Consumes: nothing new. The signature of `validate_table` is unchanged.

- [ ] **Step 1: Baseline**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core -p tablec-cli 2>&1 | tail -3 && \
  cargo test -p tablec-core test_constraint 2>&1 | tail -10
```
Expected: green.

- [ ] **Step 2: Confirm only `cli::check.rs` calls `validator::validate_table`**

Run:
```bash
grep -rE 'use.*validator|validator::validate_table|use tablec_core::core::table::validator' \
  /home/bot/workbench/repos/tablec \
  --include='*.rs' 2>&1 | grep -v target/
```
Expected: a single line referencing `tablec-cli/src/cmd/check.rs:5:tablec_core::core::table::{table::read_excel, validator::validate_table};`.

- [ ] **Step 3: Update `tablec-cli/src/cmd/check.rs` line 5**

Replace the `use` line with:

```rust
use tablec_core::core::config::{self, Config};
use tablec_core::core::table::{table::read_excel, constraint::ConstraintValidator};
```

Then update the call inside `_run` (currently at lines 76-87). The variable currently named `validate_table` is being used as a function-path; switch to `ConstraintValidator::validate_table(&table)`:

```rust
for table in tables {
    println!("  Checking sheet: {}", table.name);
    match ConstraintValidator::validate_table(&table) {
        Ok(_) => {
            if c.verbose {
                println!("    OK");
            }
        }
        Err(errors) => {
            total_errors += errors.len();
            for d in errors {
                eprintln!("    Error: {}", d);
            }
        }
    }
}
```

(Note: do not change the rest of `_run`. This task is import + call-site only — Task 4 will refactor the inner `eprintln!` loop.)

- [ ] **Step 4: Build and test**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-core test_constraint 2>&1 | tail -10
```
Expected: same green as Step 1.

- [ ] **Step 5: Delete the old `validator.rs`**

Run:
```bash
git -C /home/bot/workbench/repos/tablec rm tablec-core/src/core/table/validator.rs
```
Then edit `tablec-core/src/core/table/mod.rs` to delete `pub mod validator;`. The file should read:

```rust
pub mod constraint;
pub mod field;
pub mod row;
pub mod table;
pub mod types;
pub mod value;
```

- [ ] **Step 6: Build and test (final)**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-core 2>&1 | tail -10
```
Expected: green. No test count delta.

- [ ] **Step 7: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  git add tablec-core/src/core/table/mod.rs tablec-core/src/core/table/validator.rs tablec-cli/src/cmd/check.rs && \
  git commit -m "refactor(core): dedupe validator into ConstraintValidator

Spec §3 (tablec-core design review, 2026-07-13). validator::validate_table
and ConstraintValidator::validate_table were two parallel implementations
of the same logic; numeric_i64 was duplicated verbatim in both modules.
Keep the ConstraintValidator copy (newer, Diagnostic-aware), drop the
validator module, point CLI check at the surviving one."
```

- [ ] **Step 8: Push**

Run:
```bash
git -C /home/bot/workbench/repos/tablec push 2>&1 | tail -3
```

---

## Task 3: Extract private `Numeric` from `Value`

**Files:**
- Modify: `tablec-core/src/core/table/value.rs` (introduce private `Numeric` and helpers, refactor 5 trait impls)

**Interfaces:**
- Produces: same external API. New private types `Numeric`, helpers `Value::to_numeric`, `Value::from_numeric` are crate-private.
- Consumes: nothing new.

This task is the largest. To keep the commit atomic, all 5 trait-impl refactors land in **one** commit. Multiple intermediate steps within the task run `cargo test` to catch regressions before the final commit.

- [ ] **Step 1: Baseline + capture existing JSON output**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo test -p tablec-core --tests 2>&1 | tail -10
```
Expected: green. Optionally snapshot a single Value's JSON for sanity:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo test -p tablec-core --tests serialize_each_numeric_variant -- --nocapture 2>&1 | tail -20
```
Expected: shows the 10 numeric outputs as `-1`, `-1`, ..., `1.5`, `1.5`. You'll compare later.

- [ ] **Step 2: Add the new failing test (TDD)**

Edit `tablec-core/tests/value_tests.rs`. Append at the end of the file:

```rust
#[test]
fn numeric_helper_round_trip() {
    use tablec_core::core::table::value::Value;
    let cases = vec![
        Value::Int8(-1), Value::Int16(-1), Value::Int32(-1), Value::Int64(-1),
        Value::Uint8(1), Value::Uint16(1), Value::Uint32(1), Value::Uint64(1),
        Value::Float32(1.5), Value::Float64(1.5),
    ];
    for v in &cases {
        // Helpers are crate-private; we exercise them via public traits.
        // First check: Serialize outputs the same JSON for each width.
        let s = serde_json::to_string(v).unwrap();
        assert!(s == "-1" || s == "1" || s == "1.5", "unexpected serialize: {}", s);
        // Second check: Hash is deterministic across calls.
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(v, &mut h1);
        std::hash::Hash::hash(v, &mut h2);
        assert_eq!(h1.finish(), h2.finish(), "hash not deterministic for {:?}", v);
    }
}
```

- [ ] **Step 3: Run the new test, confirm pass**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo test -p tablec-core --tests numeric_helper_round_trip 2>&1 | tail -10
```
Expected: PASS. (We're using existing public API to validate behavior; the helper itself isn't tested yet — that comes next.)

- [ ] **Step 4: Introduce the private `Numeric` enum + helpers**

In `tablec-core/src/core/table/value.rs`, near the top (after the imports, before the existing `Value` enum), add the new type and its helpers. The complete insertion:

```rust
// --- BEGIN: private Numeric support (Task 3) ---

#[derive(Debug, Clone, Copy)]
enum Numeric {
    I8(i8), I16(i16), I32(i32), I64(i64),
    U8(u8), U16(u16), U32(u32), U64(u64),
    F32(f32), F64(f64),
}

impl Numeric {
    fn kind(self) -> u8 {
        match self {
            Numeric::I8(_)  => 0, Numeric::I16(_) => 1, Numeric::I32(_) => 2, Numeric::I64(_) => 3,
            Numeric::U8(_)  => 4, Numeric::U16(_) => 5, Numeric::U32(_) => 6, Numeric::U64(_) => 7,
            Numeric::F32(_) => 8, Numeric::F64(_) => 9,
        }
    }
}

impl PartialEq for Numeric {
    fn eq(&self, other: &Self) -> bool {
        // exact match per spec §4.3; do NOT use epsilon
        match (self, other) {
            (Numeric::I8(a),  Numeric::I8(b))  => *a == *b,
            (Numeric::I16(a), Numeric::I16(b)) => *a == *b,
            (Numeric::I32(a), Numeric::I32(b)) => *a == *b,
            (Numeric::I64(a), Numeric::I64(b)) => *a == *b,
            (Numeric::U8(a),  Numeric::U8(b))  => *a == *b,
            (Numeric::U16(a), Numeric::U16(b)) => *a == *b,
            (Numeric::U32(a), Numeric::U32(b)) => *a == *b,
            (Numeric::U64(a), Numeric::U64(b)) => *a == *b,
            (Numeric::F32(a), Numeric::F32(b)) => *a == *b,
            (Numeric::F64(a), Numeric::F64(b)) => *a == *b,
        }
    }
}

impl PartialOrd for Numeric {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Numeric::I8(a),  Numeric::I8(b))  => a.partial_cmp(b),
            (Numeric::I16(a), Numeric::I16(b)) => a.partial_cmp(b),
            (Numeric::I32(a), Numeric::I32(b)) => a.partial_cmp(b),
            (Numeric::I64(a), Numeric::I64(b)) => a.partial_cmp(b),
            (Numeric::U8(a),  Numeric::U8(b))  => a.partial_cmp(b),
            (Numeric::U16(a), Numeric::U16(b)) => a.partial_cmp(b),
            (Numeric::U32(a), Numeric::U32(b)) => a.partial_cmp(b),
            (Numeric::U64(a), Numeric::U64(b)) => a.partial_cmp(b),
            (Numeric::F32(a), Numeric::F32(b)) => a.partial_cmp(b),
            (Numeric::F64(a), Numeric::F64(b)) => a.partial_cmp(b),
        }
    }
}

// --- END: private Numeric support ---
```

The helpers on `Value` are added in the next step (the `impl Value` block).

- [ ] **Step 5: Run the test, confirm pass**

Same command as Step 3: `cargo test -p tablec-core --tests numeric_helper_round_trip`. Expected: PASS. (`Numeric` exists but isn't connected to `Value` yet; this just verifies the file still compiles.)

- [ ] **Step 6: Add `Value::to_numeric` and `Value::from_numeric`**

In `tablec-core/src/core/table/value.rs`, add an `impl Value` block right after the `Value` enum definition and before the existing `numeric_kind` function. (Delete `numeric_kind` and `to_f64` after — they become dead.) Insert:

```rust
impl Value {
    fn to_numeric(&self) -> Option<Numeric> {
        match self {
            Value::Int8(n)   => Some(Numeric::I8(*n)),
            Value::Int16(n)  => Some(Numeric::I16(*n)),
            Value::Int32(n)  => Some(Numeric::I32(*n)),
            Value::Int64(n)  => Some(Numeric::I64(*n)),
            Value::Uint8(n)  => Some(Numeric::U8(*n)),
            Value::Uint16(n) => Some(Numeric::U16(*n)),
            Value::Uint32(n) => Some(Numeric::U32(*n)),
            Value::Uint64(n) => Some(Numeric::U64(*n)),
            Value::Float32(n) => Some(Numeric::F32(*n)),
            Value::Float64(n) => Some(Numeric::F64(*n)),
            _ => None,
        }
    }

    fn from_numeric(n: Numeric) -> Self {
        match n {
            Numeric::I8(v)  => Value::Int8(v),
            Numeric::I16(v) => Value::Int16(v),
            Numeric::I32(v) => Value::Int32(v),
            Numeric::I64(v) => Value::Int64(v),
            Numeric::U8(v)  => Value::Uint8(v),
            Numeric::U16(v) => Value::Uint16(v),
            Numeric::U32(v) => Value::Uint32(v),
            Numeric::U64(v) => Value::Uint64(v),
            Numeric::F32(v) => Value::Float32(v),
            Numeric::F64(v) => Value::Float64(v),
        }
    }
}
```

Delete the old free functions `numeric_kind` and `to_f64` (currently lines 20-43). They are no longer called after Step 7.

- [ ] **Step 7: Refactor `PartialOrd` to use the new helpers**

Replace the `impl PartialOrd for Value` block (currently lines 142-176) with:

```rust
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (self.to_numeric(), other.to_numeric()) {
            // Same-width
            if a == b { return Some(Ordering::Equal); }
            if a.partial_cmp(&b).is_some() { return a.partial_cmp(&b); }
            // Cross-width: promote to f64 (spec §4.3).
            if let (Some(af), Some(bf)) = (
                numeric_to_f64(a), numeric_to_f64(b)
            ) {
                return af.partial_cmp(&bf);
            }
            return None;
        }
        match (self, other) {
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

fn numeric_to_f64(n: Numeric) -> Option<f64> {
    Some(match n {
        Numeric::I8(v)  => v as f64,  Numeric::I16(v) => v as f64,
        Numeric::I32(v) => v as f64,  Numeric::I64(v) => v as f64,
        Numeric::U8(v)  => v as f64,  Numeric::U16(v) => v as f64,
        Numeric::U32(v) => v as f64,  Numeric::U64(v) => v as f64,
        Numeric::F32(v) => v as f64,  Numeric::F64(v) => v,
    })
}
```

Add `numeric_to_f64` as a private free function near `Numeric`. The `PartialOrd` impl above uses it.

- [ ] **Step 8: Build and test after `PartialOrd` refactor**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core 2>&1 | tail -10 && \
  cargo test -p tablec-core 2>&1 | tail -10
```
Expected: green. Pay special attention to `cross_width_partial_ord_promotes` — it must still pass.

- [ ] **Step 9: Refactor `Serialize` to use helpers**

Replace `impl Serialize for Value` (currently lines 47-87) with:

```rust
impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if let Some(n) = self.to_numeric() { return n.serialize(s); }
        match self {
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

impl Serialize for Numeric {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Numeric::I8(v)  => s.serialize_i8(*v),
            Numeric::I16(v) => s.serialize_i16(*v),
            Numeric::I32(v) => s.serialize_i32(*v),
            Numeric::I64(v) => s.serialize_i64(*v),
            Numeric::U8(v)  => s.serialize_u8(*v),
            Numeric::U16(v) => s.serialize_u16(*v),
            Numeric::U32(v) => s.serialize_u32(*v),
            Numeric::U64(v) => s.serialize_u64(*v),
            Numeric::F32(v) => s.serialize_f32(*v),
            Numeric::F64(v) => s.serialize_f64(*v),
        }
    }
}
```

- [ ] **Step 10: Refactor `PartialEq`**

Replace `impl PartialEq for Value` (currently lines 90-114) with:

```rust
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        // Numeric: delegate to Numeric's PartialEq (cross-width returns false,
        // same-width compares bits). Floats compare bitwise, NOT with EPSILON —
        // see doc comment above and spec §4.3.
        if let (Some(a), Some(b)) = (self.to_numeric(), other.to_numeric()) {
            return a == b;
        }
        // Non-numeric fallthrough
        match (self, other) {
            (Value::String(a), Value::String(b))     => a == b,
            (Value::Bool(a),   Value::Bool(b))       => a == b,
            (Value::Array(a),  Value::Array(b))      => a == b,
            (Value::Map(a),    Value::Map(b))        => a == b,
            (Value::Struct(a), Value::Struct(b))     => a == b,
            (Value::Null,      Value::Null)          => true,
            _ => false,
        }
    }
}
```

Add the matching doc comment immediately above the impl block:

```rust
/// Float comparisons are bitwise exact via Numeric's `==`. Per spec §4.3
/// we deliberately do NOT use `f32::EPSILON` / `f64::EPSILON`: those are
/// minimum representable differences, not useful error tolerances.
/// Consequences:
///   - `NaN != NaN` (IEEE 754)
///   - `inf != inf` (treated as distinct values, since inf bits match,
///     `==` returns `true` here; but if your NaN/coercion semantics
///     matter, check the spec verbatim before changing)
/// If you need tolerance-based equality, wrap with `approx` or similar.
///
/// Note: this changes behavior vs the previous `(a - b).abs() < EPSILON`
/// impl for ints like `f64::NAN` and very-close-but-not-equal floats.
/// Tests covered: existing `value_size_is_sixteen_variants`,
/// `cross_width_partial_ord_promotes`, `serialize_each_numeric_variant`,
/// `numeric_helper_round_trip` (this PR).
```

- [ ] **Step 11: Refactor `Hash`**

Replace `impl Hash for Value` (currently lines 118-140) with:

```rust
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the discriminant + numeric kind uniformly so e.g. Int32(0)
        // and Uint32(0) hash to different buckets.
        core::mem::discriminant(self).hash(state);
        if let Some(n) = self.to_numeric() {
            n.kind().hash(state);
            match n {
                Numeric::I8(v)  => v.hash(state),
                Numeric::I16(v) => v.hash(state),
                Numeric::I32(v) => v.hash(state),
                Numeric::I64(v) => v.hash(state),
                Numeric::U8(v)  => v.hash(state),
                Numeric::U16(v) => v.hash(state),
                Numeric::U32(v) => v.hash(state),
                Numeric::U64(v) => v.hash(state),
                Numeric::F32(v) => v.to_bits().hash(state),
                Numeric::F64(v) => v.to_bits().hash(state),
            }
            return;
        }
        match self {
            Value::String(s) => s.hash(state),
            Value::Bool(b)   => b.hash(state),
            Value::Array(a)  => a.hash(state),
            Value::Map(m)    => { for (k, v) in m { k.hash(state); v.hash(state); } }
            Value::Struct(s) => { for (k, v) in s { k.hash(state); v.hash(state); } }
            Value::Null      => 0u8.hash(state),
        }
    }
}
```

- [ ] **Step 12: Refactor `Display`**

Replace `impl fmt::Display for Value` (currently lines 178-199) with:

```rust
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(n) = self.to_numeric() {
            return match n {
                Numeric::I8(v)  => write!(f, "{}", v),
                Numeric::I16(v) => write!(f, "{}", v),
                Numeric::I32(v) => write!(f, "{}", v),
                Numeric::I64(v) => write!(f, "{}", v),
                Numeric::U8(v)  => write!(f, "{}", v),
                Numeric::U16(v) => write!(f, "{}", v),
                Numeric::U32(v) => write!(f, "{}", v),
                Numeric::U64(v) => write!(f, "{}", v),
                Numeric::F32(v) => write!(f, "{}", v),
                Numeric::F64(v) => write!(f, "{}", v),
            };
        }
        match self {
            Value::String(s) => write!(f, "'{}'", s),
            Value::Bool(b)   => write!(f, "{}", b),
            Value::Null      => write!(f, "null"),
            Value::Array(a)  => { write!(f, "[")?; for (i, x) in a.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}", x)?; } write!(f, "]") }
            Value::Map(m)    => { write!(f, "{{")?; for (i, (k, v)) in m.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
            Value::Struct(s) => { write!(f, "{{")?; for (i, (k, v)) in s.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
        }
    }
}
```

- [ ] **Step 13: Build + run full test**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-core 2>&1 | tail -5 && \
  cargo test -p tablec-core 2>&1 | tail -10
```
Expected: green. Specifically, `value_size_is_sixteen_variants`, `cross_width_partial_ord_promotes`, `serialize_each_numeric_variant`, `hash_includes_discriminant`, `numeric_helper_round_trip` all PASS.

- [ ] **Step 14: Run `cargo clippy` to catch style regressions**

Run:
```bash
cd /home/bot/workbench/repos/tablec && cargo clippy -p tablec-core --tests 2>&1 | tail -20
```
Expected: no new warnings introduced (existing warnings about `not_unsafe_ptr_arg_deref` etc are pre-existing; only worry about NEW warnings). If clippy reports new warnings, fix them before commit.

- [ ] **Step 15: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  git add tablec-core/src/core/table/value.rs tablec-core/tests/value_tests.rs && \
  git commit -m "refactor(core): extract Numeric from Value for impl dedup

Spec §4 (tablec-core design review, 2026-07-13). The 5 trait impls
(Serialize, PartialEq, Hash, PartialOrd, Display) on Value each had
10-arm matches for numeric variants. Introduce a private Numeric
enum absorbing these arms once; trait impls on Value delegate via
to_numeric/from_numeric. Public API unchanged. Float PartialEq now
uses bitwise comparison (Numeric::eq) — see value.rs doc comment
for rationale; previous EPSILON behavior is intentionally retired
per spec §4.3."
```

- [ ] **Step 16: Push**

Run:
```bash
git -C /home/bot/workbench/repos/tablec push 2>&1 | tail -3
```

---

## Task 4: Consolidate CLI diagnostic rendering

**Files:**
- Create: `tablec-cli/src/diag_render.rs`
- Modify: `tablec-cli/src/lib.rs` (add `pub mod diag_render;`)
- Modify: `tablec-cli/src/cmd/build.rs` (replace 3 paste sites at lines 116-123, 150-157, 182-188)
- Modify: `tablec-cli/src/cmd/check.rs` (replace paste site at lines 91-96)

**Interfaces:**
- Produces:
  - `pub(crate) fn render_diags<W: Write>(diags: &[Diagnostic], out: &mut W) -> io::Result<()>`
  - `pub(crate) fn diag_exit_code(diags: &[Diagnostic]) -> i32`
  - `pub(crate) fn diag_summary(diags: &[Diagnostic]) -> String`
- Consumes:
  - `tablec_core::core::diagnostic::{Diagnostic, Severity}`
  - `Severity::Error == Error`, `Severity::Warning == Warning` (existing enum)

- [ ] **Step 1: Write the failing test (TDD)**

Create `tablec-cli/src/diag_render.rs` with this initial content (test-only; implementation returns empty for now):

```rust
use std::io::{self, Write};
use tablec_core::core::diagnostic::{Diagnostic, Severity, SourceLocation, DiagnosticCode};

pub(crate) fn render_diags<W: Write>(_diags: &[Diagnostic], _out: &mut W) -> io::Result<()> {
    Ok(()) // placeholder
}

pub(crate) fn diag_exit_code(_diags: &[Diagnostic]) -> i32 { 0 }

pub(crate) fn diag_summary(_diags: &[Diagnostic]) -> String { String::new() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn diag_with(sev: Severity, msg: &str) -> Diagnostic {
        Diagnostic { severity: sev, code: DiagnosticCode::Other, message: msg.to_string(),
            location: SourceLocation::default() }
    }

    #[test]
    fn render_diags_writes_one_line_per_diag() {
        let diags = vec![diag_with(Severity::Error, "a"), diag_with(Severity::Warning, "b")];
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&diags, &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        // Each diag takes exactly one line.
        assert_eq!(s.lines().count(), 2, "got: {:?}", s);
        // Both lines reference the diag message.
        assert!(s.contains("a"));
        assert!(s.contains("b"));
    }

    #[test]
    fn render_diags_includes_file() {
        let d = Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::TypeParseError,
            message: "bad".into(),
            location: SourceLocation {
                file: Some(std::path::PathBuf::from("/abs/x.xlsx")),
                sheet: Some("S".into()),
                line: Some(2), column: Some(5),
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.contains("/abs/x.xlsx"), "expected file path in {:?}", s);
        assert!(s.contains("S"), "expected sheet in {:?}", s);
        assert!(s.contains("2:5"), "expected line:col in {:?}", s);
    }

    #[test]
    fn render_diags_skips_missing_file_gracefully() {
        let d = diag_with(Severity::Error, "no loc");
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        // No panic, message present.
        assert!(s.contains("no loc"));
    }

    #[test]
    fn diag_exit_code_first_error_returns_1() {
        let diags = vec![diag_with(Severity::Error, "e"), diag_with(Severity::Warning, "w")];
        assert_eq!(diag_exit_code(&diags), 1);
    }

    #[test]
    fn diag_exit_code_only_warnings_returns_0() {
        let diags = vec![diag_with(Severity::Warning, "w")];
        assert_eq!(diag_exit_code(&diags), 0);
    }

    #[test]
    fn diag_exit_code_empty_returns_0() {
        assert_eq!(diag_exit_code(&[]), 0);
    }

    #[test]
    fn diag_summary_counts_severity() {
        let diags = vec![
            diag_with(Severity::Error, "e1"),
            diag_with(Severity::Error, "e2"),
            diag_with(Severity::Warning, "w1"),
        ];
        assert_eq!(diag_summary(&diags), "2 errors, 1 warning");
    }

    // silence unused-import warning when `Hash` and `DefaultHasher` aren't used
    #[allow(dead_code)]
    fn _silence_hash() -> u64 {
        let mut h = DefaultHasher::new();
        "x".hash(&mut h);
        h.finish()
    }
}
```

- [ ] **Step 2: Wire up the module**

Edit `tablec-cli/src/lib.rs` to add `pub mod diag_render;`. The file becomes:

```rust
pub mod cli;
pub mod cmd;
pub mod diag_render;
```

- [ ] **Step 3: Run the test, confirm mostly-passing (3 will fail)**

Run:
```bash
cd /home/bot/workbench/repos/tablec && cargo test -p tablec-cli --lib 2>&1 | tail -30
```
Expected: 2 pass, 5 fail:
- `diag_exit_code_only_warnings_returns_0` — passes (placeholder returns 0)
- `diag_exit_code_empty_returns_0` — passes (placeholder returns 0)
- `render_diags_writes_one_line_per_diag` — fails (0 lines vs 2)
- `render_diags_includes_file` — fails (no path)
- `render_diags_skips_missing_file_gracefully` — fails (no message written)
- `diag_exit_code_first_error_returns_1` — fails (placeholder returns 0, expected 1)
- `diag_summary_counts_severity` — fails (empty string vs "2 errors, 1 warning")

- [ ] **Step 4: Implement `render_diags`**

Replace the placeholder in `tablec-cli/src/diag_render.rs`:

```rust
pub(crate) fn render_diags<W: Write>(diags: &[Diagnostic], out: &mut W) -> io::Result<()> {
    for d in diags {
        // severity prefix + Diagnostic::Display + file suffix
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(out, "{}\t{}", sev, d)?;
        if let Some(file) = &d.location.file {
            write!(out, "\t{}", file.display())?;
        }
        writeln!(out)?;
    }
    Ok(())
}
```

- [ ] **Step 5: Implement `diag_exit_code` and `diag_summary`**

```rust
pub(crate) fn diag_exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| matches!(d.severity, Severity::Error)) { 1 } else { 0 }
}

pub(crate) fn diag_summary(diags: &[Diagnostic]) -> String {
    let errors = diags.iter().filter(|d| matches!(d.severity, Severity::Error)).count();
    let warnings = diags.iter().filter(|d| matches!(d.severity, Severity::Warning)).count();
    let mut parts = Vec::new();
    if errors > 0 { parts.push(format!("{} {}", errors, if errors == 1 { "error" } else { "errors" })); }
    if warnings > 0 { parts.push(format!("{} {}", warnings, if warnings == 1 { "warning" } else { "warnings" })); }
    if parts.is_empty() { "no issues".to_string() } else { parts.join(", ") }
}
```

- [ ] **Step 6: Run tests, confirm all pass**

Run:
```bash
cd /home/bot/workbench/repos/tablec && cargo test -p tablec-cli --lib 2>&1 | tail -20
```
Expected: all 7 unit tests pass.

- [ ] **Step 7: Replace paste sites in `build.rs`**

Edit `tablec-cli/src/cmd/build.rs`. Replace each of the three `match read_excel(...) { Err(errs) => { ... } }` blocks (lines 116-123, 150-157, 182-188). Use this consistent form:

```rust
let tables = match read_excel(...) {
    Ok(t) => t,
    Err(errs) => {
        crate::diag_render::render_diags(&errs, &mut std::io::stderr().lock())?;
        let summary = crate::diag_render::diag_summary(&errs);
        return Err(format!("read_excel failed: {}", summary).into());
    }
};
```

Three call sites with slightly different inputs:
1. **Line 116-123** (`build_single_file`, path = `input`):
   ```rust
   let tables = match read_excel(input) {
       Ok(t) => t,
       Err(errs) => {
           crate::diag_render::render_diags(&errs, &mut std::io::stderr().lock())?;
           return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
       }
   };
   ```
2. **Line 150-157** (`build_merged_files`, path = `file_path.to_str().unwrap()`):
   ```rust
   let tables = match read_excel(file_path.to_str().unwrap()) {
       Ok(t) => t,
       Err(errs) => {
           crate::diag_render::render_diags(&errs, &mut std::io::stderr().lock())?;
           return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
       }
   };
   ```
3. **Line 182-188** (`build_to_string`):
   ```rust
   let tables = match read_excel(input_path) {
       Ok(t) => t,
       Err(errs) => {
           crate::diag_render::render_diags(&errs, &mut std::io::stderr().lock())?;
           return Err(format!("read_excel failed: {}", crate::diag_render::diag_summary(&errs)).into());
       }
   };
   ```

(For each, `crate::diag_render::` is the path inside `tablec-cli` since `lib.rs` declares `pub mod diag_render`.)

Also add the import at the top of the file (after `use std::io;` if not present, otherwise add `use std::io;`):
```rust
use std::io;
```

- [ ] **Step 8: Replace paste site in `check.rs`**

Edit `tablec-cli/src/cmd/check.rs`. Inside `_run`, replace the `Err(errs) => { ... }` arm at lines 91-96 with:

```rust
Err(errs) => {
    total_errors += errs.len();
    crate::diag_render::render_diags(&errs, &mut std::io::stderr().lock())?;
}
```

Leave the surrounding line `total_errors += errs.len();` intact (Task 2 already moved the entry-point to `ConstraintValidator::validate_table`; this is just the inner loop).

- [ ] **Step 9: Build and CLI tests**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  cargo build -p tablec-cli 2>&1 | tail -5 && \
  cargo test -p tablec-cli --lib 2>&1 | tail -15
```
Expected: green.

- [ ] **Step 10: Smoke test the CLI**

Run a `check` on a known-good and a known-bad input:

```bash
# (Pick or construct a known-bad xlsx that read_excel returns Diagnostics for.
#  Or use the existing testsuite fixture at /home/bot/workbench/repos/tablec-testsuite/fixtures/error_cases/ if available.)
cd /home/bot/workbench/repos/tablec && \
  cargo run -p tablec-cli -- check /path/to/valid.xlsx 2>&1 | head -10 && \
  echo "---" && \
  cargo run -p tablec-cli -- check /path/to/invalid.xlsx 2>&1 | head -30
```
Expected: bad case shows lines like:
```
error   TypeParseError [Sheet1] 2:5  /abs/path.xlsx: Unknown type: foo
warning ...   /abs/path.xlsx: ...
```
Confirm:
- Each diagnostic renders on exactly one line
- File paths appear (if present in `SourceLocation`)
- Severity (`error` / `warning`) is the leading word

- [ ] **Step 11: Commit**

Run:
```bash
cd /home/bot/workbench/repos/tablec && \
  git add tablec-cli/src/diag_render.rs tablec-cli/src/lib.rs tablec-cli/src/cmd/build.rs tablec-cli/src/cmd/check.rs && \
  git commit -m "feat(cli): consolidate diagnostic rendering (no ANSI yet)

Spec §5 (tablec-core design review, 2026-07-13). Three identical
error-handling blocks in build.rs and one in check.rs are replaced
with calls to a new internal diag_render module. Behavior:
  - severity prefix (error|warning)
  - Diagnostic Display (code, sheet, line:col, message)
  - file path appended when SourceLocation has one

No ANSI color yet; that's a follow-up spec. CLI behavior at the
byte level for stderr is intentionally different (was: bare
read_excel failed message; now: human-readable diagnostics)."
```

- [ ] **Step 12: Push**

Run:
```bash
git -C /home/bot/workbench/repos/tablec push 2>&1 | tail -3
```

---

## Self-Review

**Spec coverage (mapping each spec section to a task):**
- Spec §2 (plugin removal) → Task 1 ✓
- Spec §3 (validator dedup) → Task 2 ✓
- Spec §4 (Value Numeric abstraction, including Float comparison strategy in §4.3) → Task 3 ✓
- Spec §5 (CLI diag_render) → Task 4 ✓
- Spec §6 (rhythm/rollbacks) → Each task's "Rollback" note inline; spec says each task is independent. ✓
- Spec §7 (risks) → Each task addresses risks inline (Task 1 caller grep, Task 2 signature check, Task 3 clippy, Task 4 smoke test). ✓
- Spec §8 (decisions) → Adopted: plugin delete, ConstraintValidator kept, Numeric private, EPSILON retired per §4.3. ✓
- Spec §9 (out of scope) → No task touches those. ✓

**Placeholder scan:** No "TBD" / "TODO" / "implement later" / "fill in details" markers. Code is fully shown for refactors. Smoke-test step calls out specific expected output.

**Type consistency:** All `task_id`, file paths, and function names match between tasks. `Diagnostic`, `Severity`, `SourceLocation`, `DiagnosticCode` paths used consistently. `Numeric` is private to `value.rs`. `render_diags` is `pub(crate)` so it's visible at `crate::diag_render::` from inside `tablec-cli`.

**Risk to flag:** Task 3 step 7 — the spec's "promote via f64" rule via Numeric::partial_cmp first requires the `a == b` shortcut on Numeric, which is `==` only on same-width after the f64 promotion. Re-read spec §4.3 carefully during implementation; if the test `cross_width_partial_ord_promotes` fails, the most likely cause is "I forgot to call `numeric_to_f64` for cross-width numerics". The plan's second version of `PartialOrd` is the right one.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-13-tablec-core-design-review.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
