# tablec Schema 抽象 + SchemaParser 插件机制 — 设计稿

**日期**: 2026-07-30
**仓库**: `repos/tablec`
**触发**: 用户希望从"自定义头布局"的数据表里解析出 tablec 规范的数据结构。

**范围**:
- 新增 `tablec-core/src/core/schema.rs`：`Schema` 数据结构 + `SchemaParser` trait + `SchemaParseResult` 枚举 + `SchemaParserRegistry` + 默认 `StandardSchemaParser`
- `Table` 重构为 `{ name, schema: Schema, data }`，迁移所有构造/读取点
- `read_excel` 拆为 `read_excel(fpath)`（保留 wrapper，内部用 `StandardSchemaParser`）和 `read_excel_with(fpath, parser)`（新主路径）
- CLI / `tablec.toml` / Python binding 各新增一个 parser 选择入口
- 新增 `DiagnosticCode::HeaderParserError / SchemaFieldOverlap / SchemaDataStartOOB`
- 示例 plugin `EightRowHeaderParser`（仅 test/doc 编译）

**不做**:
- 运行时按 sheet 名自动 dispatch 到不同 parser（一个 build 一个 parser）
- Python 侧让用户写 plugin（PyO3 trait 暴露留作下个 spec）
- WASM 插件
- `Schema` 的序列化 roundtrip 独立测试（serde derive 自动加）
- 改 `FieldType` / `Constraint` 内部表示

---

## 1. 背景与目标

### 1.1 现状

`tablec-core/src/core/table/table.rs:20-198` 的 `read_excel` 把 sheet 头布局硬编码成 5 行：
- row 0: 字段名
- row 1: 字段类型
- row 2: 字段注释
- row 3: 字段约束
- row 4: 表约束
- row 5+: 数据

字段名还内嵌 `[tag1,tag2]` 语法切 tag；类型走 `FieldType::from_str`；约束走 `Constraint::from_str`。这是 5 行布局 + sheet 起始 `#` 跳过的单一约定。

### 1.2 痛点

用户的数据表头布局不一定符合 5 行约定：
- 头可能是 3 行 / 8 行 / 任意行
- 头里可能夹带无关行（注释、装饰、英文表名）
- 数据起始行不一定是 row 6
- 不同 sheet 命名约定不同（`Config_*` / `Item_*` 走不同布局）

目前要支持以上任何一种都得改 `read_excel` 主体，扩展性差。

### 1.3 设计目标

让用户写 Rust 代码接管"sheet 单元格 → Schema"的转换路径；下游（数据行解析、Constraint 校验、JSON/MessagePack 导出）继续走 tablec 自己的标准实现。

---

## 2. Schema 抽象

### 2.1 `Schema` 结构体

新文件 `tablec-core/src/core/schema.rs`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    pub fields: Vec<Field>,
    pub constraints: Vec<Constraint>,
    pub data_start_row: usize,
}

impl Schema {
    /// 兼容旧调用方：fields/constraints 直接给，自动设 data_start_row = 5
    pub fn from_parts(
        fields: Vec<Field>,
        constraints: Vec<Constraint>,
    ) -> Schema {
        Schema { fields, constraints, data_start_row: 5 }
    }
}
```

- `fields` / `constraints` 从 `Table` 平移过来。
- `data_start_row` 是 sheet 中数据行起始的绝对行号（0-based）。
- `from_parts` 是迁移期便利构造器，最终代码里都直接用结构体字面量。

### 2.2 `Table` 重构

```rust
// tablec-core/src/core/table/table.rs
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub data: Vec<Row>,
}
```

迁移点（一次性扫描仓库全替换）：
- `Table { name, fields, data, constraints }` → `Table { name, schema: Schema::from_parts(fields, constraints), data }`
- `table.fields` → `table.schema.fields`
- `table.constraints` → `table.schema.constraints`
- `ConstraintValidator::validate_table(&Table)` 内部读 `table.schema.fields` / `table.schema.constraints`

`from_parts` 临时存在，直到所有构造点被清理干净；后续 spec 里可以删除。

---

## 3. SchemaParser trait

### 3.1 trait 定义

```rust
pub trait SchemaParser: Send + Sync {
    fn name(&self) -> &str;

    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>>;
}

