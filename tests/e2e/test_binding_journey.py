"""e2e · Python 绑定旅程：HEAD 构建的 wheel 上走 build → check 全链路。

由 qa/run.py 驱动：QA_VENV_PY 指向装好当前 HEAD wheel 的解释器。
绑定 API 本身不能在 pytest 进程内直接 import（需要独立解释器），
因此通过子进程运行内嵌脚本并断言其输出。
"""
from __future__ import annotations

import os
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

VENV_PY = os.environ.get("QA_VENV_PY")
pytestmark = pytest.mark.skipif(not VENV_PY, reason="QA_VENV_PY 未设置（由 qa/run.py 驱动）")

SCRIPT = textwrap.dedent("""
    import json, sys
    from pathlib import Path
    import tablec

    # 纯 Python 包面：spec 列出的全部名字可导入
    from tablec import (Project, Table, parse_type, FieldType, ArrayType,
                        MapType, StructType, ValidationError, ParseError, ExportError)

    assert tablec.__version__, "version missing"
    src = Path(sys.argv[1])
    out = Path(sys.argv[2])

    # build()：合法 xlsx → JSON 落盘
    tablec.build(str(src), str(out), "json")
    data = json.loads(out.read_text())
    assert data["name"] and data["tables"], "unexpected export shape"

    # check()：合法文件 → None；违约文件 → ValueError 带诊断文本
    assert tablec.check(str(src)) is None
    try:
        tablec.check(str(sys.argv[3]))
        raise AssertionError("check should have raised on invalid workbook")
    except ValueError as e:
        assert "ValueOutOfRange" in str(e), str(e)

    # 错误路径：缺 output / 不支持的 format / 未注册 parser
    for kwargs, needle in [
        ({"input": str(src), "format": "json"}, "output"),
        ({"input": str(src), "output": str(out), "format": "yaml"}, "json"),
        ({"input": str(src), "output": str(out), "format": "json", "parser": "nope"}, "not registered"),
    ]:
        try:
            tablec.build(**kwargs)
            raise AssertionError(f"build should have raised for {kwargs}")
        except ValueError as e:
            assert needle.lower() in str(e).lower(), f"{needle!r} not in {e}"

    print("BINDING-JOURNEY-OK")
""")


def test_binding_build_check_round_trip(tmp_path: Path) -> None:
    valid = tmp_path / "valid.xlsx"
    invalid = tmp_path / "invalid.xlsx"
    _write_xlsx(valid, bad=False)
    _write_xlsx(invalid, bad=True)

    r = subprocess.run(
        [VENV_PY, "-c", SCRIPT, str(valid), str(tmp_path / "out.json"), str(invalid)],
        capture_output=True, text=True, timeout=120,
        cwd=str(Path(tmp_path)),  # 中立 cwd：避免本地纯 Python 包遮蔽已装原生模块
    )
    assert r.returncode == 0, f"stderr:\n{r.stderr}\nstdout:\n{r.stdout}"
    assert "BINDING-JOURNEY-OK" in r.stdout


def _write_xlsx(path: Path, bad: bool) -> None:
    from openpyxl import Workbook

    wb = Workbook()
    ws = wb.active
    ws.title = "Item"
    ws.append(["id", "name"])
    ws.append(["int8", "string"])
    ws.append(["", ""])
    ws.append(["", ""])
    ws.append([""])
    ws.append([1, "ok"])
    if bad:
        ws.append([200, "overflow"])
    wb.save(path)
