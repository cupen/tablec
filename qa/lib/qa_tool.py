#!/usr/bin/env python3
"""qa-test 全量工具（单文件、纯标准库、跨平台，无 shell 依赖）。

子命令:
  check [--dir D]                    分流检测: READY / HALF N / MISSING（未搭建），附上次验收结论
  probe [--dir D]                    摸清现场: 交付形态 / 测试层 / 环境设施 / 容器引擎
  port-free [--start N] [--end N] [--count N] [--sep S]
  wait --tcp h:p [--tcp ...] [--http URL ...] [--timeout S] [--interval S]
  leftovers --project P [--engine docker|podman] [--prune]
  report --layer 'K=V|K=V' [--failure ...] [--event ...] [--note ...] [--out F]
  scaffold --preset P-X [--dir D] [--engine E] [--ports a,b] [--force]
         [--refresh-libs] [--list-presets]

scaffold 会把本文件 vendor 进目标仓库 qa/lib/，run.py 直接 import 其中的
wait_healthy / render_report / leftovers / git_baseline 函数，脱离本 skill 独立复跑。

运行期目录约定（scaffold 自动生成内嵌 .gitignore，不动仓库根 .gitignore）:
  qa/reports/<run>/   归档：report.md + logs + 留证截图，按 run 保留
  qa/.run/            工作区：框架原始输出 + last-run.json，每次运行清空只留最近一次
"""

from __future__ import annotations

import argparse
import contextlib
import fnmatch
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import time
import urllib.request
from datetime import datetime
from pathlib import Path
from urllib.error import URLError

TOOL_FILE = Path(__file__).resolve()

# ───────────────────────── check：分流检测 ─────────────────────────


def triage(repo: Path) -> tuple:
    """以 qa/run.py 为锚点判环境状态：READY / HALF N / MISSING。"""
    run_py = repo / "qa" / "run.py"
    if not run_py.is_file():
        return "MISSING", 2
    body = run_py.read_text(encoding="utf-8")
    n = body.count('todo_layer("')
    if n:
        return f"HALF {n}", 1
    return "READY", 0


def last_run(repo: Path) -> dict:
    """读 qa/.run/last-run.json；没有或损坏返回 {}。"""
    p = repo / "qa" / ".run" / "last-run.json"
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def cmd_check(args: argparse.Namespace) -> int:
    repo = Path(args.dir)
    verdict, rc = triage(repo)
    hints = {
        "READY": "已就绪: 直接 python qa/run.py",
        "HALF": "搭建中断: 补 qa/run.py 里残留的 todo_layer 占位",
        "MISSING": "未搭建: 走摸清 → 计划确认 → 搭建",
    }
    key = verdict.split()[0]
    print(verdict)
    print(f"# {hints[key]}", file=sys.stderr)
    prev = last_run(repo)
    if prev:
        print(
            f"# 上次验收: {prev.get('finished', '?')} {prev.get('verdict', '?')}"
            f"（{prev.get('report', '?')}）",
            file=sys.stderr,
        )
    return rc


# ───────────────────────── probe：摸清现场 ─────────────────────────

SKIP_DIRS = {
    ".git", "node_modules", "target", "dist", "build", ".venv", "venv",
    "__pycache__", ".cache", "vendor", ".idea", ".vscode", ".gradle",
    "coverage", "*.egg-info",
}

PROBE_PATTERNS = {
    "构建/清单": [
        "package.json", "go.mod", "Cargo.toml", "pyproject.toml",
        "requirements.txt", "pom.xml", "build.gradle*", "Makefile", "justfile",
        "project.godot", "*.uproject",
    ],
    "容器/部署": [
        "Dockerfile", "*/Dockerfile", "*/Dockerfile.*",
        "docker-compose*.yml", "docker-compose*.yaml",
        "compose*.yml", "compose*.yaml", "*/compose*.yml", "*/compose*.yaml",
        "k8s/*", "deploy/*", "charts/*",
    ],
    "e2e 设施": [
        "playwright.config.*", "cypress.config.*", "cypress/*",
        "e2e/*", "tests/e2e/*", "test/e2e/*",
    ],
    "环境/配置": [".env", ".env.*", "*.env"],
    "CI": [".github/workflows/*", ".gitlab-ci.yml", ".circleci/*"],
    "规格": ["openspec/*", "openspec/*/*", "specs/*"],
}


def iter_repo_files(root: Path, max_depth: int = 4):
    root = root.resolve()
    base_depth = len(root.parts)
    for dirpath, dirnames, filenames in os.walk(root):
        if len(Path(dirpath).parts) - base_depth >= max_depth:
            dirnames[:] = []
        dirnames[:] = [d for d in dirnames if not any(fnmatch.fnmatch(d, s) for s in SKIP_DIRS)]
        for f in filenames:
            yield Path(dirpath) / f


def engine_ok(name: str) -> bool:
    try:
        return subprocess.run([name, "info"], capture_output=True, timeout=10).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def env_keys(path: Path) -> list:
    """只取键名，绝不输出值。"""
    keys = []
    try:
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                keys.append(line.split("=", 1)[0])
    except OSError:
        pass
    return keys


