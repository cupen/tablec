#!/usr/bin/env python3
"""qa 一键验收 —— tablec（CLI + 本地 webui + Python 绑定）

由 qa-test 脚手架生成（预设 P-B 改造）：tablec 无容器依赖、无生产部署定义，
环境为宿主机进程——webui 以真实二进制起本地服务器（qa 专属端口），夹具仓库
运行时重建。分层：构建(+单测回归) → 集成(绑定 pytest · 报告集成测试) →
e2e(CLI · webui API · Python 绑定 · webui UI/Playwright)。

用法: python qa/run.py [--keep]    --keep: 保留现场不拆除（排查用）

运行期目录：qa/reports/<run>/ 归档（报告+日志+留证）；qa/.run/ 只放轻量
last-run.json。重型暂存（venv/wheel/夹具/构建产物）在 /tmp tmpfs（QA_SCRATCH
可覆盖位置）：本机 /data 长期满载，cargo 构建与链接的瞬时需求只有 tmpfs 扛
得住，且与开发构建各用各的 CARGO_TARGET_DIR 互不踩踏。tmpfs 占的是内存，
所以退出时默认整目录清空（冷编译只要十几秒）；--keep 才保留。
"""
from __future__ import annotations

import atexit
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.request
from datetime import datetime
from pathlib import Path

QA = Path(__file__).resolve().parent
ROOT = QA.parent
sys.dont_write_bytecode = True          # 不在 qa/lib/ 里留 __pycache__
sys.path.insert(0, str(QA / "lib"))
import qa_tool

RUN_ID = datetime.now().strftime("%Y%m%d-%H%M%S")
REPORTS = QA / "reports" / RUN_ID
LOGS = REPORTS / "logs"
LOGS.mkdir(parents=True, exist_ok=True)
RUN_DIR = QA / ".run"                   # 轻量工作区（last-run.json 等，git 忽略）
KEEP = "--keep" in sys.argv

# 重型暂存（venv/wheel/夹具/构建产物）放 /tmp tmpfs。可用 QA_SCRATCH 覆盖。
SCRATCH = Path(os.environ.get("QA_SCRATCH", "/tmp/qa-tablec"))
# 本工具会放进 SCRATCH 的全部条目；出现别的条目说明目录不是专用的，拒绝清空。
SCRATCH_ENTRIES = {"cargo-target", "venv", "wheels", "fixture-repo"}


def reset_scratch() -> None:
    """每次运行开头清空暂存：上一次 --keep 或异常退出留下的 venv，会让
    `uv venv` 以「environment already exists」拒绝覆盖，整轮绑定层直接挂掉。"""
    if SCRATCH.exists():
        stray = {p.name for p in SCRATCH.iterdir()} - SCRATCH_ENTRIES
        if stray:
            sys.exit(f"qa: QA_SCRATCH={SCRATCH} 含非本工具的条目 {sorted(stray)}，"
                     "拒绝清空；请改用一个专用目录")
        shutil.rmtree(SCRATCH)
    SCRATCH.mkdir(parents=True)


reset_scratch()

# 豁免登记：命中的豁免让全绿也判「通过 · 须人工裁决」（qa-test 第 5 步的 1%）。
# 环境经生产校准后清空本列表，结论才会变成「通过（可上线）」。
EXEMPTIONS = [
    "首次验收——qa 环境刚建立，尚无生产校准基线",
]

# ── 环境常量（qa/env/.env.qa 登记）────────────────────────
APP_HOST = "127.0.0.1"
APP_PORT = 19712                        # qa 专属端口区间
APP_URL = f"http://{APP_HOST}:{APP_PORT}"
BIN = SCRATCH / "cargo-target" / "debug" / "tablec"
WEBUI_DIR = ROOT / "tablec-webui" / "webui"
FIXTURE_REPO = SCRATCH / "fixture-repo"
VENV = SCRATCH / "venv"
VENV_PY = str(VENV / "bin" / "python")
UI_DIR = ROOT / "tests" / "e2e" / "webui-ui"

