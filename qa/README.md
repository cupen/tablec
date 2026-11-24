# QA 验收环境

tablec 的上线级 e2e 验收：**99% 的情况下本环境通过即可正式上线**（豁免场景见报告）。
一键验收：`python qa/run.py`（`--keep` 保留现场排查）。报告落 `qa/reports/<run>/report.md`，
run.py 退出码即验收结论（0 = 通过，含「须人工裁决」；1 = 不通过），CI 可直接复用。

## 结论三态

- **通过（可上线）**：三层全绿、无豁免命中。
- **通过 · 须人工裁决**：全绿但命中 `qa/run.py` 里的 `EXEMPTIONS` 登记（当前登记：
  首次验收，环境尚无生产校准基线）。环境经生产校准后清空该列表。
- **不通过（N 项失败）**：失败明细见报告，交裁决。

## 交付形态与环境方式

CLI（真进程）+ 本地 webui（Rust 单二进制内嵌 SPA）+ Python 绑定的组合形态。
仓库**无生产部署定义**（compose/Dockerfile/k8s 均无——本地开发工具，本就没有服务端
部署拓扑），故环境为**骨架起步的宿主机进程**：真实二进制 + 真实服务器 + 真实浏览器
（Playwright），无容器。同构缺口与偏离在每份报告的「遗留与建议」段标注。

## 验收流程（qa/run.py）

1. **构建**：`pnpm install --frozen-lockfile && pnpm build`（前端 dist，嵌入用；新克隆
   没有 `node_modules`，按锁文件现装）→ `cargo build`（debug，遵循 AGENTS.md
   磁盘约定；产物形态与 release 一致）。失败断路。
2. **单测回归**：`cargo test --workspace`。
3. **夹具**：`tests/e2e/fixtures.py` 幂等重建 git 夹具仓库（干净 / modified / untracked /
   含错四态 xlsx，含 `main` 与 `diverge` 两个分支）。
4. **起服务器 + 探活门禁**：`tablec webui --dir <夹具> --no-browser --port 19712`，
   `/api/health` 就绪**且** `/api/state` 报告的目录就是夹具仓库（防止端口被同区间
   遗留进程冒名顶替）才进套件。
5. **集成层**：绑定 pytest（uv 隔离 venv + maturin 现构建 HEAD wheel，从中立 cwd 运行）；
   `tests/check_report_tests.py`（经 `TABLEC_BIN` 指向本次产物）。
6. **e2e 层**：CLI 旅程（真进程）· webui API 旅程（真实 HTTP）· Python 绑定旅程 ·
   webui UI 旅程（Playwright 真浏览器）。
7. **报告与拆除**：渲染 report.md，杀服务器，清空暂存。

## 分层门禁

- 构建 / 探活失败 → 断路，后续层不跑。
- 用例级失败跑完该层再汇总——一次验收看到全部问题。
- 环境级故障（服务器崩溃、依赖不可达）才中断后续层。

## 运行期目录与提交边界

- `qa/reports/<run>/` 归档（报告 + 日志 + 留证）；`qa/.run/` 只放轻量产物
  （last-run.json、Playwright 的 JSON 结果与失败留证），每次运行清空只留最近一次——
  两者**仅作本地运行历史，不进版本库**，由目录内嵌 `.gitignore` 忽略（run.py 每次运行
  自动补齐 keeper），不动仓库根 `.gitignore`。
- 提交：`qa/run.py`、`qa/lib/`、`qa/env/.env.qa`（只放本地占位值）、`qa/README.md`、
  `tests/e2e/`（用例本体，含 `webui-ui/pnpm-lock.yaml`）；忽略：`qa/reports/`、
  `qa/.run/`、`tests/e2e/webui-ui/node_modules/`。

## 隔离纪律

- 重型暂存（venv / wheel / 夹具仓库 / cargo 产物）放 `/tmp/qa-tablec`（`QA_SCRATCH`
  可覆盖）：本机 /data 长期满载，cargo 链接的瞬时需求只有 tmpfs 扛得住；qa 用独立的
  `CARGO_TARGET_DIR`，与开发构建（仓库 `target/`）互不踩踏，也**不往仓库 `target/`
  里放任何东西**。tmpfs 占的是内存，所以退出时默认整目录清空（冷编译只要十几秒）。