def cmd_probe(args: argparse.Namespace) -> int:
    repo = Path(args.dir).resolve()
    files = [p.relative_to(repo).as_posix() for p in iter_repo_files(repo)]

    for bucket, pats in PROBE_PATTERNS.items():
        hits = sorted({f for f in files for pat in pats if fnmatch.fnmatch(f, pat)})
        print(f"== {bucket} ==")
        if not hits:
            print("  （无）")
            continue
        for f in hits[:8]:
            extra = ""
            if fnmatch.fnmatch(f, ".env*") or f.endswith(".env"):
                ks = env_keys(repo / f)
                extra = f"  键: {', '.join(ks[:12])}" + ("…" if len(ks) > 12 else "")
            print(f"  {f}{extra}")
        if len(hits) > 8:
            print(f"  …共 {len(hits)} 项")

    print("== 容器引擎 ==")
    for name in ("docker", "podman"):
        print(f"  {name}: {'ok' if engine_ok(name) else 'no'}")

    verdict, _ = triage(repo)
    print(f"== qa 环境 ==\n  qa/run.py: {verdict.split()[0]}")
    return 0


# ───────────────────────── 环境小件 ─────────────────────────


def port_is_free(port: int) -> bool:
    """0.0.0.0 绑定测试：任一接口被占即视为不空闲（不用 SO_REUSEADDR，避免误判）。"""
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(("0.0.0.0", port))
        return True
    except OSError:
        return False


def cmd_port_free(args: argparse.Namespace) -> int:
    found = [p for p in range(args.start, args.end + 1) if port_is_free(p)]
    if len(found) < args.count:
        print(
            f"端口不足: {args.start}-{args.end} 仅 {len(found)} 个空闲, 需 {args.count}",
            file=sys.stderr,
        )
        return 1
    print(args.sep.join(str(p) for p in found[: args.count]))
    return 0


def tcp_ok(target: str) -> tuple:
    host, _, port = target.rpartition(":")
    if not host or not port.isdigit():
        return False, f"非法 --tcp 目标: {target}(应为 host:port)"
    try:
        with socket.create_connection((host, int(port)), timeout=2):
            return True, "connected"
    except OSError as e:
        return False, str(e)


def http_ok(url: str) -> tuple:
    try:
        with urllib.request.urlopen(url, timeout=2) as resp:
            status = resp.getcode() or 0
    except URLError as e:
        return False, str(e.reason)
    except OSError as e:
        return False, str(e)
    return (200 <= status < 400, f"HTTP {status}")


def wait_healthy(tcp=None, http=None, timeout=120, interval=2) -> bool:
    """轮询探活：全部目标健康返回 True，超时返回 False（供 run.py 直接 import）。"""
    targets = [("tcp", t) for t in tcp or []] + [("http", u) for u in http or []]
    if not targets:
        raise ValueError("wait 至少需要一个 tcp 或 http 目标")
    deadline = time.monotonic() + timeout
    last = {}
    while time.monotonic() < deadline:
        healthy = True
        for kind, target in targets:
            ok, msg = tcp_ok(target) if kind == "tcp" else http_ok(target)
            last[target] = msg
            healthy = healthy and ok
        if healthy:
            for t, msg in last.items():
                print(f"healthy: {t} ({msg})")
            return True
        time.sleep(interval)
    for _, target in targets:
        print(f"UNHEALTHY: {target} 最后状态: {last.get(target, '未探测')}", file=sys.stderr)
    return False


def cmd_wait(args: argparse.Namespace) -> int:
    try:
        return 0 if wait_healthy(args.tcp, args.http, args.timeout, args.interval) else 1
    except ValueError as e:
        print(str(e), file=sys.stderr)
        return 1


LS_CMDS = {
    "container": ["ps", "-a", "--format", "{{.Names}}"],
    "volume": ["volume", "ls", "--format", "{{.Name}}"],
    "network": ["network", "ls", "--format", "{{.Name}}"],
}
RM_CMDS = {
    "container": ["rm", "-f"],
    "volume": ["volume", "rm"],
    "network": ["network", "rm"],
}


def engine_ls(engine: str, kind: str) -> list:
    try:
        out = subprocess.run(
            [engine, *LS_CMDS[kind]], capture_output=True, text=True, timeout=30
        )
    except (OSError, subprocess.TimeoutExpired):
        return []
    if out.returncode != 0:
        print(f"警告: {engine} {kind} 列举失败: {out.stderr.strip()}", file=sys.stderr)
        return []
    return [line.strip() for line in out.stdout.splitlines() if line.strip()]