# 隔离纪律：所有子进程剥离 LD_LIBRARY_PATH（本机 AppImage 会污染 sys.executable）；
# 构建产物统一走 tmpfs 的 qa 专属 CARGO_TARGET_DIR。
CLEAN_ENV = {k: v for k, v in os.environ.items() if k != "LD_LIBRARY_PATH"}
CLEAN_ENV["CARGO_TARGET_DIR"] = str(SCRATCH / "cargo-target")
CLEAN_ENV["TABLEC_BIN"] = str(BIN)    # tests/check_report_tests.py 读取
CLEAN_ENV["QA_TABLEC_BIN"] = str(BIN)
CLEAN_ENV["QA_FIXTURE_REPO"] = str(FIXTURE_REPO)
CLEAN_ENV["QA_WEBUI_URL"] = APP_URL
CLEAN_ENV["QA_VENV_PY"] = VENV_PY


def ensure_ignored(d: Path) -> None:
    """确保目录内嵌 .gitignore 存在——报告与运行数据仅作本地历史，绝不进版本库。"""
    gi = d / ".gitignore"
    if not gi.is_file():
        gi.write_text("*\n!.gitignore\n", encoding="utf-8")


def reset_run_dir() -> None:
    RUN_DIR.mkdir(parents=True, exist_ok=True)
    ensure_ignored(RUN_DIR)
    for p in RUN_DIR.iterdir():
        if p.name == ".gitignore":
            continue
        if p.is_dir():
            shutil.rmtree(p, ignore_errors=True)
        else:
            p.unlink(missing_ok=True)


ensure_ignored(QA / "reports")
reset_run_dir()

LAYERS: list = []
FAILURES: list = []
EVENTS: list = []

SERVER: subprocess.Popen | None = None


def sh(cmd: list, log_name: str, cwd: Path | None = None, env: dict | None = None) -> bool:
    """跑外部命令，输出整体落日志；返回是否成功。日志打开失败按环境故障记失败。"""
    try:
        f = open(LOGS / (log_name + ".log"), "wb")
    except OSError as e:
        EVENTS.append(f"日志打开失败({log_name}): {e}")
        return False
    with f:
        return subprocess.run(
            cmd, cwd=cwd, env=env or CLEAN_ENV,
            stdout=f, stderr=subprocess.STDOUT,
        ).returncode == 0


def run_layer(name: str, cmd: list, cwd: Path | None = None) -> bool:
    """跑一层：输出落 logs/<层名>.log，结果与耗时累积进 LAYERS；提取失败明细。"""
    t0 = time.monotonic()
    ok = sh(cmd, name, cwd=cwd)
    LAYERS.append({
        "name": name,
        "cmd": " ".join(str(c) for c in cmd),
        "pass": ok,
        "duration": int(time.monotonic() - t0),
        "log": f"logs/{name}.log",
    })
    extract_failures(name)
    return ok


def extract_failures(layer: str) -> None:
    """从该层日志提取 pytest FAILED 行 / playwright JSON 失败用例，补进 FAILURES。"""
    log_path = LOGS / f"{layer}.log"
    if not log_path.is_file():
        return
    log = log_path.read_text(errors="replace")
    before = len(FAILURES)
    for m in re.finditer(r"^FAILED (.+?)(?: - .*)?$", log, re.M):
        FAILURES.append({"layer": layer, "case": m.group(1),
                         "detail": "见日志", "evidence": f"logs/{layer}.log"})
    ui_json = RUN_DIR / "ui-results.json"
    if layer == "e2e·UI" and ui_json.is_file():
        try:
            data = json.loads(ui_json.read_text())
            for spec in data.get("suites", []):
                for case in spec.get("suites", []):
                    for test in case.get("specs", []):
                        if test.get("ok") is False:
                            FAILURES.append({
                                "layer": layer,
                                "case": f"{spec.get('title', '')} · {test.get('title', '')}",
                                "detail": "UI 旅程断言失败",
                                "evidence": f"logs/{layer}.log + qa/.run/test-results/",
                            })
        except (json.JSONDecodeError, KeyError):
            pass
    # 该层失败却没解析出用例级明细（环境 / 命令级失败）：把日志尾部当明细，
    # 报告里的「不通过」永远带着可读的原因。
    if not LAYERS[-1]["pass"] and len(FAILURES) == before:
        tail = [line.rstrip() for line in log.splitlines() if line.strip()][-4:]
        FAILURES.append({
            "layer": layer,
            "case": "层命令失败（无用例级明细）",
            "detail": " ⏎ ".join(tail)[:400],
            "evidence": f"logs/{layer}.log",
        })


