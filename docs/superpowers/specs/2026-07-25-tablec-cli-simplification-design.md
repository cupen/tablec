# tablec CLI 简化与文档同步 — 设计稿

**日期**: 2026-07-25
**仓库**: `repos/tablec`
**前置**: PR #1 (`feat/tablec-core-cleanup`) + PR #2 (`feat/core-design-review`) 已合入 main (`91c9e5a`)
**范围**: `tablec-cli` 减负 + `binding-python` 与新 CLI 默认对齐 + 项目文档同步
**不做**: `binding-python` Value enum 升级（c3.1 之后更大的活）、`protobuf` 输出模块（CLAUDE.md 提到但从未实现）、`Table::validate_constraints` 去包一层

---

## 1. 背景与目标

`2026-07-13` 的 design review 把 tablec-core 的设计与测试债清完，但留下了三类尾巴：

1. `tablec web` 命令携带 `actix-web` + `tokio` 两个大依赖，只为 "Hello world!"；与"游戏数据表编译器"的项目定位无关
2. `binding-python::build` 硬编码 `pretty: true`，与 CLI 在 `7b57636` 后把 json 默认改为 minified 不一致；且 Python 端不支持 `json-pretty`
3. `CLAUDE.md` / `README.md` 仍引用已不存在的 `proto/`、错误的 `pybinding/` 路径、把 `web` 列为 "Key Component"

本 spec 三项一起收尾，落地为 3 个 commit（外加 spec doc 自身）。

---

## 2. 改动一：移除 `web` 命令及其依赖

### 2.1 证据

- `tablec-cli/src/cmd/web.rs` (35 行)：`actix_web::HttpServer` 提供 `/`（"Hello world!"）和 `/health`（"OK"）两个 endpoint
- `tablec-cli/src/cli.rs:5,25`：`pub use ... WebCommand;` 和 `Web(WebCommand)` 子命令
- `tablec-cli/src/main.rs`：包含 `Command::Web(c) => { c.run().await?; }` 分支
- `tablec-cli/Cargo.toml` 引入 `actix-web = "4.0"` 与 `tokio = { version = "1", features = ["full"] }`
- 全仓 `grep -r "WebCommand\|cmd::web" --include="*.rs"` 命中点只在这 3 个文件 + `cli.rs`

### 2.2 删除清单

| 文件 | 改动 |
|------|------|
| `tablec-cli/src/cmd/web.rs` | **删除**整文件 |
| `tablec-cli/src/cmd/mod.rs` | 删除 `pub mod web;`（现状包含该行） |
| `tablec-cli/src/cli.rs` | 删除 `pub use crate::cmd::web::WebCommand;` 与 `Web(WebCommand),` 变体及其文档注释 |
| `tablec-cli/src/main.rs` | 删除 `Command::Web(c) => { c.run().await?; }` 分支 |
| `tablec-cli/Cargo.toml` | 删除 `actix-web = "4.0"` 与 `tokio = { ..., features = ["full"] }` |

### 2.3 验证

```bash
grep -rE 'web|WebCommand|actix|tokio' /home/bot/workbench/repos/tablec/tablec-cli/src /home/bot/workbench/repos/tablec/tablec-cli/Cargo.toml
# 期望：0 命中（除本 spec 引用与 cli 自身元数据）
cargo build -p tablec-cli   # 期望：编译通过，输出更小
cargo test -p tablec-cli --lib   # 期望：现存测试仍过
cargo run -p tablec-cli -- --help   # 期望：只显示 build / check / example
```

### 2.4 影响

- 编译产物大小减小：`actix-web` 树（含 tokio 全特性）通常占 release binary 2-5 MB
- 启动时间改善（不再静态拉入 actix runtime）
- 用户面：移除一个长期承诺的"占位"特性

---

## 3. 改动二：binding-python 与新 CLI 默认对齐

### 3.1 证据

`binding-python/src/lib.rs:32` 写死 `Json { pretty: true, include_fields: false }`；CLI 在 commit `7b57636`（已合入 main）之后把 `json` 改为 minified (`pretty: false`)。同时 CLI 暴露 `json-pretty` 作为新格式名。

binding-python 的 format string 仅支持 `"json" | "msgpack"`（`binding-python/src/lib.rs:30,36`），缺失 `json-pretty`。

### 3.2 改动

`binding-python/src/lib.rs` 内 `build()` 函数：

- `format == "json"` → `Json { pretty: false, include_fields: false }`（与 CLI 默认对齐）
- `format == "json-pretty"` → `Json { pretty: true, include_fields: false }`（新增）
- `format == "msgpack"` → `Msgpack`（不变）
- 不支持的 format → 返回 `PyValueError`，消息列出有效选项 `"json, json-pretty, msgpack"`

### 3.3 验证

```bash
cd /home/bot/workbench/repos/tablec/binding-python && \
  cargo build --release && \
  cargo run --example roundtrip --features  # 若有 example
# 现有 pytest 仍过
cd /home/bot/workbench/repos/tablec/binding-python && pytest -v
# 期望：3 个测试文件全部通过
```

新增 2 个 pytest 用例（追加到 `binding-python/tests/test_python_binding.py`）：

