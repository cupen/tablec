# tablec-core 设计回顾与四项清理 — 设计稿

**日期**: 2026-07-13
**仓库**: `repos/tablec`
**前置**: PR #1 `feat/tablec-core-cleanup` 已合入 (`main @ 0988f3a`)
**范围**: `tablec-core` 内部去重 + `tablec-cli` 错误呈现收口
**不做**: ANSI color / `--format json` 诊断输出 / plugin 重新设计 / binding-python 升级 / protobuf 模块

---

## 1. 背景与目标

PR #1 解了"运行时正确性"债，但留下了 4 处二级改进：僵尸代码、模块去重、Trait 实现可维护性、错误呈现一致性。本 spec 集中处理这 4 项，落地为 4 个独立 commit，各 commit 互不阻塞、可独立回退。

### 1.1 本 spec 不改变

- `Project`/`Format`/`Config` 接口
- `Value` / `Type` / `FieldType` 对外形状（variant 数、序列化 wire format）
- `Diagnostic` 与 `SourceLocation` 结构
- CLI 命令行界面
- Excel row 1-5 约定

### 1.2 决策摘要

| 维度 | 选择 |
|------|------|
| 范围 | 4 项按独立 commit 落；每 commit 必过 build + test |
| 兼容性 | 全部允许 breaking（私有项目，但内部可调整） |
| plugin | 整体删除（不下沉、不重设计） |
| validator | 保留 `ConstraintValidator`，删除 `validator.rs`，删除 `numeric_i64` 复本 |
| Value 抽象 | 引入私有 `Numeric` 类型，trait impl 不变对外行为 |
| CLI 错误 | 新增 cli 内部 `diag_render` 模块，消除 4 处复制粘贴 |
| Float 比较 | 改用 bitwise exact（`a.to_bits() == b.to_bits()`）；doc comment 写明设计选择 |

---

## 2. 改动一：plugin 模块移除

### 2.1 证据

`tablec-core/src/core/plugin.rs` (245 行) 内置 3 个 plugin：

- `JsonFormatterPlugin` — 把 JSON 字符串重新 pretty-print；`export/json.rs` 已做
- `DataValidatorPlugin` — 输出 `{valid, errors, warnings}` 报告；逻辑与 `validator.rs`/`ConstraintValidator` 重叠
- `CsvExporterPlugin` — 接受 JSON 输入生成 CSV；`Project::export` 不支持 CSV，无 caller

`lib.rs:8 pub use core::plugin::*` 把 plugin 名字暴露为 public API 但**生产代码 0 处调用**（grep `PluginManager\|JsonFormatterPlugin\|DataValidatorPlugin\|CsvExporterPlugin` 在 `tablec-cli/src/`、`binding-python/src/` 内无匹配）。

测试为注释模块 (`plugin.rs:218-246`)。

### 2.2 改动清单

| 文件 | 改动 |
|------|------|
| `tablec-core/src/core/plugin.rs` | **删除** |
| `tablec-core/src/core/mod.rs` | 删 `pub mod plugin;` |
| `tablec-core/src/lib.rs` | 删 `pub use core::plugin::*;` |
| `docs/superpowers/specs/2026-07-05-tablec-core-cleanup-design.md` §9.3 | 把"plugin 模块处置"从开放点列表删除 |

### 2.3 验证

```bash
grep -rE 'PluginManager|JsonFormatterPlugin|DataValidatorPlugin|CsvExporterPlugin|create_default_plugin_manager' \
  tablec tablec-cli binding-python \
  --include='*.rs' --include='*.toml' --include='*.md'   # 应为 0 命中（除本 spec 引用外）
cargo build -p tablec-core
cargo test -p tablec-core
cargo build -p tablec-cli
```

`cargo build -p binding-python`（c3.1 已在 PR #1 同步；本 spec 不再次触及）

### 2.4 commit

`chore(core): remove unused plugin module` — 单 commit，含 spec §2.2 全表。

