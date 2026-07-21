# tablec 约束系统扩展 — 设计稿

**日期**: 2026-07-15
**仓库**: `repos/tablec`
**范围**: `tablec-core` 内 `Constraint` 子系统; 含参数解析的引号语义升级
**不做**: 浮点路径覆盖 (下一 spec)、约束推导 / 自动类型生成、可视化编辑器

---

## 1. 背景与目标

### 1.1 现状

`tablec-core` 当前只支持 3 种跨行约束: `@unique`、`@seq`、`@order`。这套约束只覆盖"行之间的关系"，没有覆盖"单格是否合法"、"行内多列是否一致"、"跨表是否引用得到"这三类同样基础的需求。结果是同样的合法性检查在调用方手动重做、或散落在 Python 业务脚本里,无法被统一的 `tablec check` 捕获。

`@func(args)` 的现有语法用 `,` 分隔字符串,参数中不含引号 — 这导致 `@pattern("[a-z]+")` 这种必须保留正则字面量的场景无法表达。

### 1.2 目标

围绕"手填数据容易出错"补足 4 层共 **14 个具名约束**; 升级参数解析支持双引号字符串; 保持单测覆盖率 ≥ 95%。

### 1.3 不在本 spec 范围

- 浮点路径上的 `@seq`/`@order` (浮点 ε 比较,需独立设计)
- 约束错误在 CLI 输出时的美化 / 颜色 / 退出码
- Python binding 的兼容
- 错误信息的中文化 / i18n
- 跨 cell 的格式正则的"性能预算"审计(约束本身 ≤ O(rows × cols))

---

## 2. 关键设计选择

| 维度 | 选择 |
|------|------|
| 命名空间 | 沿用 `@func(args)` 单层命名,不再加前缀 |
| 字段级 vs 表级 | 一律允许两处声明; 跨字段约束只能在表级 |
| 浮点 | 不进本轮; 错误信息明示"需要 Int*" |
| `@unique` 与 NULL | SQL 风格:空 / NULL cell 不参与唯一比较,但**仍记入总数**;`@unique` 与先前版本的唯一差异是空值不再冲突。 |
| `@range` / `@min` / `@max` | 不提供 `@range(min,max)`,直接用 `@min` + `@max` 复合在同字段上 |
| `@id` 命名 | 主键语义用 `@id` 而不是 `@pkey` |
| 参数引号 | 解析器升级识别 `"..."`; 不引号 → 历史行为不变 |
| 引号转义 | 支持 `\"` 与 `\\`; 其它 `\X` 原样保留 |
| 跨表 FK | 引入"项目级约束"; `ConstraintValidator::validate_project` 接收 `&[Table]` |
| 错误信息 | 沿用 `Diagnostic` + `SourceLocation`, 新增 enum variant |
| 兼容性 | 现有 constraint (unique/seq/order) 行为不变 |
| 实施节奏 | 一个 spec, 落地为 6 个 commit |

---

## 3. 约束目录

围绕"手填容易出错"的目标设计,保留 **14 个具名约束**:

| 层级 | 约束 | 关注点 |
|---|---|---|
| Layer 1 (字段级) | `@notnull`, `@min`, `@max`, `@oneof`, `@maxlen`, `@pattern` | 单格值域 |
| Layer 2 (表级) | `@eq`, `@gt`, `@lt` | 行内多列一致性 |
| Layer 3 (字段/表级) | `@unique`, `@id`, `@seq`, `@seq(step)`, `@order` | 行间关系 |
| Layer 4 (项目级) | `@ref` | 跨表引用 |

### 3.1 单格级 (Layer 1)

| 名称 | 语法 | 级 | 适用 | 失败 |
|------|------|----|------|------|
| `@notnull` | `@notnull` | 字段 | 任意 | cell 为空字符串 / `Value::Null` |
| `@min(n)` | `@min(0)` | 字段 | 整数 | n < min |
| `@max(n)` | `@max(100)` | 字段 | 整数 | n > max |
| `@oneof(...)` | `@oneof("a", "b", "c")` | 字段 | 整数/字符串 | 值不在集合中 |
| `@maxlen(n)` | `@maxlen(16)` | 字段 | 字符串 | 字符数 > maxlen |
| `@pattern("regex")` | `@pattern("^[a-z]+$")` | 字段 | 字符串 | 正则不匹配 |