pub enum SchemaParseResult {
    Schema(Schema),
    Skip,
}
```

参数语义：
- `sheet_name`: 让 plugin 按 sheet 名分支（`Config_*` / `Item_*` 走不同布局，或"x 开头跳过"）
- `sheet`: 完整 sheet 单元格，物化好的 `Vec<Vec<String>>`（行 × 列）
- 错误用 `Vec<Diagnostic>` 复用现有诊断系统

### 3.2 `StandardSchemaParser`（默认实现）

```rust
pub struct StandardSchemaParser;

impl SchemaParser for StandardSchemaParser {
    fn name(&self) -> &str { "standard" }

    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        // 复用原本 read_excel 里 row 0..5 的提取逻辑
        // 首列以 # 开头 → 返回 SchemaParseResult::Skip
        // 首行缺失 → 返回 HeaderParserError
        // data_start_row 固定 5
    }
}
```

行为必须**字节级**保留现有 `read_excel`：
- 字段名 `[tag1,tag2]` 切 tag
- 类型 parse 失败 fallback 到 `FieldType::String`
- 缺列补 `""`
- `#` 开头 skip sheet

### 3.3 `SchemaParserRegistry`

```rust
pub struct SchemaParserRegistry {
    parsers: HashMap<String, Arc<dyn SchemaParser>>,
}

impl SchemaParserRegistry {
    pub fn with_standard() -> Self {
        let mut r = Self { parsers: HashMap::new() };
        r.register(StandardSchemaParser);  // 自动取 parser.name()
        r
    }

    pub fn register<P: SchemaParser + 'static>(&mut self, parser: P) {
        let name = parser.name().to_string();
        if self.parsers.contains_key(&name) {
            panic!("parser '{}' already registered", name);
        }
        self.parsers.insert(name, Arc::new(parser));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn SchemaParser>>;

    pub fn parser_names(&self) -> Vec<String>;
}
```

- `with_standard()` 默认装好 `standard → StandardSchemaParser`
- 注册同名 → `panic!`（开发期错误，配置错也会立刻崩）
- `Send + Sync` 是必须的：`read_excel` / CLI 异步路径都跨线程

---

## 4. `read_excel` 重构

### 4.1 新签名

```rust
pub fn read_excel(fpath: &str) -> Result<Vec<Table>, Vec<Diagnostic>> {
    read_excel_with(fpath, &StandardSchemaParser)
}

pub fn read_excel_with(
    fpath: &str,
    parser: &dyn SchemaParser,
) -> Result<Vec<Table>, Vec<Diagnostic>> {
    let mut workbook = open_workbook_auto(fpath).map_err(...)?;
    let mut tables = vec![];
    let mut diagnostics = vec![];

    for sheet_name in workbook.sheet_names().to_owned() {
        let range = match workbook.worksheet_range(&sheet_name) {
            Ok(r) => r,
            Err(e) => { eprintln!(...); continue; }
        };
        let cells: Vec<Vec<String>> = range.rows()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect();

        let parse_result = match parser.parse_schema(&sheet_name, &cells) {
            Ok(r) => r,
            Err(d) => { diagnostics.extend(d); continue; }
        };

        let schema = match parse_result {
            SchemaParseResult::Skip => continue,
            SchemaParseResult::Schema(s) => s,
        };

        // 校验 schema.fields 互不重名
        if let Some(d) = check_field_overlap(&schema.fields, ...) {
            diagnostics.push(d);
            continue;
        }

        // 校验 data_start_row 在 sheet 范围内
        if schema.data_start_row > cells.len() {
            diagnostics.push(Diagnostic::new(
                SchemaDataStartOOB,
                format!("data_start_row={} > sheet rows={}", schema.data_start_row, cells.len()),
                ...
            ));
            continue;
        }

        // 数据行解析
        let (rows, mut diags) = parse_data_rows(
            cells.iter().skip(schema.data_start_row).enumerate(),
            &schema.fields,
            fpath,
            &sheet_name,
            schema.data_start_row,
        );
        diagnostics.append(&mut diags);
        tables.push(Table { name: sheet_name, schema, data: rows });
    }

    if diagnostics.is_empty() { Ok(tables) } else { Err(diagnostics) }
}
```