---

## 3. 改动二：validator 去重

### 3.1 证据

`validator.rs::validate_table`（5-59 行）与 `constraint.rs::ConstraintValidator::validate_table`（222-246 行）**实现相同**：

- 同样 `Result<(), Vec<Diagnostic>>` 签名
- 同样"field-level 循环 + table.constraints 循环"
- 同样通过 `constraint.to_diagnostic(&msg)` 收 Diagnostic

CLI 调前者 (`check.rs:5 validate_table`)，后者无 caller。

`numeric_i64` 在两个文件**逐字重复**：`validator.rs:134-147` 与 `constraint.rs:86-100`。

### 3.2 改动清单

| 文件 | 改动 |
|------|------|
| `tablec-core/src/core/table/validator.rs` | **删除**整文件（含 `numeric_i64`） |
| `tablec-core/src/core/table/mod.rs` | 删 `pub mod validator;` |
| `tablec-cli/src/cmd/check.rs:5` | import 改 `use tablec_core::core::table::constraint::ConstraintValidator;`，调 `ConstraintValidator::validate_table(&table)` |
| `tablec-core/src/lib.rs` | `pub use core::table::*` 已经导出 constraint 模块，validator 删除后自动不含 — 无文本改动 |

`ConstraintValidator::validate_table` 接收 `&Table`，不是 `&[Field], &[Row]`，所以 `check.rs` 调用形态不变。

### 3.3 验证

```bash
grep -rE 'validate_table|validator::|use.*validator' \
  tablec tablec-cli --include='*.rs'      # 应只剩 ConstraintValidator::validate_table 一处定义 + check.rs 调用
cargo build -p tablec-core
cargo test -p tablec-core test_constraint   # 现有 test 仍过
cargo test -p tablec-core test_validator   # 已无此模块，依赖项消失亦 OK
cargo build -p tablec-cli
```

### 3.4 commit

`refactor(core): dedupe validator into ConstraintValidator` — 单 commit。

---

## 4. 改动三：Value 数值抽象（私有 Numeric）

### 4.1 目标

`Value` 16 变体外的 5 个 trait impl 各手写 10-arm numeric match。新增内部 `Numeric` 类型吸收这层展开，trait impl 写法变为"先 to_numeric，否则外层 match"。**公共 API、wire format、variant 数量不变。**

### 4.2 新增内部类型

```rust
// tablec-core/src/core/table/value.rs（私有，可见性 = crate 私有）
#[derive(Debug, Clone, Copy)]
enum Numeric {
    I8(i8), I16(i16), I32(i32), I64(i64),
    U8(u8), U16(u16), U32(u32), U64(u64),
    F32(f32), F64(f64),
}
```

私有辅助：

```rust
impl Value {
    fn to_numeric(&self) -> Option<Numeric> { match self { /* 10 个 numeric 分支 */ _ => None } }
    fn from_numeric(n: Numeric) -> Self { match n { /* 10 个分支返回 Value::Int8(..) 等 */ } }
}
```

`Numeric` 自带一份 trait impl（与 `Value` 平行），trait impl 改写示例：

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
            Value::Map(m)    => { /* 不变，序列化逻辑按原 §4.5 (90-87 行) 保留 */ },
        }
    }
}

