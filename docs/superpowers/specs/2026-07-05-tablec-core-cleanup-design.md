# tablec-core 清理与正确性提升 — 设计稿

**日期**: 2026-07-05
**仓库**: `repos/tablec`
**范围**: `tablec-core` 内部分层与对应测试
**不做**: CLI 表现层错误处理(独立 spec)、plugin 模块迁移(独立 spec)

---

## 1. 背景与目标

针对 `tablec-core` 中已识别的隐患与设计债,本 spec 集中解决"运行时正确性"问题,涉及 5 处代码改动,落地为 6 个 commit。本 spec 不改变 `Project`/`Format`/`Config` 的总体方向,只补全既有的语义与可观测性。

### 1.1 已识别问题(摘要)

| 优先级 | 问题 | 位置 |
|--------|------|------|
| P0 | 解析错误全部静默吞掉 | `table.rs:112`, `field.rs:78` |
| P0 | 类型宽度信息在 Value 阶段丢失 | `field.rs → value.rs` |
| P0 | `tokenizer` 遇到未识别字符直接 `panic!` | `tokenizer.rs:52` |
| P1 | `table.constraints` 字段是死代码 | `constraint.rs:194` |
| P1 | `Meta.hash` 用 `DefaultHasher`,跨进程非确定 | `project.rs:44`, `meta.rs` |
| (同行) | 僵尸代码:`type_parser.rs`、legacy `Json::to_string` | `parser/type_parser.rs`, `export/json.rs:56` |

### 1.2 设计范围决策

| 维度 | 选择 |
|------|------|
| 范围 | P0 全部 + P1 部分(`table.constraints` 接通 + 稳定 hash) |
| 兼容性 | 允许 breaking API 变更(私有仓库、自用、Cargo workspace 内可调整) |
| 错误风格 | 结构化 `Diagnostic` + `SourceLocation`(取代现有的 `Result<_, String>` 与 stderr 静默) |
| 类型宽度 | Value 八宽度区分(i8/i16/i32/i64、u8/u16/u32/u64、f32、f64),解析时做范围检查 |
| 实施节奏 | 单一 spec,落地为 6 个 commit,每 commit 独立编译/测试 |
| Hash 算法 | Blake3(`blake3` crate),派生键 + 列方向序列化 |
| Hash 行序敏感 | 是,默认情况下任何行调换/删除必变 hash |

### 1.3 不在本 spec 范围

- `tablec-cli` 的错误呈现层(漂亮打印、退出码)
- `binding-python` 的升级(另起一个 spec/plan,本 spec 的 c3 涉及 Value enum 重排后会破坏 binding,需要同步跟进 commit c3.1)
- `plugin` 模块的迁移 / 删除
- `tablec-testsuite` 的覆盖率扩展(只更新快照与现有断言)

---

## 2. 架构与模块布局

### 2.1 新增模块

```
tablec-core/src/core/diagnostic.rs        ← 新增
```

公开类型:

- `SourceLocation { file: Option<PathBuf>, sheet: Option<String>, line: Option<u32>, column: Option<u32> }`
- `Severity { Error, Warning }`
- `Diagnostic { severity, code, message, location }`
- `DiagnosticCode` enum:见 §3.1
- 实现:`Clone`、`Serialize`、`Deserialize`、`Display`、`From<&str>`

### 2.2 模块改动

| 文件 | 改动 |
|------|------|
| `parser/tokenizer.rs` | `scan_tokens(&str)` → `scan_tokens(&str, loc: SourceLocation) -> Result<Vec<Token>, Diagnostic>` |
| `parser/type_parser.rs` | **删除**(整文件无引用,僵尸代码) |
| `parser/value_parser.rs` | 改签名收 `FieldType` + `SourceLocation`,递归结构改为按字段名匹配,不再位置强绑 |
| `core/table/field.rs` | `FieldType::to_type()` 改为 width-aware,展开具体宽度不再塌缩 |
| `core/table/value.rs` | `Value` enum 重写为 10 数值变体(§4.1) |
| `core/table/types.rs` | `Type` enum 同步展宽(§4.1) |
| `core/table/constraint.rs` | `Constraint` 加 `location` 字段,`validate_table` 真接 `table.constraints`,错误转 `Diagnostic` |
| `core/table/table.rs` | `read_excel` 改 `Result<Vec<Table>, Vec<Diagnostic>>`;row 5 解析为表级约束 |
| `core/project/project.rs` | `calculate_hash` 改用 Blake3,行序敏感 |
| `core/project/meta.rs` | `hash: i64` → `[u8; 32]`,新增 `source: Vec<PathBuf>` 与 `tool: ToolVersion` |
| `export/json.rs` | 删除 `to_string(legacy)`(整文件无引用) |
| `Cargo.toml` | 新增依赖 `blake3 = "1"`、`serde` 仍在用(无需新增)、`thiserror` 不引 |