- `test_build_json_is_minified_by_default`：构造一个最小可解析 xlsx（复用 `binding-python/tests/test_complex_types.py` 内的 fixture 路径或新写一个 1×1 表 fixture；命名如 `tests/fixtures/minimal.xlsx`）→ 调用 `tablec.build(input, output, "json")` → 读 output → 断言**不含 `\n` 字符**（minified 单行）
- `test_build_json_pretty_has_indentation`：同一 fixture → `tablec.build(input, output, "json-pretty")` → 读 output → 断言**至少含 2 个 `\n` 字符且首层 key 前有 `    ` 缩进**

fixture 复用策略优先：若现有 fixture 太大或 import 麻烦，直接在 conftest 用 `python-calamine` 或 `openpyxl` 现场生成一个最小 xlsx。

### 3.4 影响

- Python 调用者拿到的 json 默认与 CLI 调用者一致
- 文档可加一行"格式支持 json | json-pretty | msgpack"

---

## 4. 改动三：CLAUDE.md / README.md 同步

### 4.1 改动点

| 文档 | 当前位置 | 改动 |
|------|----------|------|
| `CLAUDE.md` Overview / Architecture | "Four commands - build, check, example, and web server" | 改为 "Three commands - build, check, example" |
| `CLAUDE.md` Key Components | "Web server: Basic Actix-web server with hello endpoint" | 删除整行 |
| `CLAUDE.md` Build Commands | `tablec web --listen 127.0.0.1:8080` | 删除整行 |
| `CLAUDE.md` File Structure | `src/core/table/` `- src/export/` `- pybinding/` `- proto/` | 删除 `pybinding/` 与 `proto/`；`pybinding/` 改为 `binding-python/`；`src/export/` 注释加 "JSON, MessagePack（无 protobuf）" |
| `README.md` Features | "Web server" 类似的描述 | 删除 |
| `README.md` Quick Start | `tablec web` 例子 | 删除 |

### 4.2 不动

- 项目定位描述（"table compiler for gamedev"）
- 安装 / 依赖说明
- Excel 数据格式约定
- 开发进度管理（beads / bd）段

### 4.3 验证

```bash
grep -n "actix\|web\|proto\|pybinding" /home/bot/workbench/repos/tablec/CLAUDE.md /home/bot/workbench/repos/tablec/README.md
# 期望：0 命中（CLAUDE.md 的"开发进度管理"段提及 beads/bd 不在此检查范围内）
```

---

## 5. 不在本 spec 范围

- `binding-python` Value enum 完整升级（暴露 `Project`、`Table`、`Value` 给 Python 类型系统；pyo3 `IntoPy` 全量适配）——独立 spec，体量大于本 spec 三项之和
- `protobuf` 输出模块（CLAUDE.md 提到但从未实现，移除 CLAUDE.md 引用 ≠ 实现）
- `Table::validate_constraints` 去包一层（仅是 4 行 wrapper；调用方 `binding-python::check`、`Project::validate_all`、8 个测试位都在用，删掉会扩散改动；保留更直观的调用形式）
- `cargo tarpaulin` / `cargo llvm-cov` 集成到 CI（实现 >95% 覆盖率规则；与本 spec 正交）
- `actix-web` 替换为更轻量的 web 框架（前提是没意义：项目根本不该有 web server）

---

## 6. 落地节奏

3 个 commit + spec doc：

```
c1  doc(spec): tablec CLI 简化与文档同步 (本文件)
c2  chore(cli): remove web command and its dependencies   (改动一)
c3  feat(binding-python): align json defaults with CLI    (改动二)
c4  doc: sync CLAUDE.md and README.md with current state  (改动三)
```

每个 commit 独立可回退。`feat/cli-simplification` 分支（由 superpowers:subagent-driven-development 派发），PR 评审后合入 main。

---

## 7. 决策摘要

| 决策 | 选择 | 否决方案 |
|------|------|----------|
| `web` 命令处置 | 删除 | 替换成轻量 web 框架（前提错误：项目不需要 web） |
| `binding-python` json 默认 | minified（与 CLI 对齐） | 维持 pretty=true |
| `json-pretty` 是否暴露给 Python | 是 | 暂不暴露（API 一致性更重要） |
| 文档更新范围 | CLAUDE.md + README.md | 仅 CLAUDE.md（README 也是用户接触面） |

---

## 8. 关键风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| `web` 命令删除破坏某个隐藏调用方 | 编译失败 | 改动前 grep 全仓；预期 0 命中 |
| binding-python 新测试需要构造 xlsx | 测试不能离线跑 | 复用现有 fixture 或内联构造；改用 JSON 输入路径若 Rust 端允许 |
| 文档同步漏改 | 用户读到过期信息 | grep "web / proto / pybinding / actix" 全文档 0 命中 |
| `actix-web` 删除后某个 `#[tokio::main]` 在 main.rs 残留 | 编译错误 | main.rs 重写为同步 main；非 async |

---

## 9. 附录

### 9.1 与前两个 spec 的关系

- `2026-07-05-tablec-core-cleanup-design.md` — 范围 = tablec-core 内部分层，本 spec 不重叠
- `2026-07-13-tablec-core-design-review.md` — 范围 = core + cli（diag_render），本 spec 是它的下游：移除 cli 不再需要的 dep，对齐 binding-python

### 9.2 references

- skill: `superpowers:brainstorming` — 本 spec 由该 skill 自驱产出（用户授权 self-direct）
- skill: `superpowers:writing-plans` — 由该 skill 转 implementation plan
- skill: `superpowers:subagent-driven-development` — 由该 skill 执行