# 设计文档
## 简介
tablec 意为 **Table** **C**ompiler, 用于编译 Excel 等表格数据到程序易读的数据格式, 如 json, msgpack

## 名词
| 名词            | 含义  | 用途                         |
| -------------- | ----- | --------------------------- |
| **Table**      | 数据表 | 描述二维数据数据的基本结构。     |
| **Row**        | 行    | 数据表(Table)的横向数据集，包含多个列的数据。 |
| **Col**        | 列    | 数据表(Table)的竖向数据集，所有行的该列数据共享同一字段定义。 |
| **Schema**     | 模式  | 数据表的整体结构，包括字段信息(Field)和约束(Constraint)。 |
| **Field**      | 字段  | 用于声明某一列数据的元信息，包括字段名、类型和注释。 |
| **Constraint** | 约束  | 对一个或多个字段施加限制，如唯一、有序等等，以确保数据完整。 |
| **Tag**        | 标签 | 用于标注列，方便进行分组、筛选和分类处理。 |
| **Type**       | 类型 | 用于声明字段中数据的类型（如int、string、日期等等）。 |
| **Value**      | 值   | 表示数据表中具体的数值或内容。 |


## 设计
 Excel Sheet 即 `Table`, 前 5 行作为保留行声明 Schema（字段名、字段类型、注释、字段级 / 表级 Constraint），第 6 行起为数据行。

| 第1行     | 字段名 |
| -------- | ----- |
| 第2行     | 字段类型 |
| 第3行     | 字段注释 |
| 第4行     | Constraint（字段级，每个 cell 一个 `@func`，作用于所在列） |
| 第5行     | Constraint（表级，每个 cell 一个 `@func`，作用于全表） |
| 第6行起   | 数据行 |



## 字段类型
### 1. 基础类型
`string`, `int`, `uint`, `float`, 按精度细分为：
1. int => int8, int16, int32, int64  
2. uint => uint8, uint16, uint32, uint64  
3. float => float32, float64  
且 string 可简写为 str

### 2. 数组类型
**`type[]`** 或 **`array<type>`**  

表示 type 类型的数组，推荐简写为 `type[]`,比如 int[] 表示 int 类型的数组
其中 type 可以是任意`基础类型`，`数组类型`，或`结构体类型`

例子  

| 类型       | array\<int\>    | int[][]           | string[]          |
| ---------- | ------------- | ----------------- | ----------------- |
| 数值       | 1, 2, 3       | [1,2,3],[1,2,3]   | [hello, world]    |
| 导出(Json) | [1, 2, 3]     | [[1,2,3],[1,2,3]] | ["hello","world"] |


| 类型       | array<struct{a:int, b:str}>           | struct{a:int, b:str}[]                |
| ---------- | ------------------------------------- | ------------------------------------- |
| 数值       | {1,abc}, {2,def}                      | {1,abc}, {2,def}                      |
| 导出(Json) | [{"a":1,"b":"abc"},{"a":2,"b":"def"}] | [{"a":1,"b":"abc"},{"a":2,"b":"def"}] |


### 3. Map类型
**`Map<keyType, type>`**  

表示键值对. 其中 `keyType` 只能是 `int` 或 `string` 类型, 而 `type` 则可以是 `基础类型`, `数组类型`, `结构体类型`.

例子  
| 类型       | map<int, string>  | map<string, int[]>      |  map<string, {a:int, b:str}>                   |
| ---------- | :---------------- | ---------------------- | ---------------------------------------------- |
| 数值       | 1:2, 2:3          | k1:[1,2], k2:[2, 3]     | k1:{1,2},  k2:{2,3}                            |
| 导出(Json) | {"1":"2","2":"3"} | {"k1":[1,2],"k2":[2,3]} | <code class="json-data"> {"k1":{"a":1, "b":"2"}, "k2":{"a":2, "b":"3"}}</code> |

### 4. 结构体类型
**`{name1:type, name2:type ... }`**  

or  

**`struct{name1:type, name2:type ... }`**  