### 2.3 对外行为保留

- **JSON 顶层 schema 不变**:仍是 `{ name, meta, tables: [{ name, data, fields? }] }`,只内部字段值(例如 `meta.hash`)类型变
- **msgpack 二进制布局同理**:`Meta` 序列化字段类型变了,下游持有 `Meta` 反序列化的位置需要同步更新
- **Excel 输入格式不变**:前 5 行约定不变(行 5 的语法在第 5 节单独说明)

---

## 3. 错误体系(Diagnostic)

### 3.1 Diagnostic 与 SourceLocation 形状

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: Option<PathBuf>,
    pub sheet: Option<String>,
    pub line: Option<u32>,      // 1-based
    pub column: Option<u32>,    // 1-based
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity { Error, Warning }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticCode {
    // parser
    TokenizerUnexpectedChar,
    TypeParseError,
    TypeUnknown,
    // value
    ValueParseError,
    ValueOutOfRange,
    StringEscapeUnsupported,
    StructFieldMismatch,
    StructFieldCountMismatch,
    // table
    SheetSkipped,
    FieldMissingValue,
    TableConstraintParseError,
    // constraint
    ConstraintUnknown,
    ConstraintDuplicate,
    ConstraintSequenceBroken,
    ConstraintOrderViolation,
    ConstraintFieldMissing,
    ConstraintCompositeMissing,
    // fallback
    Other,
}
```

### 3.2 设计要点

- `DiagnosticCode` 用 `#[non_exhaustive]`:外部增加代码不破坏下游 match
- 位置以"最小可定位"为度,只到行/列/sheet,不存 AST 节点或 byte-range
- `Severity::Warning` 默认不开:现阶段 value parse 失败是 Error;constraint 给配置选项可降级(c5 顺便加,字段在 `Meta`/config)
- 库代码统一返回 `Result<T, Vec<Diagnostic>>`(顶层),或 `Result<T, Diagnostic>`(单值函数如 tokenizer、parse_value)
- 不引入 `thiserror` / `anyhow`:Diagnostic 是值类型不是 Error trait
- `From<&str> for Diagnostic` 用 severity=Error, code=Other, location 空白

### 3.3 调用约定

```rust
// 单点解析
fn scan_tokens(s: &str, loc: SourceLocation) -> Result<Vec<Token>, Diagnostic>
fn parse_value(s: &str, ty: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic>
fn parse_type(s: &str, loc: SourceLocation) -> Result<Type, Diagnostic>  // 见 §3.4

// 边界收敛:多错误聚合
pub fn read_excel(path: &str) -> Result<Vec<Table>, Vec<Diagnostic>>
pub fn Project::validate_all(&self) -> Result<(), Vec<Diagnostic>>
```

### 3.4 parse_type 双实现合流

`parser/type_parser.rs` 现有 `parse_type(&str) -> Result<Type, String>` 与 `field::from_str(FieldType)` 是两条独立路径。本 spec:

- **删除 `parser/type_parser.rs`**(僵尸代码,无引用)
- 字段解析路径只走 `FieldType::from_str`,row 5 表级约束走 `Constraint::from_str`
- 未来若需要字符串到 Type 的便捷解析,在 `field.rs` 里新增 `FieldType::from_str_with_loc(s, loc) -> Result<Self, Diagnostic>`,不另开文件

---

## 4. 类型宽度保留

### 4.1 Value 与 Type 重定义

`Value` 一共 16 个变体:10 个数值(Int8/16/32/64、Uint8/16/32/64、Float32/64)、String、Bool、Array、Map、Struct、Null。

```rust
// value.rs
#[derive(Debug, Clone)]
pub enum Value {
    Int8(i8), Int16(i16), Int32(i32), Int64(i64),
    Uint8(u8), Uint16(u16), Uint32(u32), Uint64(u64),
    Float32(f32), Float64(f64),
    String(String), Bool(bool),
    Array(Vec<Value>), Map(IndexMap<Value, Value>),
    Struct(IndexMap<String, Value>), Null,
}

// types.rs
pub enum Type {
    Int8, Int16, Int32, Int64,
    Uint8, Uint16, Uint32, Uint64,
    Float32, Float64,
    String, Bool,
    Array(Box<Type>), Map(Box<Type>, Box<Type>),
    Struct(HashMap<String, Type>),
    Any,
}
```

