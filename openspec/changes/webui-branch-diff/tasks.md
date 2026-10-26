## 1. Side 模型与 ref 解析（tablec-webui/src/git.rs）

- [x] 1.1 引入 `Side`（`Worktree` / `Ref`）并实现 `resolve_side`：ref 先经 `git rev-parse --verify <ref>^{commit}` 解析为 sha，解析失败返回点名该 ref 的错误；`Worktree` 跳过 git。单测覆盖：分支名/tag/缩写 sha 解析成功、未知 ref 报错点名、非仓库目录返回无 diff 状态（沿用现有 temp repo 测试设施）
- [x] 1.2 实现 `branches(dir)`：`git for-each-ref refs/heads refs/remotes --format=%(refname:short)`，短名去重且本地优先，非仓库/无 git 时返回空+警告而非报错。单测覆盖：本地+remote 列出、`main` 与 `origin/main` 去重、非仓库返回空

## 2. 通用文件级对比（tablec-webui/src/git.rs）

- [x] 2.1 实现两侧文件集获取：`Worktree` 侧复用现有输入目录扫描（含 untracked），`Ref` 侧 `git ls-tree -r --name-only <sha> -- <input_dir>`，均过滤受支持扩展名与路径包含检查。单测覆盖：ref 侧列出仓库内表格文件、非 ASCII 文件名路径匹配
- [x] 2.2 实现通用状态分类 `file_statuses_sides(left, right)`：并集上按"仅左 `added` / 仅右 `deleted` / 两侧 `git diff --name-status`（pathspec 限定）判 `modified`/`clean`、`--numstat` 取增删行数"。单测覆盖四种状态各一例、modified 计数、与 porcelain 路径在默认参数下结果一致的等价测试
- [x] 2.3 确认默认模式（无参数）仍走现有 porcelain 五态路径且行为不变：现有 `file_statuses` 测试全绿，不改动其代码路径

## 3. 内容物化与单元格 diff 泛化（tablec-webui/src/git.rs）

- [x] 3.1 把 `materialize_head` 泛化为 `materialize_side(side, repo_rel)`：`git show <sha>:<path>` 字节写入临时文件供 calamine 解析，沿用现有 temp 文件与路径去引号机制。单测覆盖：返回 ref blob 字节、路径不存在时报错
- [x] 3.2 把 `diff_preview` 的"工作区 vs HEAD"泛化为"左内容 vs 右内容"（两侧各自 parse，行匹配与数值相等逻辑复用 `diff_parsed`），并支持仅右存在文件全行 `deleted`。单测覆盖：分支 blob 对比出的 modified cell / added row / deleted row、deleted 文件全行 deleted、int vs float 等值不算差异

## 4. HTTP API（tablec-webui/src/handlers.rs、router.rs）

- [x] 4.1 新增 `GET /api/branches` 并注册路由：返回 `{ branches, warning? }`，非仓库 200 空列表+warning。handler 单测覆盖：列出与去重、非仓库降级、`/api/*` 未知路径仍 404 的既有行为不回归
- [x] 4.2 `/api/files` 接受可选 `left`/`right`（默认 `worktree`/`HEAD`）：默认走旧路径；非默认走 `file_statuses_sides`；未知 ref 返回 400 点名。handler 单测覆盖：temp repo 双分支对比出 added/deleted/modified、未知 ref 400、默认参数与无参数响应一致
- [x] 4.3 `/api/parsed_preview` 与 `/api/preview` 接受 `left`/`right`：预览主体按"左存在取左（ref 则 materialize），仅右存在取右 blob"；parsed per-cell diff 相对右侧；未知 ref 400。handler 单测覆盖：`left=分支` 时主体为该分支 blob、仅右存在文件 200 全行 deleted、默认参数行为不变

## 5. 前端（tablec-webui/webui/src）

- [x] 5.1 `api.ts`：`fetchFiles`/`fetchParsedPreview`（及 raw preview）统一携带 `left`/`right` 参数，新增 `fetchBranches()` 与类型；`pnpm check` 通过
- [x] 5.2 `store.ts`：新增 `diffSides {left, right}` 状态与变更动作，选择持久化到 localStorage（key `tablec.diff-sides`，默认值不落盘、加载时缺失回退默认），变更后自动重拉 files 与当前 preview；类型与逻辑有测试或 `pnpm check` + 手动验证
- [x] 5.3 新增 `compare-bar` 组件（左：worktree+分支下拉；右：分支下拉+自由输入 ref；400 错误在组件内展示点名信息）并挂入 `app-shell`；file-list 徽章、modified-only 过滤、file-preview 高亮不改渲染逻辑即随新基线生效；`pnpm check && pnpm build` 通过并重新 `cargo build` 嵌入

## 6. 集成验证

- [x] 6.1 端到端验证（`cargo test` 全绿 + 手动 webui）：temp repo 双分支场景下 `/api/files?left=A&right=B` 状态正确、点开仅右侧存在文件全行 deleted、默认打开行为与变更前一致、刷新页面选择记忆保持；按项目规则确认 tablec-webui 单测覆盖达标
