# tablec build 目录支持 — 设计稿

**日期**: 2026-07-26
**仓库**: `repos/tablec`
**前置**: PR #1 (`feat/tablec-core-cleanup`) + PR #2 (`feat/core-design-review`) + PR #3 (`feat/cli-simplification`) 已合入 main (`f039596`)
**范围**: `tablec build` 子命令接受目录输入，auto-discover `tablec.toml`，合并输出
**不做**: `--per-file` 模式（独立输出）、`--include` / `--exclude` CLI 标志、`binding-python` 的目录支持、递归扫描默认开启

---

## 1. 背景与目标

当前 `tablec build` 的两个分支行为不对称：
- **单文件**：`tablec build -i foo.xlsx -o out.json`（强制要求 `-i` + `-o`）
- **目录**：仅在 `-c tablec.toml` 路径下存在，且固定为"合并单输出"
- **裸目录**（无 config）：直接报错

`check` 子命令早就支持裸目录（`path.is_dir()` 分支），`build` 与之不一致。本 spec 把目录模式提到一等位置：

1. `tablec build`（无参数）= 构建当前目录
2. `tablec build ./path/to/dir` = 构建指定目录
3. auto-discover `<input>/tablec.toml` 或 `<input>/.tablec.toml`，决定细节
4. 目录模式固定合并输出（`Project::from_tables` 语义不变）
5. 单文件模式 `-i foo.xlsx -o out.json` 保留兼容

本 spec 不改 `Project::from_tables` 的合并语义（同名 sheet 后者覆盖前者），不改 `binding-python`，不动 `check`。

---

## 2. 行为矩阵

| 调用 | `-i` 解析 | 行为 |
|------|-----------|------|
| `tablec build` | cwd `.` | auto-discover `./tablec.toml`；有则按 config 走 dir-合并；无则默认 `*.xlsx` + 默认格式走 dir-合并 |
| `tablec build ./data` | dir `./data` | 同上但 input_dir 是 `./data` |
| `tablec build -i foo.xlsx` | file `foo.xlsx` | 单文件，要求 `-o` |
| `tablec build -c foo.toml` | cwd `.` | 用显式 config，cwd 当 input_dir，不 auto-discover dir |
| `tablec build -c foo.toml -i bar.xlsx` | file | 显式 config 优先；bar.xlsx 走单文件 |

**`-o` 默认值与校验**：

| 模式 | `-o` 缺省 | `-o` 类型校验 |
|------|----------|--------------|
| 单文件 | 报错"missing -o" | 必须是文件路径（父目录需可创建） |
| dir-合并 | 有 config：`<cfg.export.output_dir>/<cfg.project.name>.<ext>`；无 config：报错"missing -o (or tablec.toml)" | 必须是文件路径 |

`<ext>`：format=msgpack → `msgpack`，否则 → `json`（沿用 `build.rs:102-108` 现逻辑）。

---

## 3. CLI 形状改动

```rust
// tablec-cli/src/cmd/build.rs
#[derive(Args, Debug)]
pub struct BuildCommand {
    /// Input Excel file or directory of Excel files.
    /// Defaults to current directory if omitted.
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (merge mode / single-file).
    #[arg(short, long)]
    pub output: Option<String>,

    /// Config file path. If omitted in directory mode, the
    /// tool searches for `tablec.toml` / `.tablec.toml`
    /// inside the input directory.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Export format: json (minified) | json-pretty (indented) | msgpack.
    /// Overrides config when given.
    #[arg(long)]
    pub format: Option<String>,

    /// Include field metadata. Overrides config when given.
    #[arg(long)]
    pub include_fields: Option<bool>,
}
```

**`-i` 解析内部约定**：保持 `Option<String>`，进入 `run()` 后：

```rust
let input_path: PathBuf = match self.input.as_deref() {
    Some(s) => PathBuf::from(s),
    None   => PathBuf::from("."),
};
```

**`-o` 类型校验**：进入 dir / merge 分支后、写到磁盘前校验；fail fast，错误信息具体（"missing -o (or tablec.toml)" for dir mode无 config 时）。

---

## 4. dir 分支实现

`build.rs::run()` 的 dispatch 改造（替换当前 `match config { Some | None }`）：

```rust
pub fn run(&self) -> Result<(), Box<dyn Error>> {
    let input_path = self.input.as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    let explicit_cfg = Config::load(self.config.as_deref())?;

    match (input_path.is_dir(), input_path.is_file()) {
        (true, _)  => self.run_dir(input_path, explicit_cfg),
        (false, true) => self.run_single(input_path, explicit_cfg),
        (false, false) => Err(format!("input {:?} is neither a file nor a directory", input_path).into()),
    }
}
```

`run_single`：单文件路径，复用现有 `build_single_file` 逻辑；要求 `-o` 必填；config 可选（仅用其 `export.*` 字段作默认）。

`run_dir(input_dir, explicit_cfg)`：

1. 解析有效 cfg：
   - `explicit_cfg` 已是 `Some(cfg)` → 用它（不再 auto-discover）
   - 否则在 `<input_dir>/tablec.toml` 和 `<input_dir>/.tablec.toml` 中查找，存在则 `load_from_file`
   - 都没有 → `Config::default()`（其 `data.include` 已是 `Some(vec!["*.xlsx"])`）
2. CLI `--format` / `--include-fields` 覆盖 cfg 对应字段
3. 调用 `config::find_excel_files(input_dir, &cfg.data.include.clone().unwrap_or_default(), &cfg.data.exclude.clone().unwrap_or_default())`
4. 空 → 报错"directory contains no xlsx files matching config"
5. 调用 `build_merged_files(&files, &output_file, &format, include_fields)`（已存在）