旧别名 `Int / Uint / Float` 全部删除。`Type::Int` 不再存在——`FieldType::Int` 在 `field::parse_base_type` 已是 `FieldType::Int32`,`to_type()` 精确返回 `Type::Int32`。

### 4.2 解析与范围检查

```rust
// value_parser.rs(新签名)
pub fn parse_value(
    s: &str,
    ty: &FieldType,
    loc: SourceLocation,
) -> Result<Value, Diagnostic>
```

**两阶段**:

1. `s.parse::<target_rust_type>()` — Rust 内置解析,fail 时 `ParseIntError` / `ParseFloatError`
2. 失败 → `Diagnostic`:
   - 数字但超过目标宽度 → `ValueOutOfRange`(`message` 含字面范围)
   - 空串、非数字字符 → `ValueParseError`

每个 `FieldType` 数值变体单独分派一行:

```
FieldType::Int8   → s.parse::<i8>()
FieldType::Int16  → s.parse::<i16>()
…
FieldType::Float64 → s.parse::<f64>()
```

递归: `Array { type }` / `Map { key, value }` 内部递归调用同一 `parse_value`,**`Struct { fields }` 接 `&[Field]`**(取代 `&HashMap<String, Type>`),按字段名匹配——不再位置强绑。

### 4.3 Trait 一致性

每个 trait 在 10 数值变体上的策略:

| Trait | 策略 |
|-------|------|
| `Serialize` | `serializer.serialize_i8/i16/i32/i64/u8/u16/u32/u64/f32/f64` |
| `PartialEq` | 同宽度直接 `==`;同族跨宽度(Int8 vs Int64)用 `as` 升位;跨族 Int/Uint 走 `i128` 升位;跨族与 Float 走 `as f64`;不同族的"字符串-数字"互比始终不等 |
| `PartialOrd` | 同宽度直接 `partial_cmp`;同族跨宽度升位;跨族(Int/Uint)走 `i128`;Float vs Int/Uint 走 `f64` |
| `Hash` | 数值变体用原生 `Hash`;`f32`/`f64` 用 `to_bits()` 后 hash |
| `Display` | 各宽度 `{}` 字符串形式 |

**前提**:parse 时已经保证数值在声明宽度内,跨宽度升位不会溢出。若某个跨宽度比较确实溢出(极端 i64 ↔ u64 在 i128 范围内已可表达),Rust 默认 wrap 由调用方承担——这是极少数 corner case,不要求额外诊断。

---

## 5. 表级约束接通

### 5.1 row 5 约定

Excel sheet 行 5(原本预留)装载表级约束。每 cell 一条约束,使用与 row 4 相同的 `@func(args)` DSL,**cell 所在列与该约束无关**,约束归表。

示意:

```
| id  | name   | level    |              ← 行 1 字段名
| int | string | int      |              ← 行 2 类型
| 唯一 | 名称   | 等级     |              ← 行 3 注释
| @seq |       |          |              ← 行 4 字段约束
| @unique(id, name) |      |              ← 行 5 表级约束
| 1  | alice   | 1        |              ← 行 6 数据
| 2  | bob     | 2        |
```

### 5.2 解析与执行

- `read_excel` 在 row 5 收集所有 cell,逐个调 `Constraint::from_str`,成功即 `table.constraints.push(constraint_with_loc(row=5, col=N))`,失败用 `Diagnostic { code: TableConstraintParseError, location }` 收纳。
- 字段约束(row 4)与表级约束(row 5)在 `ConstraintValidator::validate_table` 中两段循环:

```rust
fn validate_table(table: &Table) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    // 字段约束(行 4)
    for field in &table.fields {
        if let Some(c) = &field.constraint {
            if let Err(err) = c.validate(&[field.clone()], &table.data) {
                diags.push(c.to_diagnostic(&err, …));
            }
        }
    }
    // 表级约束(行 5,接入本次实现)
    for c in &table.constraints {
        if let Err(err) = c.validate(&table.fields, &table.data) {
            diags.push(c.to_diagnostic(&err, …));
        }
    }
    diags
}
```