def leftovers(project: str, engine: str = "docker", prune: bool = False) -> bool:
    """核查 / 清理 qa 命名空间残留；干净返回 True。"""
    try:
        probe = subprocess.run([engine, "info"], capture_output=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired):
        print(f"容器引擎不可用: {engine} 不存在或无响应", file=sys.stderr)
        return False
    if probe.returncode != 0:
        print(f"容器引擎不可用: {engine} info 失败", file=sys.stderr)
        return False

    found: dict = {}
    for kind in LS_CMDS:
        hits = [n for n in engine_ls(engine, kind) if n.startswith(project)]
        if hits:
            found[kind] = hits
    if not found:
        print(f"干净: 未发现 {project}* 残留")
        return True

    if not prune:
        for kind, names in found.items():
            for n in names:
                print(f"残留 {kind}: {n}", file=sys.stderr)
        total = sum(len(v) for v in found.values())
        print(f"共 {total} 项 {project}* 残留(--prune 可清理)", file=sys.stderr)
        return False

    for kind, names in found.items():
        for n in names:
            r = subprocess.run(
                [engine, *RM_CMDS[kind], n], capture_output=True, text=True, timeout=60
            )
            if r.returncode != 0:
                print(f"清理失败 {kind} {n}: {r.stderr.strip()}", file=sys.stderr)
    still = {
        k: v
        for k in found
        if (v := [n for n in engine_ls(engine, k) if n.startswith(project)])
    }
    if still:
        for kind, names in still.items():
            for n in names:
                print(f"未能清除 {kind}: {n}", file=sys.stderr)
        return False
    print(f"已清理 {sum(len(v) for v in found.values())} 项 {project}* 残留")
    return True


def cmd_leftovers(args: argparse.Namespace) -> int:
    return 0 if leftovers(args.project, args.engine, args.prune) else 1


# ───────────────────────── 报告渲染 ─────────────────────────


def parse_kv(item: str) -> dict:
    out = {}
    for kv in item.split("|"):
        if "=" in kv:
            k, v = kv.split("=", 1)
            out[k.strip()] = v.strip()
    return out


def _is_true(v) -> bool:
    return str(v).lower() == "true"


def _int(v, default=0) -> int:
    try:
        return int(v)
    except (TypeError, ValueError):
        return default


def fmt_dur(sec) -> str:
    sec = _int(sec, -1)
    if sec < 0:
        return "—"
    if sec < 60:
        return f"{sec}s"
    m, s = divmod(sec, 60)
    if m < 60:
        return f"{m}m{s:02d}s"
    h, m = divmod(m, 60)
    return f"{h}h{m:02d}m"


def git_baseline() -> tuple:
    def run(*cmd: str) -> str:
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
            return r.stdout.strip() if r.returncode == 0 else "unknown"
        except (OSError, subprocess.TimeoutExpired):
            return "unknown"

    sha = run("git", "rev-parse", "--short", "HEAD")
    branch = run("git", "rev-parse", "--abbrev-ref", "HEAD")
    dirty = run("git", "status", "--porcelain") not in ("", "unknown")
    return sha, branch, dirty


def render_report(
    layers: list, failures: list, events: list, notes: list, *,
    engine=None, mode=None, project=None,
    duration_sec=None, sha=None, branch=None,
) -> tuple:
    """渲染报告，返回 (markdown, 是否不通过)。layer 键: name/cmd/pass/duration/total/failed/log。"""
    if sha is None or branch is None:
        g_sha, g_branch, dirty = git_baseline()
        sha = sha or g_sha
        branch = branch or g_branch
    else:
        _, _, dirty = git_baseline()
    baseline = f"{sha} ({branch}{', dirty' if dirty else ''})"

    env_bits = [b for b in (engine, mode, project) if b]
    total_dur = _int(duration_sec) or sum(_int(l.get("duration")) for l in layers)

    any_fail = any(not _is_true(l.get("pass")) for l in layers) or bool(failures)
    if not any_fail:
        conclusion = "通过"
    elif failures:
        conclusion = f"不通过（{len(failures)} 项失败）"
    else:
        conclusion = "不通过（失败层未附明细）"

    lines = [
        f"# QA 验收报告 · {time.strftime('%Y-%m-%d %H:%M')}",
        f"基线：{baseline}｜环境：{' · '.join(env_bits) if env_bits else '未登记'}｜总时长：{fmt_dur(total_dur)}",
        f"## 结论：{conclusion}",
    ]
    for l in layers:
        mark = "✅" if _is_true(l.get("pass")) else "❌"
        bits = [f"- {l.get('name', '?')} {mark}"]
        if l.get("cmd"):
            bits.append(str(l["cmd"]))
        if l.get("total") not in (None, ""):
            ok = _int(l["total"]) - _int(l.get("failed"))
            bits.append(f"{ok}/{l['total']}")
        tail = []
        if l.get("duration") not in (None, ""):
            tail.append(fmt_dur(l["duration"]))
        if l.get("log"):
            tail.append(f"log: {l['log']}")
        if tail:
            bits.append(f"（{'，'.join(tail)}）")
        lines.append(" ".join(bits))

    if failures:
        lines.append("## 失败明细")
        for i, f in enumerate(failures, 1):
            tag = f"[{f.get('layer', '—')}] " if f.get("layer") else ""
            detail = f"：{f['detail']}" if f.get("detail") else ""
            evidence = f"｜证据 {str(f['evidence']).replace(',', '、')}" if f.get("evidence") else ""
            lines.append(f"{i}. {tag}{f.get('case', '?')}{detail}{evidence}")

    if events:
        lines.append("## 环境事件")
        lines += [f"- {e}" for e in events]
    if notes:
        lines.append("## 遗留与建议")
        lines += [f"- {n}" for n in notes]
    return "\n".join(lines) + "\n", any_fail


