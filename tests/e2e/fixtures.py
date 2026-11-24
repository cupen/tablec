"""qa e2e 共享夹具：xlsx 生成与 git 夹具仓库构建。

由 qa/run.py 在起服务器前调用 build_fixture_repo()；测试套件只读
环境变量指向的现成夹具，不自行重建。

夹具仓库形态（幂等：每次整体重建）：
  fixture-repo/
  ├── tablec.toml          # [data] input_dir = "data"
  └── data/
      ├── a.xlsx           # 已提交、干净（main 上 v1，diverge 分支上是 v2）
      ├── b.xlsx           # 已提交 v1，工作区改成 v2 未提交 → modified
      ├── c_untracked.xlsx # 未跟踪 → untracked
      └── d_err.xlsx       # 已提交，内含 int8 越界值 200 → check 报 1 个错误
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

from openpyxl import Workbook

HEADER_ROWS = 5  # 标准 5 行表头：名/型/注释/字段约束/表级约束，数据从第 6 行


def _run(cmd: list, cwd: Path) -> None:
    subprocess.run(cmd, cwd=cwd, check=True,
                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT)


def write_item_sheet(path: Path, rows: list[tuple[int, str]], bad_value: int | None = None) -> None:
    """写一个标准 5 行表头的 Item 表；bad_value 在 id 列种一个 int8 越界值。"""
    wb = Workbook()
    ws = wb.active
    ws.title = "Item"
    ws.append(["id", "name"])
    ws.append(["int8", "string"])
    ws.append(["", ""])
    ws.append(["", ""])
    ws.append([""])
    data = [(i, n) for i, n in rows]
    if bad_value is not None:
        data = [(bad_value, "overflow")] + data
    for i, n in data:
        ws.append([i, n])
    wb.save(path)


def build_fixture_repo(dest: Path) -> Path:
    """在 dest 整体重建 git 夹具仓库，返回其路径。"""
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    (dest / "data").mkdir()

    (dest / "tablec.toml").write_text(
        '[data]\ninput_dir = "data"\n', encoding="utf-8"
    )
    write_item_sheet(dest / "data" / "a.xlsx", [(1, "alpha"), (2, "beta")])
    write_item_sheet(dest / "data" / "b.xlsx",
                     [(i, f"row{i}") for i in range(1, 7)])          # v1: 6 行
    write_item_sheet(dest / "data" / "d_err.xlsx",
                     [(1, "ok"), (2, "fine")], bad_value=200)
    write_item_sheet(dest / "data" / "c_untracked.xlsx", [(9, "loose")])

    _run(["git", "init", "-b", "main"], dest)
    _run(["git", "config", "user.email", "qa@example.local"], dest)
    _run(["git", "config", "user.name", "qa"], dest)
    _run(["git", "add", "tablec.toml", "data/a.xlsx", "data/b.xlsx", "data/d_err.xlsx"], dest)
    _run(["git", "commit", "-m", "fixture baseline"], dest)

    # diverge 分支：改 a.xlsx 内容后提交（main 与 diverge 的 tip 直比可见差异）
    _run(["git", "branch", "diverge"], dest)
    _run(["git", "checkout", "diverge"], dest)
    write_item_sheet(dest / "data" / "a.xlsx", [(1, "alpha-diverged"), (2, "beta"), (3, "gamma")])
    _run(["git", "add", "data/a.xlsx"], dest)
    _run(["git", "commit", "-m", "diverge a.xlsx"], dest)
    _run(["git", "checkout", "main"], dest)

    # b.xlsx 工作区改成 v2（14 行）不提交 → modified
    write_item_sheet(dest / "data" / "b.xlsx",
                     [(i, f"row{i}") for i in range(1, 15)])         # v2: 14 行
    return dest


if __name__ == "__main__":  # pragma: no cover
    import os
    target = Path(os.environ.get("QA_FIXTURE_REPO", "qa/.run/fixture-repo"))
    print(build_fixture_repo(target.resolve()))