### 5.3 `Constraint` 结构扩展

```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Constraint {
    pub func: String,
    pub args: Vec<String>,
    pub location: SourceLocation,   // 新增
}
```

`from_str(s, loc)` 新签名注入位置;`from_str(s)` 保留(默认 location 为空),便于纯字符串解析场景。

### 5.4 错误码映射

| 约束 | Diagnostic code |
|------|-----------------|
| `unique` 复合冲突 | `ConstraintCompositeMissing`(行号在 message) |
| `seq` 断号 | `ConstraintSequenceBroken` |
| `order` 违反 | `ConstraintOrderViolation` |
| 字段不存在 | `ConstraintFieldMissing` |
| 未知 func | `ConstraintUnknown` |
| row 5 cell 解析失败 | `TableConstraintParseError` |

### 5.5 现状差异

- `Constraint::validate_xxx` 内已有 `Result<(), String>` 返回,需重构为 `Result<(), ValidationError> { row, fields, message }`
- `#[serde]` 字段加 `location` 是新增字段,反序列化时 `serde` 默认对缺失字段报错,所以**对 json 持久化已有 `Constraint` 的下游是 breaking**。本 spec 范围内只有 `Meta`、`Table.constraints` 序列化,影响范围可控

---

## 6. 稳定 hash 与 Meta 扩展

### 6.1 字段变更

```rust
// meta.rs
pub struct Meta {
    pub version: String,         // CARGO_PKG_VERSION
    pub hash: [u8; 32],          // 改:Blake3 输出
    pub build_at: i64,           // SystemTime unix 秒
    pub source: Vec<PathBuf>,    // 新增:本次编译包含的输入文件
    pub tool: ToolVersion,       // 新增:工具版本指纹
}

pub struct ToolVersion {
    pub tablec: String,         // = version
    pub calamine: &'static str,
    pub serde_json: &'static str,
    pub blake3: &'static str,
}
```

`hash` 显示为 hex(`impl Display for Meta` 输出前 16 字节 hex)。

### 6.2 Blake3 集成

- 依赖:`blake3 = "1"`(待确认最新 minor)
- 派生键 `Blake3::new_derive_key("tablec.project.v1")`
- hasher 单实例贯穿整 Project 计算,避免多次 final 切分
- `Meta` 自动 `Serialize/Deserialize`,`[u8; 32]` 在 serde_json 中序列化为数字数组(若需 hex string 见 §6.5)

### 6.3 calculate_hash 算法

```rust
pub fn calculate_hash(&mut self) {
    let mut hasher = Blake3::new_derive_key("tablec.project.v1");
    hasher.update(self.name.as_bytes());

    // 按 sheet 名字典序,确保顺序无关
    let mut sheets: Vec<(&String, &Table)> = self.tables.iter().collect();
    sheets.sort_by(|a, b| a.0.cmp(b.0));

    for (sheet_name, table) in sheets {
        hasher.update(sheet_name.as_bytes());

        // schema(字段声明顺序由 IndexMap 保住,Vec → canonical JSON)
        let fields_canon = serde_json::to_vec(&canonical_fields(&table.fields))
            .expect("fields always serializable");
        hasher.update(&fields_canon);

        // 数据行(行序敏感)
        for row in &table.data {
            let row_canon = serde_json::to_vec(&row.fields)
                .expect("row always serializable");
            hasher.update(&row_canon);
        }
    }

    self.meta.hash = *hasher.finalize().as_bytes();
}
```

`canonical_fields(vec<&Field>)` 输出为 sorted-key 的 JSON,确保字段声明顺序在不同来源下也产出同一 hash。

### 6.4 行序敏感性

- 数据行按 `table.data` 的原有顺序进入 hasher
- 任何插入/删除/调换必变 hash(序列化字节流变化)
- 字段声明顺序不进入 hash(由 `canonical_fields` 归一化)——字段顺序属于表达细节,不应影响产物指纹

### 6.5 Hash JSON 表示

- `Meta::hash` 在 JSON / msgpack 中表示为 **64-char hex 字符串**(`[u8;32]` 的完整 2 字符/字节映射)
- 实装:`Meta` 自定义 `Serialize`/`Deserialize` 把 `[u8;32]` 字段输出为 hex 字符串,避免 serde 默认的 32 个数字数组
- `impl Display for Meta` 输出 `hash=<hex16> version=… build_at=… source=[…] tool={…}`
- 这是 breaking:testsuite 现有 fixture 若直接比较 JSON 字节需 snapshot update