表示结构体，它包含名为 name1, name2 的多个字段，最多支持 32 个。 类型分别由 `type` 声明， 如省略 `type`, 则默认为 `string` 类型

例子  
| 类型       | {a:int, b:str}  | struct{foo:str, bar: int[]} | {hello:str, world: {a:int, b:float}}[]                             |
| ---------- | --------------- | --------------------------- | ------------------------------------------------------------------ |
| 数值       | {1, 2}          | {yes, [2,3]}                | {yes, {1, 1.0} },{no, {2, 2.0}}                                    |
| 导出(Json) | {"a":1,"b":"2"} | {"foo":"yes", "bar": [2,3]} | `[{"hello":"yes", "world":{"a":1, "b":1.0}},{"hello":"no","world":{"a":2, "b":2.0}} ]` |


## Constraint

是对字段类型 / 表关系的额外约束，文法为 **`@func(arg ...)`**. 参数可以是裸标识符或 `"带引号"` 的字符串:引号内允许保留空白/逗号/`\"`/`\\` 转义.

围绕"手填 Excel 容易出错"的目标，目前提供 **9 个具名约束**，分 4 层:

| 层级 | 关注点 | 约束 |
|---|---|---|
| 字段级 (Layer 1) | 字段值域 | `@nullable`, `@range(lo, hi)`, `@oneof(...)`, `@maxlen(n)`, `@pattern("…")` |
| 表级 (Layer 2) | 跨行关系 | `@unique` / `@unique(a, b, …)`, `@seq` / `@seq(step)`, `@order` / `@order(asc)` / `@order(desc)` |
| 项目级 (Layer 3) | 跨表外键 | `@ref("T.c")` / `@ref(host, "T.c")` |

### 默认非空 (schema-level default-not-null)

不在表里声明任何约束，默认要求每个 cell 必须有值。空 cell (空字符串 / `Value::Null` / 缺失) 由 `ConstraintValidator::validate_table` 的 pre-check 直接报 `ConstraintNullNotAllowed`，再跑 inner 校验之前。

要允许空 cell 的字段必须显式声明 `@nullable`，即字段级第五行 cell 写 `@nullable`。该字段上其余 inner 约束 (`@range` / `@oneof` …) 遇到空 cell 时跳过该 row。

### 各约束

#### 字段级

- `@range(lo, hi)` 两个整数参数；cell 必须是整数且 `lo <= value <= hi`，闭区间。`lo > hi` 拒绝。一边界的写法用大极值 (`@range(0, MAX_I64)` / `@range(MIN_I64, 100)`)，不另设 `@min` / `@max`。
- `@oneof(v1, v2, ...)` 1+ 个参数。每个参数按能否 parse 成 `i64` 自动归到数字桶或字符串桶:cell 是 String 就只比对字符串桶,cell 是数值就只比对数字桶。`@oneof(red, green, blue)` ≡ `@oneof("red", "green", "blue")`。
- `@maxlen(n)` 一个整数参数；字符串字符数 (UTF-8 chars) 不超过 n。仅保留上界 (`@minlen` / `@len` 不再设)。
- `@pattern("regex")` 一个参数，必须 `"..."` 引号包裹正则字面量。cell 字符串匹配正则。

#### 表级 (行内跨字段约束已砍)

设计上有过 `@eq(host, other)` / `@gt` / `@lt`，表内数字列之间的一致性 / 数值大小关系。已删——手填场景下这类错很罕见，且 GitHub merge 时通常都会被表上的 sum / check 脚本兜住。

#### 表级 (跨行)

- `@unique` / `@unique(a, b, ...)` 0 或多个字段名。SQL 风格: 任一 cell 在 `@nullable` 覆盖下为空时，该 row 跳过; 否则按整行 key 去重。
- `@seq` / `@seq(step)` 0 / 1 个整数参数。默认起点 1 步长 1; `@seq(step)` 给定 step (可为负)。值必须依次 `1, 1+step, 1+2*step, …`。
- `@order` / `@order(asc)` / `@order(desc)` 字段级。`asc` 时不允许 `prev > current`;`desc` 时不允许 `prev < current`。