### 4.2 `parse_data_rows` 抽出

```rust
fn parse_data_rows<'a, I: Iterator<Item = (usize, &'a Vec<String>)>>(
    rows: I,
    fields: &[Field],
    fpath: &str,
    sheet_name: &str,
    data_start_row: usize,  // 绝对行号；用于计算 1-based 行号
) -> (Vec<Row>, Vec<Diagnostic>) {
    // 跳过空行
    // 用 field.t 调用 parse_value
    // 行号 = data_start_row + enumerate_index + 1（1-based，反映真实行号）
    //       旧代码用 row_index + 6 是因为前 5 行是 header；
    //       新代码用 实际行号 + 1，与 data_start_row 自然衔接
}
```

行号变化：旧 `row_index + 6` (0-based 增量 + 6) → 新 `(actual_row_index) + 1` (1-based 真实行号)。
两者**数值上**对应：旧代码 row 0 → line 6 = 第 6 行；新代码 row 5 → line 6 = 第 6 行。一致。

### 4.3 包装层（保留外部 API）

`read_excel(fpath)` 是 wrapper，内部用 `StandardSchemaParser`。所有现有调用方（CLI / Python / 测试）零改动。

---

## 5. 入口选 parser

### 5.1 CLI

`tablec build` 加 `--parser <name>`：

```
tablec build -i data.xlsx --parser my-parser
```

默认 `"standard"`。`tablec check` 同样支持。

### 5.2 `tablec.toml`

```toml
[parser]
name = "my-parser"
```

CLI 显式 `--parser` 优先于 config；config 缺省时走 `standard`。

### 5.3 Python binding

```python
tablec.build(input, output, parser="my-parser")
tablec.check(input, parser="my-parser")
```

默认 `"standard"`。

### 5.4 注册自定义 parser

本期不开放给 Python 用户（PyO3 trait 暴露复杂）。Rust 用户：
- 写 `impl SchemaParser for MyParser`
- 在自己的二进制里 `let mut reg = SchemaParserRegistry::with_standard(); reg.register(MyParser);`
- 通过 `--parser my-parser` 走

未来 spec 考虑：(a) plugin 动态库 (cdylib + abi_stable) 加载 (b) PyO3 暴露 trait 给 Python。

### 5.5 动态加载插件（cdylib + libloading）

发布出去的 tablec 是编译好的二进制，用户拿不到源码。要让外部 plugin 注入进来，必须支持运行时动态加载。

**Plugin 形态**：Rust crate，`[lib] crate-type = ["cdylib"]`，编译成 `.so` / `.dll` / `.dylib`。

**C ABI v1（约定 host / plugin 用同 Rust 工具链）：**

```rust
// plugin/src/lib.rs
use tablec_core::schema::{Schema, SchemaParser, SchemaParseResult};
use tablec_core::diagnostic::Diagnostic;

pub struct MyParser;

impl SchemaParser for MyParser {
    fn name(&self) -> &str { "my-parser" }
    fn parse_schema(...) -> Result<SchemaParseResult, Vec<Diagnostic>> { ... }
}

#[no_mangle]
pub unsafe extern "C" fn tablec_plugin_create_v1() -> *mut dyn SchemaParser {
    Box::into_raw(Box::new(MyParser))
}

#[no_mangle]
pub unsafe extern "C" fn tablec_plugin_drop_v1(p: *mut dyn SchemaParser) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)); }
    }
}
```

**Host 加载（tablec-core/src/core/schema/dynamic.rs）：**