def cmd_report(args: argparse.Namespace) -> int:
    report, failed = render_report(
        [parse_kv(x) for x in args.layer],
        [parse_kv(x) for x in args.failure],
        args.event,
        args.note,
        engine=args.engine, mode=args.mode, project=args.project,
        duration_sec=args.duration_sec, sha=args.sha, branch=args.branch,
    )
    if args.out:
        path = Path(args.out)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(report, encoding="utf-8")
        print(f"报告: {path}｜结论: {'不通过' if failed else '通过'}")
    else:
        sys.stdout.write(report)
    return 1 if failed else 0


# ───────────────────────── 脚手架 ─────────────────────────

PRESETS = {
    "P-A": {"title": "Web 前端", "deps": False, "static": True, "native": False},
    "P-B": {"title": "Web 后端 / API", "deps": True, "static": False, "native": False},
    "P-C": {"title": "全栈 Web", "deps": True, "static": True, "native": False},
    "P-D1": {"title": "Web 游戏", "deps": False, "static": True, "native": False},
    "P-D2": {"title": "引擎原生游戏", "deps": False, "static": False, "native": True},
}

RUN_PY_HEAD = '''#!/usr/bin/env python3
"""qa 一键验收 —— 预设 __PRESET__（__TITLE__）

由 qa-test 脚手架生成；栈特定命令见各 TODO(qa) 标记。
用法: python qa/run.py [--keep]    --keep: 保留现场不拆除（排查用）

运行期目录：qa/reports/<run>/ 归档（报告+日志+留证）；qa/.run/ 工作区
（框架原始输出、last-run.json），每次运行清空只留最近一次。
"""
from __future__ import annotations

import atexit
import contextlib
import json
import shutil
import subprocess
import sys
import time
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
RUN_DIR = QA / ".run"                   # 运行期工作区（git 忽略，只留最近一次）
KEEP = "--keep" in sys.argv


def ensure_ignored(d: Path) -> None:
    """确保目录内嵌 .gitignore 存在——报告与运行数据仅作本地历史，绝不进版本库。"""
    gi = d / ".gitignore"
    if not gi.is_file():
        gi.write_text("*\\n!.gitignore\\n", encoding="utf-8")


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


def sh(cmd: list, log_name: str) -> bool:
    """跑外部命令，输出整体落日志；返回是否成功。"""
    with open(LOGS / (log_name + ".log"), "wb") as f:
        return subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT).returncode == 0


def run_layer(name: str, cmd: list) -> bool:
    """跑一层：输出落 logs/<层名>.log，结果与耗时累积进 LAYERS。"""
    t0 = time.monotonic()
    ok = sh(cmd, name)
    LAYERS.append({
        "name": name,
        "cmd": " ".join(str(c) for c in cmd),
        "pass": ok,
        "duration": int(time.monotonic() - t0),
        "log": f"logs/{name}.log",
    })
    return ok


PY = sys.executable if sys.executable.lower().endswith(("python", "python3", "python.exe")) else "python3"


def todo_layer(name: str, msg: str) -> bool:
    """TODO 占位层：直接记失败，逼着填真命令（不依赖任何解释器）。"""
    LAYERS.append({"name": name, "cmd": f"TODO(qa): {msg}", "pass": False, "duration": 0, "log": ""})
    print(f"TODO(qa): {msg}（层「{name}」未实现）", file=sys.stderr)
    return False


def render_report() -> None:
    """汇总渲染报告、记录 last-run 并退出；退出码即验收结论。"""
    out = REPORTS / "report.md"
    text, failed = qa_tool.render_report(
        LAYERS, FAILURES, EVENTS, notes=[],
__REPORT_ARGS__    )
    out.write_text(text, encoding="utf-8")
    sha, branch, _ = qa_tool.git_baseline()
    (RUN_DIR / "last-run.json").write_text(json.dumps({
        "run_id": RUN_ID,
        "verdict": "不通过" if failed else "通过",
        "sha": sha,
        "branch": branch,
        "finished": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "report": str(out.relative_to(ROOT)),
    }, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"报告: {out}｜结论: {'不通过' if failed else '通过'}")
    sys.exit(1 if failed else 0)

'''