### 6.6 下游影响

- `testsuite` 现有 fixture 若断言 `meta.hash` 字段,需更新
- testsuite `tablec_integration_tests` 工程内 `tools/compare.py` 可能也要适配新格式
- `binding-python` 若 re-export `Meta`,需要同步重新导出新字段(本 spec 不修复,但 c6 后 binding-python 编译会被破坏)

---

## 7. 单元测试策略

### 7.1 每 commit 必走节奏

1. `cargo build -p tablec-core` 通过
2. `cargo test -p tablec-core` 通过
3. 触及 value data plumbing 的 commit(本 spec 内 c3, c4, c5)同步跑 `pytest /home/bot/workbench/repos/tablec-testsuite`
4. snapshot/断言失败必须**逐个确认是 design 预期变更**后再用 `bash scripts/update_snapshots.sh --apply` 批改

### 7.2 各 commit 测试增量

| Commit | 必测 |
|--------|------|
| c1 diagnostic | `Diagnostic` Serialize roundtrip;`Display` 三种空 vs 满 loc;`DiagnosticCode` 大小冻结 |
| c2 tokenizer Result | happy path 保留;**新增** `scan_tokens("int🙂")` → `Err(TokenizerUnexpectedChar)` 带 loc;`scan_tokens("int<>")` OK;空串 OK + 空 Vec |
| c3 Value 八宽度 | 每个宽度 `(parse_value("42", Int8..), FieldType) -> Value` 对应变体;**`"200"` 进 Int8 → `Err(ValueOutOfRange)`** message 含 `[-128, 127]`;空串/非数字 → `ValueParseError`;cross-width `PartialEq` 同字面同值;跨族 `PartialOrd` 样本;Float32 vs Float64 不等;`Serialize` 在 JSON 中类型无损 |
| c4 read_excel | 故意 fixture:cell "abc" typed int → Diagnostic w/ row+col;多个错误聚合;happy path 与原 fixture 100% 字节一致(向后兼容就靠这条) |
| c5 constraint table-level | composite unique 在两 Field 同时 uniq → pass;重复 → err 带行号 + 字段集;Project::validate_all 收各 sheet 的错误;row 5 损坏 → `TableConstraintParseError` |
| c6 Blake3 hash | 同 Project 两次调用 hash 相等;调换两行 → hash 不同;删除一行 → hash 不同;增列 → hash 不同;`Meta::hash_hex()` Display 64-char hex |

### 7.3 共享测试工具

`tablec-core/tests/common/mod.rs`(或 lib 内 `#[cfg(test)] mod common`):

```rust
pub fn expect_diagnostic(errs: &[Diagnostic], code: DiagnosticCode) -> &Diagnostic
pub fn assert_in_range(result: &Result<Value, Diagnostic>, range: (i128, i128))
pub fn fixture_path(name: &str) -> PathBuf
```

仅 `#[cfg(test)]` 下,不放生产代码。

### 7.4 故意失败 fixture

新增 `tablec-core/tests/fixtures/error_cases/`:

- `bad_int_range.xlsx` — int8 列里放 200、300
- `bad_struct_field.xlsx` — struct 字面字段名错配
- `bad_unique_constraint.xlsx` — 故意重复行 6/7 的特定字段

跑 `cargo test` 时这些 fixture 触发错误路径;`tablec-testsuite` **不引用**(避免循环)。

### 7.5 snapshot 批改原则

- `update_snapshots.sh --apply` 仅在确认 design 行为变更后用
- 每批改必须配对一个 commit message 说明原因(例如 `c6: meta.hash 字段类型从 i64 → [u8;32],JSON 表示为 hex`)
- 一次性批量更新禁止

---

## 8. 落地节奏与回滚

### 8.1 6 个 commit 顺序与依赖

```
c1  diagnostic types       (纯加,零风险,锁 Diagnostic API)
c2  tokenizer Result       (改 1 个函数签名,改调用点透传)
c3  Value 八宽度           (enum 重排,大 commit)
   c3.1 binding-python 同步 (c3 后立即,独立 commit)
c4  read_excel Result      (改返回类型,CLI 改 caller,testsuite 跑通)
c5  constraint table-level  (constraint.rs + table.rs)
c6  hash → Blake3 + Meta   (字段类型变,Meta Serialize 改写)
```

### 8.2 每 commit 回滚粒度