```rust
use libloading::Library;
use std::ptr::NonNull;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct DynamicPlugin {
    _lib: Library,
    parser: NonNull<dyn SchemaParser>,
    drop_fn: unsafe extern "C" fn(*mut dyn SchemaParser),
}

impl DynamicPlugin {
    /// 加载 cdylib 并返回 plugin
    /// host / plugin 必须用相同 Rust 工具链编译（Rust ABI 不稳定）
    pub unsafe fn load(path: &Path) -> Result<Arc<Self>, DynamicPluginError> {
        let lib = Library::new(path).map_err(DynamicPluginError::Load)?;
        let create: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn SchemaParser> =
            lib.get(b"tablec_plugin_create_v1").map_err(DynamicPluginError::Symbol)?;
        let drop_fn: libloading::Symbol<unsafe extern "C" fn(*mut dyn SchemaParser)> =
            lib.get(b"tablec_plugin_drop_v1").map_err(DynamicPluginError::Symbol)?;
        let raw = create();
        let parser = NonNull::new(raw).ok_or(DynamicPluginError::NullPointer)?;
        Ok(Arc::new(Self {
            _lib: lib,
            parser,
            drop_fn: *drop_fn,
        }))
    }
}

impl SchemaParser for DynamicPlugin {
    fn name(&self) -> &str {
        unsafe { (*self.parser.as_ptr()).name() }
    }
    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        // 关键：捕获 plugin panic，防止 tablec 整体崩溃
        catch_unwind(AssertUnwindSafe(|| unsafe {
            (*self.parser.as_ptr()).parse_schema(sheet_name, sheet)
        })).unwrap_or_else(|_| {
            Err(vec![Diagnostic::new(
                DiagnosticCode::HeaderParserError,
                format!("plugin panicked while parsing sheet '{}'", sheet_name),
                SourceLocation::default(),
            )])
        })
    }
}

impl Drop for DynamicPlugin {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.parser.as_ptr()) }
    }
}
```

**注册表扩展：**

```rust
impl SchemaParserRegistry {
    /// 在 with_standard 基础上加载 CLI / config 指定的插件
    pub fn with_standard_and_plugins(plugin_paths: &[PathBuf]) -> Result<Self, DynamicPluginError> {
        let mut reg = Self::with_standard();
        for path in plugin_paths {
            let plugin = unsafe { DynamicPlugin::load(path) }?;
            reg.register_arc(plugin);
        }
        Ok(reg)
    }
}
```

**配置 + CLI 入口：**

```toml
# tablec.toml
[[plugins]]
path = "./target/release/libmy_parser.so"
```

```bash
tablec build --plugin-path ./libmy_parser.so
tablec build --plugin-path ./lib_a.so --plugin-path ./lib_b.so
```

CLI 累加到 config；同名 plugin 通过 `name()` 重复时 panic（与静态注册一致）。

**ABI 版本约定：** 符号以 `_v1` 结尾。后续 ABI 不兼容时引入 `_v2`，host 优先尝试 v1，回退 v2。`abi_stable` 是 v2 升级路径（本期不上）。

**线程安全 & panic 安全：**
- `SchemaParser: Send + Sync` 已强制；`DynamicPlugin` 内部 `NonNull<dyn SchemaParser>` 在 `Send + Sync` 边界上对调用方透明
- `parse_schema` 包 `catch_unwind`，plugin 崩溃降级为 `Diagnostic::HeaderParserError` 并跳过该 sheet，整个 host 不崩
- plugin 内部多用 `Box::leak` / 内部 `Mutex` 都行，但 panic 一律走 host 兜底

**为什么不直接用 `abi_stable`：** 本期目标是"用户能写 plugin 引入"，而不是"任意 Rust 工具链组合都能跑"。同 Rust 工具链是合理约束（用户已经在写 Rust）。`abi_stable` v0 → v1 升级路径复杂（需要 `#[sabi_trait]` 重写所有 trait、所有结构体换成 `RString` / `RVec`），属于另一个 spec 的工作。

---

## 6. DiagnosticCode 扩展

```rust
// tablec-core/src/core/diagnostic.rs
pub enum DiagnosticCode {
    ...existing variants...,
    HeaderParserError,        // 插件拒绝 schema
    SchemaFieldOverlap,       // fields 含重名
    SchemaDataStartOOB,       // data_start_row 越界
}
```