RUN_PY_COMPOSE = RUN_PY_HEAD + '''ENGINE = "__ENGINE__"
PROJECT = "__PROJECT__"
COMPOSE = [ENGINE, "compose", "-f", str(QA / "env" / "compose.yaml"), "-p", PROJECT]


def cleanup() -> None:
    if KEEP:
        print(f"[qa] 保留现场: {' '.join(COMPOSE)} ps")
        print(f"[qa] 手动拆除: {' '.join(COMPOSE)} down -v")
        return
    sh(COMPOSE + ["down", "-v"], "teardown")
    with open(LOGS / "teardown.log", "a", encoding="utf-8") as f, \\
            contextlib.redirect_stdout(f), contextlib.redirect_stderr(f):
        qa_tool.leftovers(PROJECT, ENGINE)


atexit.register(cleanup)

# ── 1. 环境 ────────────────────────────────────────────────
sh(COMPOSE + ["up", "-d", "--wait"], "env")

# ── 2. 探活门禁 ────────────────────────────────────────────
# TODO(qa): 按实际依赖补全目标（db/缓存/静态服务/应用就绪接口）
if not qa_tool.wait_healthy(
    tcp=__WAIT_TCP__,
    http=__WAIT_HTTP__,
    timeout=120,
):
    EVENTS.append("探活失败,环境未就绪")
    render_report()

# ── 3. 种子（可选）────────────────────────────────────────
# TODO(qa): 需要种子时启用（幂等可重放）
# subprocess.run([PY, str(QA / "env" / "seed" / "seed.py")], check=True)

# ── 4. 分层执行 ────────────────────────────────────────────
# TODO(qa): 替换为产物化构建命令（如 ["cargo", "build", "--release"]，与生产同一构建管线）
if not todo_layer("构建", "产物化构建命令"):
    EVENTS.append("构建失败,后续层跳过")
    render_report()

# TODO(qa): 集成层——真实依赖下的集成/API 测试
todo_layer("集成", "集成层命令")
# 有计数时补写: LAYERS.append({"name": "集成", "cmd": "go test -tags integration ./...",
#   "pass": False, "total": 42, "failed": 2, "duration": 65, "log": "logs/集成.log"})

# TODO(qa): e2e 层——Playwright UI 旅程 / API 旅程；失败用例逐条补 FAILURES
#   测试框架的原始输出（截图/trace/test-results）指到 qa/.run/（每次运行清空）；
#   要留档的证据拷进 qa/reports/<run>/（如 playwright: --output qa/.run/test-results）
todo_layer("e2e", "e2e 命令")
# FAILURES.append({"layer": "e2e", "case": "登录·记住我",
#   "detail": "期望持久会话,实际刷新即登出", "evidence": "logs/e2e.log:L120,shots/login.png"})

render_report()
'''

RUN_PY_NATIVE = RUN_PY_HEAD + '''

# ── 分层执行 ────────────────────────────────────────────────
# TODO(qa): 产物化构建——引擎导出模板 / release 打包
if not todo_layer("构建", "产物化构建命令"):
    EVENTS.append("构建失败,后续层跳过")
    render_report()

# TODO(qa): 引擎测试——GUT/GdUnit4 --headless / Unity -batchmode -runTests / Unreal Automation
todo_layer("引擎测试", "引擎测试命令")
# 有计数时补写: LAYERS.append({"name": "引擎测试", "cmd": "godot --headless ...",
#   "pass": False, "total": 20, "failed": 1, "duration": 40, "log": "logs/引擎测试.log"})
# FAILURES.append({"layer": "引擎测试", "case": "...", "detail": "...", "evidence": "..."})

render_report()
'''

APP_DOCKERFILE = """# qa 服务本体镜像（骨架）——可选，替代宿主机跑 release 产物
# 生产已有 Dockerfile 时删除本文件，compose 改为派生引用生产定义（同构优先）。
# 缓存语义: docker 层缓存保证产物不变不重建；系统依赖层放前面，产物层放后面。
FROM debian:bookworm-slim           # TODO(qa): 基础镜像对齐生产运行环境，版本钉死

# 系统依赖层（变化少 → 缓存命中）
# RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \\
#     && rm -rf /var/lib/apt/lists/*

# 产物层（产物由 run.py 构建层产出；产物变化才重建本层与之后的所有层）
# COPY target/release/app /usr/local/bin/app        # TODO(qa): 指向 release 产物
# COPY config /etc/app                              # TODO(qa): 运行所需配置（如有）

# ENV APP_ENV=qa
# EXPOSE 8000                                       # TODO(qa): 对齐 compose 映射的容器内端口
# ENTRYPOINT ["/usr/local/bin/app"]                 # TODO(qa): 启动命令；配置经 .env.qa 注入
"""

CADDYFILE = """:80 {
  root * /srv
  try_files {path} /index.html
  file_server
}
"""

COMPOSE_STATIC = """name: __PROJECT__
networks:
  qa-net: {}

services:
  static:
    image: caddy:2.10
    volumes:
      - ../dist:/srv:ro          # TODO(qa): 指向前端/游戏构建产物目录
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
    ports: ["__STATIC_PORT__:80"]
    healthcheck:
      test: ["CMD-SHELL", "wget -q -O /dev/null http://localhost:80/ || exit 1"]
      interval: 3s
      retries: 10
    networks: [qa-net]
"""

COMPOSE_DEPS = """name: __PROJECT__
# 骨架起步——仓库有生产 compose / Dockerfile / k8s 清单时改为派生（同镜像同拓扑，只改 qa 参数）
networks:
  qa-net: {}

services:
  db:
    image: postgres:17            # TODO(qa): 换成项目真实依赖；版本与生产一致，钉死不用 latest
    env_file: [.env.qa]
    ports: ["__DB_PORT__:5432"]
    volumes: ["__PROJECT__-db:/var/lib/postgresql/data"]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $${POSTGRES_USER}"]
      interval: 3s
      retries: 10
    networks: [qa-net]

  cache:
    image: redis:7                # TODO(qa): 版本同生产；不需要就删；其余依赖照此补（healthcheck 必配）
    ports: ["__CACHE_PORT__:6379"]
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 3s
      retries: 10
    networks: [qa-net]

  # ── 服务本体容器化（可选，替代宿主机跑产物）─────────────────
  # 启用: 填好 ../Dockerfile → 取消本段注释 → 探活目标改为容器映射端口
  # 缓存: docker 层缓存保证产物不变不重建（"只构建一次"）；镜像缺失时 compose up 自动构建
  # app:
  #   build: {context: .., dockerfile: qa/env/Dockerfile}
  #   image: __PROJECT__-app:qa
  #   pull_policy: never
  #   env_file: [.env.qa]
  #   ports: ["__APP_PORT__:8000"]   # 容器内端口对齐 Dockerfile 的 EXPOSE
  #   depends_on:
  #     db: {condition: service_healthy}
  #   networks: [qa-net]
"""