| Commit | 失败征兆 | 回滚策略 |
|--------|----------|----------|
| c1 | 编译未过 | 整体回退,无外部影响 |
| c2 | 自定义 panic 没全收 | 整体回退,影响点只有 `FieldType::from_str` |
| c3 | enum match 漏补全 | 拆 c3 为 c3a/c3b 各自独立可回退 |
| c4 | testsuite hash 不匹配 | 跑一次确认 design 预期 → `update_snapshots.sh --apply` |
| c5 | 老 fixture 暴露未生效的"自以为是"约束 | 修 fixture,**不在本 commit 改 schema 语义** |
| c6 | testsuite hash 字段断言失败 | snapshot update;每条断言逐项 diff 后批改 |

### 8.3 c3.1 binding-python 同步

c3 把 `Value` enum 整体重排,binding-python 内部 `impl IntoPy<PyObject>` / `FromPyObject` 必须同步重写。**不允许** main 在 c3 后保持 binding-python 编译失败状态超过一个 commit。

### 8.4 CLI 错误处理

CLI 的错误打印不是本 spec 的范围,但 read_excel 改 `Result<_, Vec<Diagnostic>>` 后 CLI caller 处的语法报错会被静默吞或 `expect()`。**本 spec 接受 CLI 暂时不友好**,后续单独 spec 处理错误呈现层。

---

## 9. 关键决策与权衡

### 9.1 已选方案

| 决策 | 选择 | 否决方案 |
|------|------|----------|
| 错误模型 | 结构化 `Diagnostic` + `SourceLocation` | `String`, anyhow/thiserror |
| 类型宽度 | Value 十变体 + parse 时范围检查 | 仅运行时检查(Value 不变) |
| Hash 算法 | Blake3 | SipHasher、SHA-2、FxHash |
| Hash 行序 | 敏感 | 不敏感(已被用户明确否) |
| 表级约束来源 | Excel row 5 | tablec.toml([tables.*]) |
| 落地节奏 | 单一 spec,6 commit | 单 commit,多 spec |

### 9.2 保留旧有行为

- `Project`、`Format`、`Config` 的总体方向不动
- Excel 前 5 行约定不变,只新增 row 5 的 syntax(同 row 4 DSL)
- JSON / msgpack 输出顶层 schema 不变,只内部 `meta.hash` 字节变化

### 9.3 已知后续 spec

- **tablec-cli 错误呈现层**:Diagnostic → 漂亮打印、退出码
- **plugin 模块处置**:删除或下沉到 examples
- **binding-python Value 同步升级**(本 spec c3.1 commit 包含最小改动,完整升级另起 spec)

---

## 10. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| c3 enum 重排使大量 match 漏改 | 编译报错量大 | 实施时先列出所有 `match` Value 的位置,逐步改;拆 c3 为 c3a/c3b |
| c4 暴露既有 fixture 的脏数据 | testsuite 挂 | 逐 fixture review;不要一次性 batch update |
| testsuite 现有断言破坏 | CI 红 | snapshot update 仅作最后一步 |
| binding-python 升级遗漏 | c3 后 main 编译失败 | c3.1 与 c3 同 batch 落地 |
| Meta hex JSON 表示破坏外部 fixture | 实装人选先看 fixture | sample diff 后再实装 |

---

## 11. 关闭的开放点

- 跨族数字比较溢出:极少数 corner case 被 Rust 默认 wrap 承担,设计接受(参 §4.3)
- `Meta.hash` JSON 表示:定为 64-char hex 字符串(参 §6.5)
- testsuite snapshot 批改规则:见 §8.2 与 §7.5
- plugin 模块处置:不在本 spec,见 §9.3

---

## 12. 附录

### 12.1 与现有 test 集的关系

- `tablec-core/tests/value_tests.rs`、`value_parser_tests.rs`、`integration_tests.rs`、`export_tests.rs`、`constraint_tests.rs` 在每个相关 commit 后必须仍通过
- 修改它们的代码时,**优先扩展现有测试文件**,不另起;新建测试文件仅在跨模块行为时

### 12.2 references

- skill: `superpowers:brainstorming` — 本 spec 经该 skill 流程产出
- skill: `superpowers:writing-plans` — 本 spec 落地时由该 skill 转 implementation plan
- 相关:`2026-07-04-tablec-integration-tests-design.md`(`tablec-testsuite` 内)—— 仅 cross-ref,不修改