区分"plugin 写错" vs "sheet 数据本身坏"。`SchemaFieldOverlap` 是 must-fix（plugin 不能输出同名字段）。

---

## 7. 测试

按项目规则（`tablec-core` 测试覆盖率 ≥ 95%），每个新模块单元测试全覆盖。

| 模块 | 测试用例 |
|------|---------|
| `Schema` | `from_parts` 默认 `data_start_row = 5`；serde clone 等价 |
| `SchemaParser` trait mock | `name() / parse_schema()` 满足 trait object 调用 |
| `StandardSchemaParser` | 5 行布局正常路径；首列 `#` 跳过；空 sheet；缺列；字段名 `[tag]` 切分；类型 parse 失败 fallback；与旧 `read_excel` 字节级一致 |
| `SchemaParserRegistry` | `with_standard` 含 `standard`；注册同名 panic；`get` 命中 / 不命中；`parser_names` 顺序 |
| `EightRowHeaderParser`（示例） | 8 行布局正常；行数不足报错；字段约束解析失败 |
| `DynamicPlugin` | 加载有效 `.so` 成功；`_lib` 持有期间 `name()` 正确返回；`parse_schema` 透传到 plugin；plugin panic → `HeaderParserError`；`Drop` 调用 plugin 的 `drop_fn`；`.so` 文件不存在 / 符号缺失 → `DynamicPluginError` |
| `SchemaParserRegistry::with_standard_and_plugins` | 0 / 1 / 多个 plugin 路径；plugin name 冲突 panic |
| `read_excel_with` | 现有 `examples/testdata` 全跑通；返回 `Table` 与旧 `read_excel` 字节级一致 |
| `parse_data_rows` | 空行跳过；缺 cell 报错；类型 parse 失败报错；行号正确 |
| `SchemaFieldOverlap` | 触发后 `Err`；`DiagnosticCode` 正确 |
| `SchemaDataStartOOB` | 触发后 `Err`；行号 message 包含 `data_start_row` 和 `sheet rows` |
| 端到端 cdylib | 在 `tests/fixtures/cdylib_parser/` 写一个真插件（`crate-type = ["cdylib"]`），`cargo build` 出 `.so`，`tablec build` 加载并跑通 fixture xlsx 文件 |

所有测试必须在 `cargo test` 中跑过；覆盖率 `cargo tarpaulin` 报告 ≥ 95%（参考 `.claude/rules/how-to.md`）。

---

## 8. 示例 plugin

```rust
// tablec-core/src/core/schema/example.rs
// 仅 doc / test 编译
#[cfg(any(test, doc))]
pub struct EightRowHeaderParser;

#[cfg(any(test, doc))]
impl SchemaParser for EightRowHeaderParser {
    fn name(&self) -> &str { "eight-row" }

    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        // 期望布局:
        // row 0,1: 跳过
        // row 2: 装饰（中文表名之类，跳过）
        // row 3: 字段名
        // row 4: 字段类型
        // row 5: 字段注释
        // row 6: 字段约束
        // row 7: 表约束
        // row 8+: 数据

        let fields = assemble_fields(&sheet[3], &sheet[4], &sheet[5], &sheet[6])?;
        let table_constraints = parse_table_constraints(&sheet[7])?;
        Ok(SchemaParseResult::Schema(Schema {
            fields,
            constraints: table_constraints,
            data_start_row: 8,
        }))
    }
}
```

`cargo doc` 打开能看到用法；`cargo test` 覆盖；`cargo build --release` 不打包。

注：本示例用到的 `assemble_fields` / `parse_table_constraints` 是该示例私有的辅助函数（接 row 字符串数组构造成 `Field` 列表 / 解析表级约束），其实现细节不属于本 spec 范围——它应当是 `StandardSchemaParser` 内部同款逻辑的提取，针对 8 行布局复用。落地时只需借用标准 parser 的 helper，不再重复设计。

---

## 9. 文档更新

`doc/design.md` 末尾新增一节"插件机制"（≤ 200 字）：
- schema/parser 概念
- 标准 plugin = `StandardSchemaParser`
- 自定义 plugin 通过 `SchemaParserRegistry` 注册
- 入口：CLI `--parser` / `[parser] name` / Python `parser=`

