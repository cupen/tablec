## Why

webui 目前只能回答"工作区相对当前分支 HEAD 改了什么"，策划在功能分支上改表后，更常问的问题是"我的分支相对 main 改了哪些表、哪些单元格"以及"两个分支的表数据现在差什么"——这些问题今天只能退回到命令行 `git diff`，失去文件级状态总览和逐单元格高亮。需要把固定为 HEAD 的对比基线泛化为任选 ref。

## What Changes

- 引入**统一 ref 对比模型**：对比双方为"左侧 ref vs 右侧 ref"，左侧多一个特殊值 `worktree`（当前工作区）；默认 左=`worktree`、右=`HEAD`，与现有行为完全一致。
- 文件级对比泛化：`/api/files` 按所选双方计算每个 spreadsheet 的状态（仅左有 → `added`、仅右有 → `deleted`、两侧不同 → `modified`、相同 → `clean`）及增删行数；文件集合为任一侧存在、且落在输入目录内的 spreadsheet。
- 单元格级对比泛化：`/api/parsed_preview` 的 per-cell diff 相对所选右侧 ref 计算；预览主体渲染左侧内容（左侧为分支时 materialize 该 ref 的 blob）；`deleted` 文件可预览，渲染右侧 blob、全行标 `deleted`。
- 新增 `GET /api/branches`：列出本地 + remote-tracking 分支（去重）。
- webui 前端新增全局对比切换器（左右两个下拉，右侧支持自由输入任意 ref），文件列表徽章、modified-only 过滤、预览高亮全部相对所选对比渲染；选择持久化到 localStorage。
- 对比语义为两个 ref 的直接内容对比（`git diff A B`，two-dot），不做 merge-base。

## Capabilities

### New Capabilities

（无——本变更全部为既有能力的泛化，不引入新能力域。）

### Modified Capabilities

- `git-diff`: 对比基线从固定"当前分支 HEAD"泛化为"左侧 ref vs 右侧 ref"（左侧可为 `worktree`）；方向语义固定为左侧相对右侧（右侧为基线）；文件状态集合从五态（含 `untracked`/`added` 的 staged 语义）调整为分支对比下的四态语义（`added`/`deleted`/`modified`/`clean`，默认 worktree vs HEAD 时保持现有五态行为）；blob 读取与文件 materialize 从 HEAD 扩展到任意 ref。
- `webui`: 新增 `GET /api/branches` 端点；`/api/files` 与 `/api/parsed_preview` 增加对比双方参数；前端增加全局对比切换器与选择持久化。

## Impact

- `tablec-webui/src/git.rs` — `resolve_baseline`/`file_statuses`/`head_blob`/`materialize_head`/`diff_preview` 泛化为任意 ref；新增分支枚举。
- `tablec-webui/src/handlers.rs` — `/api/branches` 新端点；`/api/files`、`/api/parsed_preview` 请求参数与响应字段扩展；ref 校验（不存在的 ref → 400）。
- `tablec-webui/src/router.rs` — 注册 `/api/branches`。
- `tablec-webui/webui/src/` — 前端：对比切换器组件、api.ts 类型、store 状态、file-list/file-preview 状态渲染、localStorage 持久化。
- 不改 CLI 子命令、不改 Python 绑定、不改 `tablec-core`。
- 依赖不变（继续 shell out 到 `git` plumbing 命令，不引入 libgit2）。