#### 项目级 (跨表)

- `@ref("T.c")` 字段级，host = 字段自身。
- `@ref(host, "T.c")` 表级，host 由第一参数指定。

`@ref` 只能在 `ConstraintValidator::validate_project(&[Table])` 这条路径执行，需要所有表一次性传入才能解析 `"a.b"`。目标表或列不存在，或 host 值不在目标列中 —— 这三种情况都报 `ConstraintForeignKeyViolation`，msg 区分。host cell 为空 / `Value::Null` 时该 row 跳过 (SQL 外键可空); 需强制非空时叠加 `@notnull` —— 见下一节.

### `@notnull` 与 `@nullable`

> **重要**: 默认非空 = "不需要写 `@notnull`"。手填 cell 为空，pre-check 直接报错。

旧版本 `@notnull` 已删除。若想让某个字段允许空 (例如备注列、引用列的 FK 软删除占位)，在该字段第 4 行写 `@nullable` 即可。`@nullable` 不接受额外参数 (写了也报错)。

举例 — 备注字段可空:

```
列: comment

| comment                                              |
| @nullable                                            |
| (用户填的备注，能空)                                  |
| (用户填的另一条)                                      |
```

FK 字段也用 `@nullable` 取消"必须引用存在"的非空强制；host 为空时该 row 跳过。

### 参数解析

- 顶层分隔符 `,`。
- `"..."` 内 `,` 保留为字面量。
- `"..."` 内支持 `\"` 与 `\\` 两个转义；其它 `\X` 报错。
- 单引号不识别 (`'...'` 按字面字符处理)。

### 错误码

校验失败抛 `Diagnostic`，每种约束映射到一个 `DiagnosticCode` (见 `Constraint::to_diagnostic`):

| 触发 | DiagnosticCode |
|---|---|
| 默认非空 (空 cell + 无 `@nullable`) | `ConstraintNullNotAllowed` |
| `@range` / `@maxlen` 越界 | `ConstraintValueViolation` |
| `@oneof` 不在枚举 | `ConstraintNotInSet` |
| `@pattern` 不匹配 | `ConstraintPatternMismatch` |
| `@unique` / `@id` 重复 (已删除) | `ConstraintDuplicate` |
| `@seq` 序列不一致 | `ConstraintSequenceBroken` |
| `@order` 违反方向 | `ConstraintOrderViolation` |
| 解析错误 (语法错) | `TableConstraintParseError` |
| 未知约束函数 | `ConstraintUnknown` |

`@ref` 报错统一用 `ConstraintForeignKeyViolation` (msg 区分)。

### 执行入口

- `ConstraintValidator::validate_table(&Table)` 先扫 pre-check (默认非空)，再跑该表所有字段级 + 表级约束 (除 `@ref`)。
- `ConstraintValidator::validate_project(&[Table])` 先对每张表跑 `validate_table`，再统一解析所有 `@ref` 跨表查值。
- `tablec check` 走 `validate_project` 这条路径 (需要所有表)；其它 build / 单表场景仍可用 `validate_table`。


## 代码工程
1. tablec 核心模块由 rust 实现，对外提供不同语言的 binding。
2. binding 用于实现其他语言 API. 
    * bingding-python 基于 pyo3 和 maturin 实现, 其使用 python 的管理工具 uv

## 数据结构
1. project
    ```rust
    #[derive(Debug, Serialize)]
    pub struct Project {
        pub name: String,
        pub meta: Meta,
        pub tables: Map<name, Table>,
    }

    #[derive(Debug, Serialize)]
    pub struct Meta {
        pub version: String,
        pub hash: i64,
        pub build_at: i64,
    }
    ```
2. table
    ```rust
    #[derive(Debug, Serialize)]
    pub struct Table {
        pub name: String,
        pub fields: Vec<field::Field>,
        pub data: Vec<Row>,
        pub constraints: Vec<constraint::Constraint>,
    }
    ```