不动 `doc/design.md` 的现有章节（字段类型、Constraint）。`README.md` 新增一段"自定义头布局"指向 `doc/design.md` 插件机制节。

---

## 10. 决策摘要

| 决策 | 选择 | 否决方案 |
|------|------|---------|
| Schema 形态 | `Schema { fields, constraints, data_start_row }` | 拆 `Header` + `Body` 两 struct |
| `Table` 字段 | `Table { name, schema, data }` | `Table { name, fields, constraints, data }` |
| plugin 入口 | 单 trait `SchemaParser` | multi-trait / builder DSL |
| 默认 plugin | `StandardSchemaParser` 字节级保留旧行为 | 让用户自己写 standard |
| 注册时机 | 创建 registry 时 `register` | 启动时全局 once_cell |
| 同名注册 | panic | 返回 Err 并静默 |
| 选 parser 入口 | CLI / config / Python 三处 | 单 CLI flag |
| 一个 build 一个 parser | 是 | 多 parser 自动 dispatch |
| 示例 plugin | `#[cfg(any(test, doc))]` | 总是打包 |
| 错误码 | 加 3 个 `DiagnosticCode` | 复用 generic Other |
| 动态加载 ABI | 同 Rust 工具链 + `extern "C"` + 符号 `_v1` | `abi_stable`（复杂，v2 升级路径） |
| plugin 加载 API | `libloading::Library` + `NonNull<dyn SchemaParser>` | 子进程 / WASM |
| plugin panic 兜底 | `catch_unwind` 转 `HeaderParserError` | 直接 unwind 整个 host |
| 插件配置位置 | `tablec.toml [[plugins]]` + `--plugin-path` CLI | 自动发现 `~/.config/tablec/plugins/` |

---

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| `Table` 字段重构破坏导出层 | JSON/MessagePack 行为变化 | 全仓库 grep 一次性更新；老 `read_excel` wrapper 保留；现有 fixture 字节级对拍 |
| `Schema::from_parts` 长期残留 | API 噪音 | 列出 `todo: remove after migration` 注释；下个 spec 收 |
| plugin 数据行解析逻辑有差异 | 输出 `Table` 与 standard 不一致 | `StandardSchemaParser` 测试 byte-level 对拍；`parse_data_rows` 抽出独立测试 |
| 动态加载 cdylib 跨 Rust 工具链版本 | host 与 plugin 编译时 Rust 不同 → ABI 不兼容 | 文档明确要求同工具链；plugin 报错时 `DynamicPluginError::Symbol` 提示 "use same Rust version as tablec" |
| plugin 内部 panic | host 进程崩溃 | `parse_schema` 用 `catch_unwind` 兜底，单 sheet 失败降级为 `HeaderParserError` |
| plugin 内存泄漏（创建后忘记 drop） | 进程退出前泄漏 | `Drop` 实现强制调用 plugin `drop_fn`；`Arc<DynamicPlugin>` RAII 兜底 |
| plugin name 冲突 | 后续注册覆盖 | 同名注册 panic（开发期错误，不静默） |
| plugin 路径不可信 | 二进制加载任意 .so → 任意代码执行 | 文档警告"plugin 用同源代码 / 受信源"；不给外部输入做插件路径（仅 config / CLI） |
| Python 写 plugin | PyO3 trait 暴露复杂 | 本 spec 不做；未来 spec |
| 行号语义变化 | 错误消息行号错位 | 1-based 真实行号；测试 `SourceLocation.line` 数值 |
| 多 parser 按 sheet dispatch | 当前 spec 不支持 | 用户写 wrapper plugin；下个 spec 评估需要 |

---

## 12. 不在本 spec 范围

- 多 parser 自动 dispatch（按 sheet 名 match）
- Python plugin
- WASM plugin
- `Schema` 独立 serde roundtrip 测试
- `Schema` 增量修改（编辑器场景）
- plugin 配置中心化（hot reload）
- `abi_stable` 升级（v2 路径）
- plugin 版本协商（host/plugin tablec-core 版本检查）