- webui 监听 `127.0.0.1:19712`（qa 专属端口区间，见 `qa/env/.env.qa`）；绝不连外部端点。
- 子进程统一剥离 `LD_LIBRARY_PATH`（本机 AppImage 注入会污染 python 解释器）。
- 不复用既有环境；夹具每次运行幂等重建。

## 覆盖清单（功能 → e2e 用例映射，持续维护）

验收语义源：`openspec/specs/`（9 个 capability、66 条 Requirement）。
目标：绝大多数功能都有旅程用例；每次验收顺手补缺口，新功能进清单才算了账。

| 功能/旅程 | 用例 | 状态 |
|---|---|---|
| CLI：单文件 build → JSON | `tests/e2e/test_cli_journeys.py::test_build_single_file_to_json` | ✅ |
| CLI：零配置目录 build → output/default.json | `::test_build_directory_zero_config_writes_default_output` | ✅ |
| CLI：msgpack 格式与扩展名（显式 `-o` 原样、默认名随格式） | `::test_build_msgpack_format_and_extension` | ✅ |
| CLI：check 退出码（干净 / 出错） | `::test_check_exit_codes_clean_vs_error` | ✅ |
| CLI：example 拒绝覆盖 / `--force` | `::test_example_refuses_overwrite_then_force` | ✅ |
| CLI：`--report` 双格式落盘（JSON 信封 + 自包含 HTML） | `::test_check_report_writes_both_formats` | ✅ |
| CLI：config 发现优先级（cwd 胜出） | `::test_config_discovery_cwd_wins_over_target` | ✅ |
| CLI：未注册 parser 失败点名 | `::test_unknown_parser_fails_naming_the_parser` | ✅ |
| webui：`/api/files` git 状态 + modified-only 过滤 | `test_webui_api.py::test_files_git_statuses_and_modified_filter` | ✅ |
| webui：sheets / preview / parsed_preview 结构 + 逐格 diff | `::test_sheets_preview_parsed_preview_shapes` | ✅ |
| webui：build API（预览 / write:true / 未知格式 400） | `::test_build_api_writes_artifact_on_request` | ✅ |
| webui：check API 与 CLI 同诊断集 | `::test_check_api_matches_cli_diagnostic_set` | ✅ |
| webui：错误路径（ref 400 / validate 501 / plugin_paths 400 / branches 去重 / 404） | `::test_error_paths_ref_reject_validate_501_plugin_paths` | ✅ |
| 绑定：build → check 全链路 + 包面 + 错误映射 | `test_binding_journey.py::test_binding_build_check_round_trip` | ✅ |
| webui UI：文件树 → 选文件 → 预览渲染 | `webui-ui/tests/journeys.spec.ts` #1 | ✅ |
| webui UI：Check 结果 → 文件行错误徽章 → modified-only 过滤 | `webui-ui/tests/journeys.spec.ts` #2 | ✅ |
| webui UI：新增文件 → 实时通知 → 自动刷新，空闲与刷新都不循环 | `webui-ui/tests/journeys.spec.ts` #3 | ✅ |
| 集成：绑定 pytest 套件（11 例） | `binding-python/tests/` | ✅ |
| 集成：`check --report` 7 场景 | `tests/check_report_tests.py` | ✅ |
| webui UI：比较选择器切分支（localStorage 持久化） | — | ⛔ 缺口 |
| webui UI：WS 断线重连后自动刷新 | —（重连逻辑在 `live.ts`） | ⛔ 缺口 |
| webui UI：目录树折叠 / 展开状态跨刷新保持 | — | ⛔ 缺口 |
| webui UI：排序三因子 + 方向切换 | — | ⛔ 缺口 |
| CLI：`--parser` / `--plugin-path` 走自定义 cdylib 插件 | —（cdylib e2e 在 `tablec-core/tests/cdylib_e2e.rs`，默认 ignored） | ⛔ 缺口 |

## 已知偏差（每份报告重复标注）

- 骨架起步、无派生源（无生产部署定义）；本地进程环境，无容器。
- 单测回归用 debug 构建（AGENTS.md 磁盘约定），release 产物差异未覆盖。
- 重型暂存在 `/tmp` tmpfs（见隔离纪律）。

## 工具（vendor 自 qa-test skill 单文件 qa_tool.py；`qa_tool.py scaffold --refresh-libs` 更新）

- `qa/lib/qa_tool.py`：check / probe / port-free / wait / leftovers / report / scaffold