// 平行 impl：Numeric 自身承担一遍展开
impl Serialize for Numeric {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Numeric::I8(n)  => s.serialize_i8(*n),
            Numeric::I16(n) => s.serialize_i16(*n),
            /* ... 8 个 numeric 分支 ... */
        }
    }
}
```

类似改写作用于 `PartialEq`、`Hash`、`PartialOrd`、`Display`。

`Deserialize` 走 `Visitor`，Visitor **不动**（其 visit_i8/u8/visit_f64 实现是类型层必须展开），但 visitor 内可重复用 `to_numeric`/`from_numeric` 路径而不是 match — 评估：保持 `Visitor` 现状、只把 5 个 trait impl 改写，方案更小。

### 4.3 PartialEq 对 Float 的处理

改用 **bitwise exact match**（`a.to_bits() == b.to_bits()`），不再使用 `EPSILON` 近似比较。

设计理由：

- `EPSILON` 是最小正正常浮点数相对差，不是绝对误差；用它做相等比较是语义混淆
- IEEE 754 浮点有明确的位表示，按位比较是可预测、可推理的
- 跨 crate 序列化/反序列化时（JSON / msgpack），位级表示是稳定不变的
- 用户若需要容差比较，自己加 wrapper（`approx` crate 等），不该内建在 `PartialEq` 里

在 `value.rs` 内 `impl PartialEq for Value` 上方加 doc comment 写明：

- `NaN != NaN`（IEEE 754，by design）
- `+0.0 == -0.0` 在 `to_bits()` 下为 `false`（位不同），这是按位比较的固有性质
- 不再使用 `EPSILON` 近似

impl 通过 `Numeric::eq`（按位）实现。

这是显式行为变更：本 spec 范围内 breaking change，不保持向后兼容。

### 4.4 不动

- `Value` variant 数量（仍是 16）
- `Type` 与 `FieldType`
- JSON / msgpack wire format
- 用户侧 `match Value` 代码
- `Visitor` 现有形态（评估后保留）

### 4.5 测试

扩 `tablec-core/tests/value_tests.rs`：

- `numeric_helper_round_trip`：遍历 10 个 numeric 字面，`v == from_numeric(to_numeric(v).unwrap())`
- 现有 `value_size_is_sixteen_variants`、`cross_width_partial_ord_promotes`、`serialize_each_numeric_variant` 仍 pass

### 4.6 commit

`refactor(core): extract Numeric from Value for impl dedup` — 单 commit。

---

## 5. 改动四：CLI 诊断呈现收口

### 5.1 目标

消除 `tablec-cli` 内 4 处复制粘贴的 diagnostic 打印代码。新增 cli 内部 helper 模块 `diag_render`，不引新依赖（ANSI color 为下一轮 spec）。

### 5.2 新增模块

```
tablec-cli/src/diag_render.rs
```

公开（crate 内）API：

```rust
use std::io::{self, Write};
use tablec_core::core::diagnostic::{Diagnostic, Severity};

/// 把 diagnostics 写到 `out`（一行一条；首字段为 severity 字面）。
/// 不写 header/trailer — 调用方自行控流。
pub(crate) fn render_diags<W: Write>(diags: &[Diagnostic], out: &mut W) -> io::Result<()>;

/// 由 diagnostic 集合决定进程退出码。
/// 当前：第一个 Error → 1；其他 → 0；空 → 0。
pub(crate) fn diag_exit_code(diags: &[Diagnostic]) -> i32;

/// 把 `[Diagnostic]` 转成简短的 summary：`"3 errors, 1 warning"`。
pub(crate) fn diag_summary(diags: &[Diagnostic]) -> String { /* severity 计数 */ }
```

`render_diags` 的输出格式（无 ANSI）：

```
error   TypeParseError [Sheet1] 2:5 [/abs/path/foo.xlsx]: Unknown type: foo
warning ConstraintOrderViolation [Items] 6:1 [/abs/path/foo.xlsx]: ...
```

字段顺序：`severity\t<Diagnostic Display>\t<file?>`。`Diagnostic::Display`（core 已实现）输出 `<code> [sheet] line:col: <message>`；`file` 由 cli 模块补（core `Display` 不读 `location.file`，参 §5.4）。

### 5.3 替换 4 处粘贴

| 位置 | 当前 | 改为 |
|------|------|------|
| `tablec-cli/src/cmd/build.rs:116-123` (build_single_file) | 6 行 match + eprintln | `render_diags(&errs, &mut io::stderr().lock())?; return Err(format!("read_excel failed").into());` |
| `tablec-cli/src/cmd/build.rs:150-157` (build_merged_files) | 同上 | 同上 |
| `tablec-cli/src/cmd/build.rs:182-188` (build_to_string) | 同上 | 同上 |
| `tablec-cli/src/cmd/check.rs:91-96` | `eprintln!("  Error: {}", d)` 循环 | `render_diags(&errs, &mut io::stderr().lock())?;` |

`build_to_string` 返回字符串给 Python 库，不写 stderr — 改为丢日志或直接返回带 Diagnostic 的错误；本 spec 选最简：保留 stderr 写一行 + 返回 Err，**Python 链路可见与之前一致**，后续在 binding-python spec 里换返回类型。

### 5.4 不动 core::Display

`Diagnostic::Display` 不动（避免破坏外部 Diagnostic JSON 用户）。`file` 字段由 cli 端 `format!(" [{}]", loc.file.display())` 补上。

### 5.5 测试

`tablec-cli/src/diag_render.rs` 同文件 `#[cfg(test)]`：