闭区间 `[lo, hi]` 用同一字段叠加 `@min(lo)` + `@max(hi)` 表达。

### 3.2 行内跨字段 (Layer 2) — 表级

| 名称 | 语法 | 适用 | 失败 |
|------|------|------|------|
| `@eq(host, other)` | `@eq(total, subtotal)` | 整数/字符串 | host ≠ other |
| `@gt(host, other)` | `@gt(price, cost)` | 整数 | host ≤ other |
| `@lt(host, other)` | `@lt(price, ceiling)` | 整数 | host ≥ other |

只用严格不等式。`host ≥ other` 用 `@gt` 错位参数表达(把 `other` 列减 1)或归到业务层;`@eq` 已经把"等于"作为特例。**任一引用 cell 为空时该 row 跳过**(用 `@notnull` 强制非空)。

host 或 other 字段不存在 → `ConstraintCrossFieldMissingColumn`。

### 3.3 表内跨行 (Layer 3)

| 名称 | 语法 | 级 | 失败 |
|------|------|----|------|
| `@unique` | `@unique` / `@unique(a, b)` | 字段(单) / 表(组合) | 非空组合重复出现 |
| `@id` | `@id` / `@id(type, id)` | 字段(单) / 表(组合) | 任一 cell 为空; 或非空组合重复 |
| `@seq` / `@seq(step)` | `@seq(2)` | 字段 | 序列值与 `1, 1+step, 1+2*step, ...` 不一致 |
| `@order` / `@order(asc)` / `@order(desc)` | `@order(desc)` | 字段 | 违反所声明的方向 |

`@unique` 用 SQL 风格: NULL/空 cell 不参与唯一比较,但**仍参与** `@id` 的 NOT NULL 检查。

`@order` 不支持双键稳定排序 (`@order(score, ts)` 这种);单字段 asc/desc 已能覆盖绝大多数手填场景。

### 3.4 跨表引用 (Layer 4) — 项目级

| 名称 | 语法 | 失败 |
|------|------|------|
| `@ref("other.id")` | `@ref("ItemTable.id")` | 当前列的值不在 `ItemTable.id` 中 |
| (字段级) | `@ref("Item.id")` | 字段级语法,host = 字段自身 |
| (表级) | `@ref(host, "Item.id")` | 表级语法,host 由第一个参数指定 |

`@ref` 在 `ConstraintValidator::validate_project` 阶段才执行;空/NULL cell 自动跳过 (SQL 外键可空语义),叠加 `@notnull` 即可强制非空。

实现要点: 解析 `"a.b"` → `(table_name, column_name)`;目标表或列缺失 → `ConstraintCrossTableMissingTable` / `ConstraintForeignKeyViolation`。

---

## 4. 参数解析器升级

### 4.1 现有问题

`Constraint::from_str_with_loc` 用 `split(',')` 分割参数,无引号识别。`@pattern("^[a-z]+$")` 会切成 `["\"^[a-z", "$\""]`,无法被正则引擎消费。

### 4.2 新解析规则

`@func(arg1, arg2, ...)`:

1. 顶层分隔符为 `,`;
2. `,` 在 `"..."` 内是字面量;
3. `"` 进入字符串模式,匹配下一个未转义的 `"`; 字符串内容里的 `"` 直接报 `TableConstraintParseError`("unexpected quote");
4. `\"` 与 `\\` 在字符串内被转义为 `"`、`\`; 其它 `\X` 报 `TableConstraintParseError`("unsupported escape");
5. 字符串外的空白仅在 arg 开始处被 trim;字符串内空白保留;
6. 非字符串模式下不接受 `"` 或 `\` (除上一条起的转义序列);
7. 单引号不识别 (`'...'` 一律按字面字符);
8. 解析结果: 解析器返回 `Vec<String>`,每项已去掉外层引号和转义后的内容。`@oneof("a", "b")` 与 `@oneof(a, b)` 在语义上等价,都是 `["a", "b"]`。

