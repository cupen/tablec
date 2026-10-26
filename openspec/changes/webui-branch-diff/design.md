## Context

`tablec-webui/src/git.rs` 今天把对比写死为"工作区 vs 当前分支 HEAD"：`resolve_baseline` 取 HEAD、`file_statuses` 用 porcelain + `--numstat` 出五态、`head_blob` 读 `HEAD:<path>`、`materialize_head` 把 HEAD blob 落成临时文件供 parser 消费、`diff_parsed`/`diff_preview` 做逐单元格比对。前端 file-list 的状态徽章、modified-only 过滤、file-preview 的 per-cell 高亮都建立在 `/api/files` 与 `/api/parsed_preview` 的响应字段上。本设计把这些泛化为"左侧 ref vs 右侧 ref"，需求与场景见 specs delta。

## Goals / Non-Goals

**Goals:**

- 默认参数（`worktree` vs `HEAD`）下行为与现状逐字节一致——现有代码路径保持不动，作为零回归基线。
- 非默认对比走一条统一的"两侧文件集 + blob 内容"通用路径，worktree 与 ref 只是两种"取内容的方式"。
- ref 校验前置：任何不解析的 ref 在进入 diff 前就以 400 拒绝并点名。

**Non-Goals:**

- 不做 merge-base / three-dot 语义、双栏渲染、非 spreadsheet 文件、rename 跟踪（沿用现状：改名表现为一删一增）。
- 不改 CLI 子命令与 Python 绑定；不引入 libgit2（继续 shell out 到 git plumbing）。
- 不做跨请求的 blob 物化缓存（每次请求临时文件，量级为预览单文件，够用；缓存留待有实测需求再加）。

## Decisions

### D1: 两侧统一建模为 `Side`，默认模式走旧路径

`enum Side { Worktree, Ref(String) }` 取代 `Baseline` 里隐含的 "HEAD"。`/api/files`、`/api/parsed_preview` 接受可选 `left`/`right`（默认 `worktree`/`HEAD`）；**当且仅当参数等于默认值时走现有 `file_statuses` 代码路径**，否则走新的通用路径。备选：把默认模式也重写进通用路径——被否，porcelain 的 staged/untracked 五态语义与通用四态语义不同，合并两条路径会把"零回归"变成"重构后对齐"，收益低风险高。

### D2: ref 解析前置，`git rev-parse --verify <ref>^{commit}`

两侧的 ref 先统一 resolve 成 commit sha（`worktree` 侧跳过），失败即 400 点名 ref。后续所有 plumbing 命令用 resolve 出的 sha，避免 `git show branch:path` 在歧义 ref 上的重复解析开销，也天然防注入（ref 始终作为 argv 元素传递，不走 shell）。备选：每条命令直接吃原始 ref 字符串——被否，解析一次既省进程也把"ref 不存在"的错误面收敛到一个点。

### D3: 文件集与状态 = 两侧 listing 的并集 + 定向 diff 分类

通用路径下：

- 左侧文件集：`Worktree` → 沿用现有输入目录扫描（含 untracked）；`Ref` → `git ls-tree -r --name-only <sha> -- <input_dir>`。
- 右侧文件集：`git ls-tree` 同上。
- 并集中的每个文件：仅左有 → `added`；仅右有 → `deleted`；两侧都有 → `git diff --name-status`（左为 `Worktree` 时比较 `<sha>` vs 工作区，左为 ref 时比较 `<leftSha> <rightSha>`，均限定 pathspec）判 `modified`/`clean`，`--numstat` 取增删行数。
- 默认模式（worktree vs HEAD）额外保留 porcelain 的 `untracked`/`added(staged)` 区分——只在旧路径里存在。

### D4: 预览主体与 diff 侧的内容获取统一走 materialize

- 预览主体（raw + parsed 共用规则）：文件在左侧存在 → 左侧内容（`Worktree` 即磁盘文件、`Ref` 即 materialize 的 blob）；仅右侧存在 → materialize 右侧 blob，parsed preview 全行 `deleted`。
- `materialize_head` 泛化为 `materialize_side(side, repo_rel)`：`git show <sha>:<path>` 字节写入临时文件（沿用现有 temp 文件机制与路径去引号逻辑），返回路径供 calamine 解析。
- per-cell diff：把现有 `diff_preview` 的"工作区 parse 结果 vs HEAD parse 结果"泛化为"左内容 parse 结果 vs 右内容 parse 结果"，行匹配（unique field / 行索引）与数值相等逻辑原样复用。

### D5: `GET /api/branches` 用 `git for-each-ref`

`git for-each-ref refs/heads refs/remotes --format=%(refname:short)`，按短名去重、本地优先，返回 `{ branches: [...], warning?: string }`；非仓库 / 无 git 时空列表 + warning（200），与 git-diff spec 的降级风格一致。备选：`git branch --all`——被否，for-each-ref 的 machine-stable 格式更稳。

### D6: 前端 — 独立 `compare-bar` 组件 + store 扩展

新增 Lit 组件 `compare-bar`（左右两个 `wa-select`，右侧开 free-input），挂进 app-shell 顶部；store 增加 `diffSides {left, right}` 状态，变更时重拉 files/preview；`api.ts` 的两个调用统一带 `left`/`right` 参数；localStorage key `tablec.diff-sides` 持久化，`worktree`/`HEAD` 默认值不写入存储。状态徽章、过滤、高亮渲染逻辑不改——它们消费的字段形状没变，只是语义随请求参数换基线。

## Risks / Trade-offs

- [大仓库下 `ls-tree`/`diff` 全量跑在输入目录 pathspec 上仍可能慢] → pathspec 已限定输入目录；实测慢再按 sha+dir 加进程内缓存（Non-Goal 已声明缓存暂不做）。
- [非 ASCII / 引号路径在 `git show <sha>:<path>` 上的匹配问题] → 沿用现有 `unquote_git_path` 与 porcelain `-z` 约定，测试覆盖中文文件名。
- [右侧 free-form ref 输入任意字符串] → D2 的 rev-parse 前置校验兜底；未解析即 400，不做任何回退猜测。
- [分支对比模式下 watcher 广播触发无意义重算] → 幂等重算无害且保持 ws 协议不变；纯分支对比结果与工作区变更天然无关。
- [默认模式与通用路径双实现漂移] → 集成测试同时断言默认参数与显式 `?left=worktree&right=HEAD` 的一致性。

## Migration Plan

纯增量：API 参数可选、默认值即现状；新端点独立。部署顺序：后端先行（旧前端不带参数，行为不变）→ 前端 `pnpm build` 重新嵌入。回滚 = revert 提交，无数据/配置迁移。

## Open Questions

（无——次要细节均已按现有工程惯例定为假设：`#` 前缀 sheet 不参与 diff、`/api/preview` 的 max_rows 钳制不变、modified-only 徽章语义沿用。）