- `render_diags_writes_one_line_per_diag` — 2 条 diagnostic → output 有 2 行
- `render_diags_includes_file` — SourceLocation { file: Some("/x.xlsx"), .. } 输出包含 `/x.xlsx`
- `render_diags_skips_missing_file_gracefully` — None 时不挂
- `diag_exit_code_first_error_returns_1`
- `diag_summary_counts_severity`

`check.rs` / `build.rs` 无单测（Cargo CLI 调用方式复杂），靠手工 `cargo run -- check path/` 验证。

### 5.6 commit

`feat(cli): consolidate diagnostic rendering (no ANSI yet)` — 单 commit。

---

## 6. 落地节奏与回滚

### 6.1 顺序与依赖

```
c1  plugin 移除            (零依赖，对其他三 commit 无影响)
c2  validator 去重         (零依赖，仅 check.rs import)
c3  Value Numeric 抽象     (零依赖，公共 API 不变)
c4  CLI diag_render        (零依赖，只动 cli crate)
```

四个 commit 互不阻塞。可并行可串行。本 spec **不**给出合并节奏，由调用方决定（PR 大小、CI 速度等）。

### 6.2 每 commit 回滚粒度

| Commit | 失败征兆 | 回滚策略 |
|--------|----------|----------|
| c1 (plugin) | 调用方其实存在但未在我 grep 范围内 | `git revert c1`，找出漏掉的 caller 再补 |
| c2 (validator) | 测试漏过的 implicit call | `git revert c2`，先扩 test 再重做 |
| c3 (Value Numeric) | trait impl 行为差异 | `git revert c3`；改回单 match 写法 |
| c4 (diag_render) | 输出格式破坏下游快照测试 | `git revert c4`；先 alignment 输出再重做 |

### 6.3 测试节奏

每个 commit：

```bash
cargo build -p tablec-core -p tablec-cli
cargo test -p tablec-core
cargo test -p binding-python -p tablec-testsuite || true   # 后两个仓库 spec 没动，看 regression
```

c4 完成后单独跑：

```bash
cargo run -p tablec-cli -- check path/to/valid/xlsx
cargo run -p tablec-cli -- check path/to/invalid/xlsx   # 应见 error 行
```

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| c1 grep 漏掉 caller | 编译通过但运行时 panic | grep 加 pattern `Plugin\|plugin\|plugin_manager`，把 `--include='*.rs'` `--include='*.toml'` 全勾；找不到再 `find` 二次人工 |
| c2 `ConstraintValidator::validate_table` 签名 vs `validator::validate_table` 不兼容 | check.rs 编译失败 | commit 前在本地编译验证 |
| c3 `to_numeric`/`from_numeric` 没覆盖 Null/Map 等非 numeric | 测试 fail | `to_numeric` 返回 `Option<Numeric>`，trait impl 内显式 fallback |
| c3 float 比较语义变化（bitwise 替代 EPSILON） | 既有测试 fail | 显式 breaking change；新增 `numeric_helper_round_trip` 测试覆盖新语义；`cross_width_partial_ord_promotes` / `value_size_is_sixteen_variants` 仍绿 |
| c4 输出格式破坏下游 snapshot | testsuite 挂 | 本 spec 不动 wire format，只动 stderr 文案；testsuite 不应对 stderr 文案做 byte-equal |
| c4 build_to_string 行为变化 | Python 链路挂 | 维持 stderr 写一行 + Err；不改返回值 |