COMPOSE_FULL = """name: __PROJECT__
networks:
  qa-net: {}

services:
  db:
    image: postgres:17            # TODO(qa): 换成项目真实依赖；版本与生产一致，钉死不用 latest
    env_file: [.env.qa]
    ports: ["__DB_PORT__:5432"]
    volumes: ["__PROJECT__-db:/var/lib/postgresql/data"]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $${POSTGRES_USER}"]
      interval: 3s
      retries: 10
    networks: [qa-net]

  static:
    image: caddy:2.10
    volumes:
      - ../dist:/srv:ro          # TODO(qa): 指向前端构建产物目录
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
    ports: ["__STATIC_PORT__:80"]
    healthcheck:
      test: ["CMD-SHELL", "wget -q -O /dev/null http://localhost:80/ || exit 1"]
      interval: 3s
      retries: 10
    networks: [qa-net]

  # ── 服务本体容器化（可选，替代宿主机跑产物）─────────────────
  # 启用: 填好 ../Dockerfile → 取消本段注释 → 探活目标改为容器映射端口
  # 缓存: docker 层缓存保证产物不变不重建（"只构建一次"）；镜像缺失时 compose up 自动构建
  # app:
  #   build: {context: .., dockerfile: qa/env/Dockerfile}
  #   image: __PROJECT__-app:qa
  #   pull_policy: never
  #   env_file: [.env.qa]
  #   ports: ["__APP_PORT__:8000"]   # 容器内端口对齐 Dockerfile 的 EXPOSE
  #   depends_on:
  #     db: {condition: service_healthy}
  #   networks: [qa-net]
"""

ENV_QA = """# qa 验收环境专用：只放本地占位值，禁放真实外部凭据（隔离纪律）
__CRED_LINES__
# 端口登记（compose 映射与宿主机服务监听保持一致）
__PORT_LINES__
"""

ENV_CREDS = """# 容器凭据（compose env_file 引用）
POSTGRES_USER=qa
POSTGRES_PASSWORD=qa_local_only
POSTGRES_DB=qa_app
"""

SEED_PY = '''#!/usr/bin/env python3
"""TODO(qa): 种子数据——幂等可重放（重复执行结果一致），覆盖核心场景。"""
import sys

print("TODO(qa): 实现种子注入")
sys.exit(1)
'''

README_TMPL = """# QA 验收环境

预设 __PRESET__（__TITLE__）。一键验收：`python qa/run.py`（unix 也可 `./qa/run.py`；
`--keep` 保留现场排查）。报告落 `qa/reports/<run>/report.md`，run.py 退出码即验收结论，
CI 可直接复用。目标是上线级验收：99% 情况下本环境通过即可正式上线（豁免场景见报告）。

## 验收流程

1. `qa/env/compose.yaml` 起验收环境（命名空间 `__PROJECT__`，qa 专属端口）
2. `qa/lib/qa_tool.py` 探活门禁：全部健康才进套件
3. 分层执行：构建 → 集成 → e2e（栈命令填在 run.py 各 TODO(qa) 处）
4. 渲染报告，环境拆除（`down -v` + 残留核查）

## 运行期目录与提交边界

- `qa/reports/<run>/` 归档（报告 + 日志 + 留证）；`qa/.run/` 工作区（框架原始输出、
  last-run.json），每次运行清空只留最近一次——两者**仅作本地运行历史，不进版本库**，
  由目录内嵌 `.gitignore` 忽略（run.py 每次运行自动补齐 keeper，防误删漏提交），
  不动仓库根 `.gitignore`
- 提交：`run.py` / `lib/` / `env/`（`.env.qa` 只放本地占位值）/ 本 README；
  忽略：`reports/`、`.run/`

## 覆盖清单（功能 → e2e 用例映射，持续维护）

目标是绝大多数功能都有旅程用例；每次验收顺手补缺口，新功能进清单才算了账。

| 旅程 | 用例 | 状态 |
|---|---|---|
| TODO(qa): 例：注册 → 登录 → 改密 | e2e/auth.spec.ts | TODO |

## 环境边界（隔离纪律）

- 一切资源进 qa 命名空间：容器 / 卷 / 网络一律 `__PROJECT__` 前缀，绝不复用既有环境
- 端口走 qa 专属区间：__PORTS__
- 凭据只读 `qa/env/.env.qa`，禁放真实外部凭据；拆除必须 `down -v`
- 种子幂等重放，不靠残留数据；依赖版本与生产一致（同构）
__DOCKERFILE_LINE__

## 工具（vendor 自 qa-test skill 单文件 qa_tool.py；`qa_tool.py scaffold --refresh-libs` 更新）

- `qa/lib/qa_tool.py`：check / probe / port-free / wait / leftovers / report / scaffold
"""

GITIGNORE = """*
!.gitignore
"""