---

## 13. 落地节奏

按"先 core 重构 + 默认 parser + 测试覆盖，再 CLI / config / Python 入口"的顺序：

```
0. beads: bd create tablec-schema-1 ~ tablec-schema-6
1. tablec-core/src/core/schema.rs: Schema 结构 + Default derivation + from_parts
2. tablec-core/src/core/table/table.rs: Table 重构 + 从 read_excel 抽出 parse_data_rows
3. tablec-core/src/core/schema.rs: SchemaParser trait + SchemaParseResult
4. tablec-core/src/core/schema.rs: StandardSchemaParser (迁移原 read_excel 逻辑)
5. tablec-core/src/core/schema.rs: SchemaParserRegistry
6. tablec-core/src/core/diagnostic.rs: 新增 3 个 DiagnosticCode
7. tablec-core: Schema 单元测试 + StandardSchemaParser 现有 fixture 字节级对拍
8. tablec-core/src/core/schema/example.rs: EightRowHeaderParser (cfg test/doc)
9. tablec-cli: --parser flag + [parser] name config 支持
10. binding-python: build() / check() 加 parser 参数
11. doc/design.md: 新增"插件机制"节
12. README.md: 新增"自定义头布局"小节
13. **动态加载 cdylib**：
    - `tablec-core/Cargo.toml` 加 `libloading` 依赖
    - `tablec-core/src/core/schema/dynamic.rs` 实现 `DynamicPlugin` + `DynamicPluginError`
    - `SchemaParserRegistry::with_standard_and_plugins(paths)` 加载
    - `tablec-cli` 加 `--plugin-path` flag（可重复）
    - `tablec-core/src/core/config.rs` 解析 `[[plugins]]` 表
    - `tests/fixtures/cdylib_parser/` 写端到端测试 fixture
14. 移除 `Schema::from_parts`(所有构造点应改为结构体字面量)；下个 spec 收
15. 各阶段提交前 bd close 对应 issue; git push 走 feat/xxxx 分支
```

---

## 14. 附录

### 14.1 与现有 spec / 计划的关系

- `2026-07-25-tablec-cli-simplification-design.md`: 现有 CLI 入口；本 spec 扩展 build/check 加 `--parser`
- `2026-07-26-build-dir-design.md`: 目录 build 路径；本 spec 在 read_excel 内部替换，与 build dir 兼容
- `2026-07-28-publish-python-and-binding-python-design.md`: Python binding 打包；本 spec 扩展 Python binding API

### 14.2 验证清单（交付前）

- [ ] `cargo test` 全绿
- [ ] `cargo tarpaulin --package tablec-core` 覆盖率 ≥ 95%
- [ ] `cargo fmt --all --check` 无错
- [ ] `cargo doc` 能打开并展示 `EightRowHeaderParser` 示例
- [ ] 现有 `examples/testdata/*.xlsx` 跑 `read_excel_with(fpath, &StandardSchemaParser)` 与旧 `read_excel(fpath)` 输出 JSON 字节级一致
- [ ] `bd list` 看到 `tablec-schema-1` ~ `tablec-schema-6` 全部 closed
- [ ] `tablec build -i data.xlsx --parser eight-row` 走通示例 plugin
- [ ] `tablec build -i data.xlsx`（默认）继续走 `StandardSchemaParser`，行为不变

### 14.3 references

- skill: `superpowers:brainstorming` — 本 spec 由该 skill 流程产出
- skill: `superpowers:writing-plans` — 由该 skill 转 implementation plan
- 现有: `tablec-core/src/core/table/table.rs:20-198` `read_excel`（被拆分）
- 现有: `tablec-core/src/core/parser/field.rs:87-184` `parse_field_type`（被 `StandardSchemaParser` 复用）
- 现有: `tablec-core/src/core/constraint.rs` `Constraint::from_str`（被 `StandardSchemaParser` 复用）
- 项目规则: `.claude/rules/how-to.md`（tablec-core 覆盖率 ≥ 95%；beads 跟踪）