def todo_layer(name: str, msg: str) -> bool:
    """TODO 占位层：直接记失败，逼着填真命令（不依赖任何解释器）。"""
    LAYERS.append({"name": name, "cmd": f"TODO(qa): {msg}", "pass": False, "duration": 0, "log": ""})
    print(f"TODO(qa): {msg}（层「{name}」未实现）", file=sys.stderr)
    return False


def render_report() -> None:
    """汇总渲染报告、记录 last-run 并退出；退出码即验收结论。"""
    verdict = ("不通过" if any(not layer.get("pass") for layer in LAYERS) or FAILURES
               else "通过 · 须人工裁决" if EXEMPTIONS
               else "通过")
    notes = [
        "无生产部署定义可派生：骨架起步，宿主机进程环境（形态=本地开发工具，容器化不适用）；"
        "重型暂存与构建产物在 /tmp tmpfs（本机 /data 满载），与开发构建各用各的 target",
        *(f"豁免：{e}" for e in EXEMPTIONS),
    ]
    out = REPORTS / "report.md"
    text, failed = qa_tool.render_report(
        LAYERS, FAILURES, EVENTS, notes=notes,
        engine=None, mode="本地进程", project="qa-tablec",
    )
    # qa_tool 只区分通过/不通过；把豁免后的三态结论写回正文标题行。
    if not failed and verdict != "通过":
        text = text.replace("## 结论：通过", f"## 结论：{verdict}", 1)
    out.write_text(text, encoding="utf-8")
    sha, branch, dirty = qa_tool.git_baseline()
    (RUN_DIR / "last-run.json").write_text(json.dumps({
        "run_id": RUN_ID,
        "verdict": verdict,
        "sha": sha,
        "branch": branch,
        "dirty": dirty,
        "finished": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "report": str(out.relative_to(ROOT)),
    }, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"报告: {out}｜结论: {verdict}")
    sys.exit(1 if failed else 0)


def start_server() -> bool:
    """起 webui 服务器（夹具仓库为 --dir），写日志，返回进程句柄。"""
    global SERVER
    try:
        f = open(LOGS / "webui-server.log", "wb")
    except OSError as e:
        EVENTS.append(f"服务器日志打开失败: {e}")
        return False
    SERVER = subprocess.Popen(
        [str(BIN), "webui", "--dir", str(FIXTURE_REPO), "--no-browser", "--port", str(APP_PORT)],
        stdout=f, stderr=subprocess.STDOUT, env=CLEAN_ENV,
    )
    return SERVER.poll() is None


def stop_server() -> None:
    global SERVER
    if SERVER is not None and SERVER.poll() is None:
        SERVER.terminate()
        try:
            SERVER.wait(timeout=10)
        except subprocess.TimeoutExpired:
            SERVER.kill()
    SERVER = None


def cleanup() -> None:
    stop_server()
    if KEEP:
        print(f"[qa] 保留现场：夹具仓库 {FIXTURE_REPO}，暂存 {SCRATCH}，日志 {LOGS}")
        return
    # tmpfs 占的是内存：整目录清空，下次运行冷编译重建。
    shutil.rmtree(SCRATCH, ignore_errors=True)


atexit.register(cleanup)

# ── 1. 构建（前端 dist → cargo build；失败断路）───────────
build_cmd = ["bash", "-c", f"cd {WEBUI_DIR} && pnpm build && cd {ROOT} && cargo build"]
if not run_layer("构建", build_cmd):
    EVENTS.append("构建失败,后续层跳过")
    render_report()

# ── 2. 快速单测回归 ────────────────────────────────────────
run_layer("单测回归", ["cargo", "test", "--workspace"], cwd=ROOT)

# ── 3. 夹具仓库（幂等重建，在 tmpfs）───────────────────────
if not run_layer("夹具", ["uv", "run", "--no-project", "--with", "openpyxl",
                          "python", "-I", str(ROOT / "tests" / "e2e" / "fixtures.py")], cwd=ROOT):
    EVENTS.append("夹具构建失败,后续层跳过")
    render_report()

# ── 4. 起 webui + 探活门禁 ─────────────────────────────────
if not start_server():
    EVENTS.append("webui 进程启动失败")
    render_report()
if not qa_tool.wait_healthy(http=[f"{APP_URL}/api/health"], timeout=60):
    EVENTS.append(f"探活失败:{APP_URL}/api/health 未就绪")
    render_report()
# 身份校验：探活命中的必须是本 run 起的服务器（--dir=夹具仓库），防止端口被
# 同区间遗留进程冒名顶替（曾发生过：旧探针服务器占着 19712，health 200 骗过门禁）。
try:
    with urllib.request.urlopen(f"{APP_URL}/api/state", timeout=10) as resp:
        state = json.loads(resp.read())
    if state.get("dir") != str(FIXTURE_REPO):
        EVENTS.append(f"服务器身份不符:state.dir={state.get('dir')},期望{FIXTURE_REPO}")
        render_report()
except Exception as e:
    EVENTS.append(f"服务器身份校验失败:{e}")
    render_report()

# ── 5. 集成层（真实产物：HEAD wheel + 报告集成测试）────────
# 5a. 绑定 pytest：uv 隔离 venv → maturin 构建 HEAD wheel（产物也走 tmpfs target）→
#     安装 → 从中立 cwd 跑绑定套件
binding_chain = (
    f"uv venv {VENV} && "
    f"uv pip install --python {VENV_PY} maturin pytest openpyxl && "
    f"cd {ROOT / 'binding-python'} && {VENV_PY} -m maturin build -o {SCRATCH / 'wheels'} && "
    f"uv pip install --python {VENV_PY} {SCRATCH}/wheels/tablec-*.whl && "
    f"cd {SCRATCH} && {VENV_PY} -m pytest {ROOT / 'binding-python' / 'tests'} -v"
)
run_layer("集成·绑定", ["bash", "-c", binding_chain])
# 5b. check --report 集成测试
run_layer("集成·报告", [
    "uv", "run", "--no-project", "--with", "openpyxl", "python",
    str(ROOT / "tests" / "check_report_tests.py"),
], cwd=ROOT)

# ── 6. e2e 层 ──────────────────────────────────────────────
PYTEST = ["uv", "run", "--no-project", "--with", "pytest", "--with", "openpyxl",
          "python", "-m", "pytest", "-v"]
run_layer("e2e·CLI", [*PYTEST, str(ROOT / "tests" / "e2e" / "test_cli_journeys.py")], cwd=ROOT)
run_layer("e2e·API", [*PYTEST, str(ROOT / "tests" / "e2e" / "test_webui_api.py")], cwd=ROOT)
run_layer("e2e·绑定", [*PYTEST, str(ROOT / "tests" / "e2e" / "test_binding_journey.py")], cwd=ROOT)

# 6d. webui UI 旅程（Playwright 真浏览器）
ui_install = ["bash", "-c",
              f"cd {UI_DIR} && pnpm install --frozen-lockfile && pnpm exec playwright install chromium"]
ui_ready = sh(ui_install, "e2e·UI-install", cwd=UI_DIR)
if not ui_ready:
    EVENTS.append("Playwright 浏览器安装失败，回退系统 chromium")
    CLEAN_ENV["QA_CHROMIUM_PATH"] = "/usr/sbin/chromium"
run_layer("e2e·UI", ["bash", "-c", "pnpm exec playwright test"], cwd=UI_DIR)

# ── 7. 报告与拆除 ──────────────────────────────────────────
render_report()