def sanitize_repo(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-") or "repo"


def pick_ports(count: int, forced: list) -> list:
    if len(forced) >= count:
        return forced[:count]
    picked = list(forced)
    for p in range(18000, 18101):
        if len(picked) >= count:
            break
        if p not in picked and port_is_free(p):
            picked.append(p)
    if len(picked) < count:
        raise SystemExit(f"qa 端口区间空闲端口不足: 需 {count} 个")
    return picked


def py_list(items: list) -> str:
    return "[" + ", ".join(f'"{i}"' for i in items) + "]"


def write_file(path: Path, content: str, force: bool, root: Path, executable: bool = False) -> str:
    rel = path.relative_to(root)
    if path.exists() and path.name == ".env.qa":
        return f"skip  {rel}（.env.qa 永不覆盖）"
    if path.exists() and not force:
        return f"skip  {rel}（已存在）"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if executable:
        path.chmod(0o755)
    return f"+     {rel}"


def scaffold(args: argparse.Namespace) -> int:
    root = Path(args.dir).resolve()
    if not root.is_dir():
        raise SystemExit(f"目标目录不存在: {root}")
    repo = sanitize_repo(root.name)
    project = f"qa-{repo}"
    preset = PRESETS[args.preset]

    qa = root / "qa"
    tool_src = TOOL_FILE.read_text(encoding="utf-8")
    if args.refresh_libs:
        print(write_file(qa / "lib" / "qa_tool.py", tool_src, force=True, root=root))
        return 0

    # 端口分配：依赖（db、cache）→ 应用（宿主机服务）→ 静态服务
    need = (
        (2 if preset["deps"] else 0)
        + (1 if not (preset["native"] or preset["static"]) else 0)
        + (1 if preset["static"] else 0)
    )
    forced = [int(p) for p in args.ports.split(",")] if args.ports else []
    ports = pick_ports(max(need, len(forced)), forced)
    db_port = ports[0] if preset["deps"] else None
    cache_port = ports[1] if preset["deps"] else None
    app_port = ports[2] if preset["deps"] else (ports[0] if need and not preset["static"] else None)
    static_port = ports[-1] if preset["static"] else None

    port_lines, port_names = [], []
    if db_port:
        port_lines.append(f"QA_DB_PORT={db_port}")
        port_names.append(f"db={db_port}")
    if cache_port:
        port_lines.append(f"QA_CACHE_PORT={cache_port}")
        port_names.append(f"cache={cache_port}")
    if app_port:
        port_lines.append(f"QA_APP_PORT={app_port}")
        port_names.append(f"app={app_port}")
    if static_port:
        port_lines.append(f"QA_STATIC_PORT={static_port}")
        port_names.append(f"static={static_port}")

    tcp_targets = [f"127.0.0.1:{db_port}"] if db_port else []
    http_targets = [f"http://127.0.0.1:{static_port}/"] if static_port else []
    if not tcp_targets and not http_targets:
        tcp_targets = ["127.0.0.1:18000"]

    results = [write_file(qa / "lib" / "qa_tool.py", tool_src, force=args.force, root=root)]

    run_py = RUN_PY_NATIVE if preset["native"] else RUN_PY_COMPOSE
    run_py = run_py.replace("__PRESET__", args.preset).replace("__TITLE__", preset["title"])
    run_py = run_py.replace("__ENGINE__", args.engine).replace("__PROJECT__", project)
    run_py = run_py.replace("__WAIT_TCP__", py_list(tcp_targets))
    run_py = run_py.replace("__WAIT_HTTP__", py_list(http_targets))
    report_args = (
        '        engine=ENGINE, mode="compose", project=PROJECT,\n'
        if not preset["native"]
        else '        mode="native",\n'
    )
    run_py = run_py.replace("__REPORT_ARGS__", report_args)
    results.append(write_file(qa / "run.py", run_py, force=args.force, root=root, executable=True))

    if not preset["native"]:
        compose = (
            COMPOSE_FULL if preset["deps"] and preset["static"]
            else COMPOSE_DEPS if preset["deps"]
            else COMPOSE_STATIC
        )
        compose = compose.replace("__PROJECT__", project)
        compose = compose.replace("__DB_PORT__", str(db_port or ""))
        compose = compose.replace("__CACHE_PORT__", str(cache_port or ""))
        compose = compose.replace("__STATIC_PORT__", str(static_port or ""))
        compose = compose.replace("__APP_PORT__", str(app_port or 18010))
        results.append(write_file(qa / "env" / "compose.yaml", compose, force=args.force, root=root))
        if preset["static"]:
            results.append(write_file(qa / "env" / "Caddyfile", CADDYFILE, force=args.force, root=root))
        if preset["deps"]:
            results.append(write_file(qa / "env" / "Dockerfile", APP_DOCKERFILE, force=args.force, root=root))
        env_qa = ENV_QA.replace("__CRED_LINES__", ENV_CREDS if preset["deps"] else "#（本预设无容器依赖凭据）")
        env_qa = env_qa.replace("__PORT_LINES__", "\n".join(port_lines))
        results.append(write_file(qa / "env" / ".env.qa", env_qa, force=False, root=root))  # 永不覆盖

    results.append(write_file(qa / "env" / "seed" / "seed.py", SEED_PY, force=args.force, root=root, executable=True))
    results.append(write_file(qa / "reports" / ".gitignore", GITIGNORE, force=args.force, root=root))
    results.append(write_file(qa / ".run" / ".gitignore", GITIGNORE, force=args.force, root=root))

    readme = README_TMPL.replace("__PRESET__", args.preset).replace("__TITLE__", preset["title"])
    readme = readme.replace("__PROJECT__", project)
    readme = readme.replace("__PORTS__", "、".join(port_names) if port_names else "—（native 预设无容器端口）")
    readme = readme.replace(
        "__DOCKERFILE_LINE__",
        "- 服务本体可选容器化：`qa/env/Dockerfile` 骨架（填好后启用 compose 里注释的 app 服务；"
        "层缓存保证产物不变不重建）" if preset["deps"] else "",
    )
    results.append(write_file(qa / "README.md", readme, force=args.force, root=root))

    todo_lines = [
        "  1. qa/env/compose.yaml: 把示例依赖换成项目真实依赖（版本同生产 + healthcheck）",
        "  2. qa/run.py: 填 TODO(qa)——构建 / 集成 / e2e 命令与探活目标",
    ]
    if preset["deps"]:
        todo_lines.append(
            "  3.（可选）qa/env/Dockerfile: 服务本体容器化骨架，填好后启用 compose 里注释的 app 服务"
        )
    todo_lines += [
        f"  {len(todo_lines) + 1}. qa/env/seed/seed.py: 幂等种子（需要时）",
        f"  {len(todo_lines) + 2}. 试跑 python qa/run.py，全绿后把 qa/ 提交进仓库",
    ]
    print("\n".join(r for r in results if r))
    print(
        f"""
预设 {args.preset}｜命名空间 {project}｜引擎 {args.engine}｜端口 {'、'.join(port_names) or '—'}
下一步 TODO:
""" + "\n".join(todo_lines)
    )
    return 0


# ───────────────────────── CLI 装配 ─────────────────────────


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("check", help="分流检测: READY / HALF N / MISSING")
    p.add_argument("--dir", default=".", help="目标仓库根")
    p.set_defaults(fn=cmd_check)

    p = sub.add_parser("probe", help="摸清现场: 形态/测试层/环境设施/引擎")
    p.add_argument("--dir", default=".", help="目标仓库根")
    p.set_defaults(fn=cmd_probe)

    p = sub.add_parser("port-free", help="挑选 qa 区间空闲端口")
    p.add_argument("--start", type=int, default=18000)
    p.add_argument("--end", type=int, default=18100)
    p.add_argument("--count", type=int, default=1)
    p.add_argument("--sep", default=" ", help="输出分隔符(默认空格)")
    p.set_defaults(fn=cmd_port_free)

    p = sub.add_parser("wait", help="探活等待: 全部目标健康才退出 0")
    p.add_argument("--tcp", action="append", metavar="HOST:PORT")
    p.add_argument("--http", action="append", metavar="URL")
    p.add_argument("--timeout", type=int, default=120)
    p.add_argument("--interval", type=int, default=2)
    p.set_defaults(fn=cmd_wait)

    p = sub.add_parser("leftovers", help="核查/清理 qa 命名空间残留资源")
    p.add_argument("--project", required=True)
    p.add_argument("--engine", choices=["docker", "podman"], default="docker")
    p.add_argument("--prune", action="store_true", help="发现残留即清理(容器 rm -f / 卷 / 网络)")
    p.set_defaults(fn=cmd_leftovers)

    p = sub.add_parser("report", help="渲染验收报告(退出码即结论)")
    p.add_argument("--out", help="报告输出路径(缺省打印到 stdout)")
    p.add_argument("--engine", help="docker / podman / local")
    p.add_argument("--mode", help="compose / native / local")
    p.add_argument("--project", help="qa 命名空间, 如 qa-foo")
    p.add_argument("--layer", action="append", default=[], metavar="K=V|K=V")
    p.add_argument("--failure", action="append", default=[], metavar="K=V|K=V")
    p.add_argument("--event", action="append", default=[])
    p.add_argument("--note", action="append", default=[])
    p.add_argument("--sha", help="覆盖基线 SHA(缺省自动取 git)")
    p.add_argument("--branch", help="覆盖基线分支")
    p.add_argument("--duration-sec", type=int, help="总时长(缺省取分层求和)")
    p.set_defaults(fn=cmd_report)

    p = sub.add_parser("scaffold", help="按预设生成 qa/ 骨架(生成物纯 Python)")
    p.add_argument("--preset", choices=sorted(PRESETS), help="预设编号")
    p.add_argument("--dir", default=".", help="目标仓库根(默认当前目录)")
    p.add_argument("--engine", choices=["docker", "podman"], default="docker")
    p.add_argument("--ports", help="逗号分隔端口, 按 依赖→应用→静态 顺序分配")
    p.add_argument("--force", action="store_true", help="覆盖已存在的生成物(.env.qa 除外)")
    p.add_argument("--refresh-libs", action="store_true", help="只更新 qa/lib/qa_tool.py")
    p.add_argument("--list-presets", action="store_true")
    p.set_defaults(fn=scaffold)

    args = parser.parse_args()
    if args.cmd == "scaffold" and args.list_presets:
        for k, v in PRESETS.items():
            print(f"{k}  {v['title']}")
        return 0
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