## 插件机制

tablec 通过 `SchemaParser` trait 暴露插件接口。用户实现该 trait 即可接管"sheet 单元格 → Schema"路径。

- 默认 plugin `StandardSchemaParser` 保留 5 行布局：字段名 / 字段类型 / 字段注释 / 字段约束 / 表约束，第 6 行起数据。
- 自定义 plugin 实现 `SchemaParser`：
  - 静态：`SchemaParserRegistry::register(MyParser)`（同进程）
  - 动态：编译为 `[lib] crate-type = ["cdylib"]`，导出 `tablec_plugin_create_v1` / `tablec_plugin_drop_v1`，tablec 运行时通过 `libloading` 加载
- 入口选 parser：
  - 静态 config：`tablec.toml [parser] name = "..."`
  - 动态 config：`tablec.toml [[plugins]] path = "./parser.so"`
  - CLI：`tablec build --parser NAME --plugin-path ./parser.so`
  - Python：`tablec.build(input, output, parser="...")`
- 注意：动态 plugin 与 host 必须用相同 Rust 工具链编译（Rust ABI 不稳定）

详细 API 见 `tablec-core/src/core/schema.rs`。

## WebUI 差异预览（git 基线）

webui（`tablec webui`）提供基于 git 的变更预览：以**当前分支 HEAD** 为基线，对比工作区表格内容。

- 左侧文件列表：每个表格显示 `modified` / `added` / `untracked` / `deleted` / `clean` 状态（来自 `git status --porcelain`），`modified` 额外显示增删行数（`git diff --numstat`）；支持 **All files / Modified only** 过滤。
- 解析预览：每个数据单元格标注 diff 状态 —— 新增 `added`（绿）、删除 `deleted`（红）、修改 `modified`（黄）。实现方式：把 HEAD blob（`git show HEAD:<path>`）写入临时文件，走与工作区**同一套 calamine 解析**，再按唯一键（无则按行号）对齐两版、逐格比较解析后的值（数值跨宽度相等视为未变）。
- 非 git 仓库 / 无 HEAD / `git` 缺失：降级为 clean + 无色彩，不报错。
- 信任边界：webui 从不接受 HTTP 传入的 plugin 路径；git 基线同样只读，不会改动仓库。

详见 `openspec/specs/git-diff/spec.md` 与 `openspec/specs/webui/spec.md`。

## WebUI 实时刷新（文件监听）

webui 监听输入目录的文件变化，并通过 WebSocket 实时推送给前端，文件列表自动刷新，无需手动 Reload。

- 监听：`notify` crate（Linux inotify / Windows ReadDirectoryChangesW / macOS FSEvents 或 kqueue），**非递归**监听解析出的输入目录（与 `/api/files` 列表一致）。
- 事件只当"脏标记"：任何 create/modify/remove/rename 都触发一次目录重扫；400ms 静默窗口把编辑器"临时文件+rename"式保存的事件风暴合并为一次刷新。最终状态永远来自重扫，不依赖事件完整性（容忍 inotify 丢事件等平台差异）。
- 推送：`/ws` WebSocket 端点，服务器通过 tokio broadcast 向所有连接广播 `files_changed`；客户端收到后调用 `refreshState()` 重新拉取文件列表（含当前过滤条件）。
- 前端：启动即连 `/ws`；断线按指数退避重连（1s → 10s 封顶），重连成功后无条件重新拉取（覆盖断线期间的变化）；Reload 按钮（⌘R）始终保留为手动兜底。**无任何轮询定时器**。
- 降级：输入目录不存在 / 监听失败（权限、inotify watch 上限）→ 记录日志，webui 照常工作（手动 Reload 可用），不崩溃。
- 明确不做：文件变化**不会**触发 build/check（本特性只刷新预览列表）。

详见 `openspec/specs/webui/spec.md`（Live file-change notifications 与 WebSocket endpoint and client lifecycle）。