---

## 8. 关键决策与权衡

| 决策 | 选择 | 否决方案 |
|------|------|----------|
| plugin 处置 | 删除 | 下沉到 examples/、保留为可选子系统 |
| validator 留谁 | `ConstraintValidator` (constraint.rs) | 反向：把 `ConstraintValidator` 并入 `validator.rs` |
| `numeric_i64` 留谁 | 与 `ConstraintValidator` 同文件 | 抽到新文件 `utils.rs` |
| Value 抽象层级 | 私有 `Numeric` enum | macro、trait 对象、外部 crate |
| Float 比较 | **bitwise exact (`a.to_bits() == b.to_bits()`)** | EPSILON 维持向后兼容（被否） |
| `diag_render` crate | `tablec-cli` 内部模块 | 进 `tablec-core` 让 binding 复用 |
| 输出格式（cli） | severity 前缀 + 复用 core Display | 自定义 `fmt::Display` 直写 |
| ANSI color | 不在本 spec | 下一 spec 单独 `owo_colors`/`yansi` |

---

## 9. 不在本 spec 范围

- ANSI color / `owo_colors` 集成（需要单独 spec，含设计主题与禁用开关）
- Diagnostic 的 JSON/machine-readable 输出格式（`--format json-diag`）
- `binding-python` Value enum 完整同步升级（PR #1 c3.1 是最小同步；完整版另开 spec）
- `protobuf` 输出模块（CLAUDE.md 提到但 `src/export/` 当前无实现，独立 spec）
- `web` 命令的 hello endpoint 去留（CLAUDE.md 提到但代码极小，独立 spec）

---

## 10. 开放点（已关闭）

- plugin 是否需要保留 API stub（让外部 crate 自己实现）？**否**——本 spec 全删，理由是"没人用即零成本"
- 私有 `Numeric` 是否 pub(crate) 暴露给 tests？**保持私有**，靠 round-trip 测试覆盖
- Float partial_cmp 是否不变？**是**，仅写 doc comment

---

## 11. 附录

### 11.1 与 PR #1 节奏的关系

PR #1 落地为 6 commit 并已合并。本 spec 4 commit 是后续小修，逻辑分离：

- c1 (plugin) — 与 PR #1 c1-diagnostic 同步可做，但 spec §9.3 当时记为后续 spec；本 spec 接续
- c2 (validator) — 与 PR #1 c5-constraint 同范围但未触及；本 spec 接续
- c3 (Value Numeric) — 与 PR #1 c3-Value 八宽度 同范围但未触及 trait impl 可维护性；本 spec 接续
- c4 (diag_render) — 与 PR #1 c4-read_excel 互补；c4 把 Diagnostic 推上来，CLI 没接，本 spec 接续

### 11.2 references

- 上游 spec: `2026-07-05-tablec-core-cleanup-design.md` — §9.3 列出的三个后续 spec 中"plugin 模块处置"对应本 spec §2
- 上游 spec: `2026-07-05-tablec-core-cleanup-design.md` — §6.5 / §7 详细描述 Meta 与 Diagnostic，决定 c3/c4 的边界
- skill: `superpowers:brainstorming` — 本 spec 经该 skill 流程产出
- skill: `superpowers:writing-plans` — 本 spec 落地时由该 skill 转 implementation plan
- 相关: `tablec-testsuite` 内的 `2026-07-04-tablec-integration-tests-design.md` — 与本 spec 无直接依赖