---

## 5. `Config::load` 复用与最小变更

不动 `config.rs::load()` 的现有语义。auto-discover 由 `run_dir` 内联完成（一次性两行：检查 `tablec.toml` / `.tablec.toml`）。原因：

- 现有 `Config::load` 搜索 cwd，与 dir 模式目标不同
- 抽出 `Config::load_from(dir: &Path)` 会让 API 表面扩大而本 spec 不需要
- 单点逻辑不需要新 helper

```rust
fn find_tablec_toml(dir: &Path) -> Option<PathBuf> {
    ["tablec.toml", ".tablec.toml"].iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}
```

放在 `build.rs::run_dir` 私有函数里。

---

## 6. 测试

`tablec-cli/src/cmd/build.rs` 已有 `#[cfg(test)] mod tests`，扩充：

| 测试 | 覆盖 |
|------|------|
| `test_find_tablec_toml_prefers_tablec_over_dotfile` | auto-discover 优先级：`tablec.toml` 存在时忽略 `.tablec.toml` |
| `test_find_tablec_toml_returns_none_when_absent` | 两个都不存在 → `None` |
| `test_dir_mode_uses_default_when_no_config` | tempdir 建 2 个 xlsx + 1 个 .csv（应被忽略），无 tablec.toml → 断言输出 1 个合并文件且 .csv 未进 |
| `test_dir_mode_auto_discovers_tablec_toml` | tempdir + tablec.toml 设 `include = ["only.xlsx"]` → 断言只 build `only.xlsx` |
| `test_single_file_still_requires_output` | `-i foo.xlsx` 无 `-o` → `Err`（回归：保留旧行为） |

**fixture 制造**：用 `rust_xlsxwriter`（已在 deps）程序化生成最小 xlsx；fixture 路径走 `tempfile::tempdir()`（已在 deps）。

---

## 7. 文档更新

`README.md` 新增"Building a directory"小节（紧跟"Quick Start / Build Excel to JSON"之后）：

```markdown
### Build a directory

By default `tablec build` reads the current directory:

\`\`\`bash
tablec build                       # build ./tablec.toml or all *.xlsx in cwd → ./<project>.json
tablec build ./data                # same, against ./data
\`\`\`

If the input directory contains `tablec.toml` (or `.tablec.toml`), it controls
include patterns, output name, format, and so on. `--config path/to/other.toml`
overrides auto-discovery.

### Build a single file (legacy)

\`\`\`bash
tablec build -i data/foo.xlsx -o out/foo.json
\`\`\`
```

`CLAUDE.md` `Build Commands` 段补一行 `tablec build [path]`。

---

## 8. 决策摘要

| 决策 | 选择 | 否决方案 |
|------|------|----------|
| 缺省 input | cwd `.` | 报错"missing input"（破坏现状） |
| Auto-discover 路径 | `<input>/tablec.toml` 优先、`<input>/.tablec.toml` 次之 | cwd-only（与 spec 冲突） |
| 输出模式 | 仅合并（与现有 config 路径一致） | 加 `--per-file` 切独立输出（增加 CLI 面积；本期不做） |
| 单文件失败后续 | 整个 build 退出 | partial success（避免歧义） |
| 不加 `--include` / `--exclude` CLI 标志 | 走 config | 增加 CLI 面积 |

---

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| cwd 解析在 CI 上行为不一致 | 测试 flake | 测试用 `tempfile::tempdir()` 显式切到 tempdir |
| Auto-discover 找到意外 `tablec.toml` | 输出到非预期位置 | spec 限定只搜 `<input>/`、不搜 cwd；错误信息明确 |
| `binding-python::build()` 在 cargo workspace 下被 link，新 CLI 行为不影响 | 无破坏 | 不改 `binding-python/src/lib.rs` |
| 单文件 + 无 `-o` 的旧脚本被打断 | 旧用户失败 | `-o` 报错信息保留原话（"No output file specified"） |
| `tempfile::tempdir()` 与 `rust_xlsxwriter` 已在 deps | 无 | 不新增依赖 |

---

## 10. 不在本 spec 范围

- `--per-file` 模式（独立输出）——本期不做；用户明确说"不要复杂"
- `--include` / `--exclude` CLI 标志（用户已选不加；走 config）
- `binding-python::build()` 的目录支持（Python 端在另一个 spec）
- 递归扫描默认开启（`find_excel_files` 已支持 `**/`，由 config 自决）

---

## 11. 落地节奏

单 PR、3 个 commit：

```
c1  doc(spec): build dir support (this spec)
c2  feat(cli): -i defaults to cwd; auto-discover tablec.toml
c3  doc: README + CLAUDE.md — Building a directory section
```

每个 commit 独立可回退；c2 的回滚粒度 = 单个 BuildCommand 重构。

---

## 12. 附录

### 12.1 与前几个 spec 的关系

- `2026-07-05-tablec-core-cleanup-design.md`：不重叠（core 内部）
- `2026-07-13-tablec-core-design-review.md`：不重叠（core / cli 错误呈现）
- `2026-07-25-tablec-cli-simplification-design.md`：本 spec 是其下游；上一个 PR 把 `web` 命令清掉、给 CLI 减负，本 spec 把 build 子命令本身扩展

### 13.2 references

- skill: `superpowers:brainstorming` — 本 spec 由该 skill 流程产出
- skill: `superpowers:writing-plans` — 由该 skill 转 implementation plan
- skill: `superpowers:subagent-driven-development` — 由该 skill 执行
- 相关 `tablec-core/src/core/config.rs::find_excel_files`：本 spec 复用，不改
- 相关 `tablec-cli/src/cmd/check.rs`：作为目录行为一致性的参考样板，不改