### 4.3 影响范围

- `Constraint::from_str_with_loc` 改造,内部委托给新 `ArgParser`;
- 现有单元测试 (`test_ok`/`test_fail`) 必须全部保留通过;
- 新增引号用法 + 转义 + 错误用例的测试。

---

## 5. 错误码扩展

`DiagnosticCode` 新增:

```
ConstraintValueViolation          // min / max / maxlen 失败
ConstraintNotInSet                // oneof
ConstraintPatternMismatch         // pattern
ConstraintNullNotAllowed          // notnull 与 id 失败
ConstraintCrossTableMissingTable  // ref 目标表名不存在
ConstraintForeignKeyViolation     // ref 失败
ConstraintCrossFieldMissingColumn  // 行内 @eq/@gt 等缺字段
```

`diagnostic_code_count_matches_spec` 测试的数组新增项,`assert_eq!(codes.len(), N)` 同步 +N。

## 6. 跨表约束的特殊流程

`@ref` 在单表 validation 阶段被 `ConstraintValidator::validate_table` 跳过;在 `validate_project(&[Table])` 阶段才执行,确保能拿到所有表。

实现要点:
- `Constraint::is_cross_table()` 返回 true 的约束在 `validate_table` 中跳过;
- `validate_project` 先对每张表跑 `validate_table`,再针对 `is_cross_table()` 跑 `Constraint::validate_cross_table`。
- 解析 `"a.b"` → `(table_name, column_name)`;目标表不存在 → `ConstraintCrossTableMissingTable`;目标列不存在 → `ConstraintForeignKeyViolation` 携带诊断信息。

## 7. 测试与覆盖率

- 每个约束至少 3 个用例: 正向、负向(典型)、类型不匹配。
- 解析器:引号内逗号、转义、错误的引号、单参无括号、空 args。
- 跨表:用临时 `Project` (2-3 张表) 跑 `validate_project`。
- 覆盖率: `cargo llvm-cov --workspace` 行覆盖 ≥ 95%。

---

## 8. 落地为 commit 列表

| Commit | 范围 | 编译/测试门槛 |
|--------|------|----------------|
| c1 | 参数解析引号支持 + 现有测试不变 | parser tests pass |
| c2 | DiagnosticCode 扩展 + 测试计数 | diag tests pass |
| c3 | Layer 1 单格级 9 个约束 + tests | constraint tests + row coverage |
| c4 | Layer 2 行内跨字段 7 个约束 + tests | 同上 |
| c5 | Layer 3 表内扩展 + Layer 4 跨表 + `validate_project` | 同上 + 跨表集成测试 |
| c6 | docs 更新 (doc/ 下约束用法) + 覆盖率验证 | `cargo llvm-cov` ≥ 95% |

c5 同时为 `Project` 引入 `validate_project` 接口,这是第一次出现项目级约束;后续扩展都走这条路径。

---

## 9. 风险与回退

- 引号解析若破坏现有 `@unique(id)`,`@seq(10)` 这类调用,立即可见:既有测试会被点亮。回退到 c1 之前的解析即可。
- `validate_project` 是新增 API, 不会破坏既有 API (`validate_table` 仍可用)。
- `Constraint::kind` 字段以 `#[serde(default)]` 加 `#[non_exhaustive]` enum,向前兼容旧 JSON / xlsx 快照。

---

## 10. 不在本轮 (留作未来 spec)

- Float 路径 (`@seq`/`@order` 覆盖 `f32`/`f64`)
- 带 ε 的近似比较
- 跨 cell 的字符串长度聚合 (例如整列总字符数 ≤ N)
- 约束错误交互修复建议(语义消息中给出"应该是什么")
- 运行时 hook (例如 build 期生成 enum 类型从 `@oneof`)
