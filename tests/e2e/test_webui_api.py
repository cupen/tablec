"""e2e · webui API 旅程（真实 HTTP 请求序列，断言只走公共 API）。

由 qa/run.py 驱动：QA_WEBUI_URL 指向已探活的服务器，QA_FIXTURE_REPO 指向夹具
仓库（fixtures.py 构建：a 干净 / b modified / c untracked / d_err 含越界值）。

注意：preview/sheets/parsed_preview 的 path 参数使用 /api/files 条目返回的
绝对路径——与前端行为一致（相对路径下 cell diff 会静默缺失）。
"""
from __future__ import annotations

import json
import os
import subprocess
import urllib.error
import urllib.request
from pathlib import Path

import pytest

URL = os.environ.get("QA_WEBUI_URL")
FIX = os.environ.get("QA_FIXTURE_REPO")
BIN = os.environ.get("QA_TABLEC_BIN")
pytestmark = pytest.mark.skipif(
    not (URL and FIX), reason="QA_WEBUI_URL/QA_FIXTURE_REPO 未设置（由 qa/run.py 驱动）"
)


def api(path: str, payload: dict | None = None) -> tuple[int, dict | str]:
    req = urllib.request.Request(
        URL + path,
        data=json.dumps(payload).encode() if payload is not None else None,
        headers={"Content-Type": "application/json"} if payload is not None else {},
        method="POST" if payload is not None else "GET",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            body = resp.read().decode()
            return resp.status, (json.loads(body) if body.startswith(("{", "[")) else body)
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        try:
            return e.code, json.loads(body)
        except json.JSONDecodeError:
            return e.code, body


def file_entries() -> dict[str, dict]:
    status, files = api("/api/files")
    assert status == 200, files
    raw = files["files"] if isinstance(files, dict) else files
    return {e["rel_path"]: e for e in raw}


def test_files_git_statuses_and_modified_filter() -> None:
    entries = file_entries()
    assert entries["a.xlsx"]["status"] == "clean"
    assert entries["b.xlsx"]["status"] == "modified"
    assert entries["c_untracked.xlsx"]["status"] == "untracked"
    assert entries["d_err.xlsx"]["status"] == "clean"

    status, filtered = api("/api/files?filter=modified")
    assert status == 200
    raw = filtered["files"] if isinstance(filtered, dict) else filtered
    rels = {e["rel_path"] for e in raw}
    assert {"b.xlsx", "c_untracked.xlsx"} <= rels
    assert "a.xlsx" not in rels and "d_err.xlsx" not in rels


def test_sheets_preview_parsed_preview_shapes() -> None:
    entries = file_entries()
    a_path = entries["a.xlsx"]["path"]
    b_path = entries["b.xlsx"]["path"]

    status, sheets = api(f"/api/sheets?path={a_path}")
    assert status == 200, sheets
    raw = sheets if isinstance(sheets, list) else sheets.get("sheets", [])
    assert any(s["name"] == "Item" for s in raw)

    status, pv = api(f"/api/preview?path={a_path}&sheet=Item")
    assert status == 200
    assert pv["sheet"] == "Item" and isinstance(pv["rows"], list) and pv["rows"]

    status, pp = api(f"/api/parsed_preview?path={a_path}&sheet=Item")
    assert status == 200
    assert pp["data_start_row"] == 5
    assert pp["schema"]["fields"] and pp["schema"]["fields"][0]["name"] == "id"
    assert "error_count" in pp["summary"] and "total_rows" in pp["summary"]

    # b.xlsx（worktree v2 14 行 vs HEAD v1 6 行）→ 逐格 diff 存在 added 状态
    status, ppb = api(f"/api/parsed_preview?path={b_path}&sheet=Item")
    assert status == 200
    cells = [c for row in ppb["rows"] for c in row["cells"]]
    assert cells and all("diff" in c for c in cells)
    assert any(c["diff"] == "added" for c in cells)


def test_build_api_writes_artifact_on_request(tmp_path: Path) -> None:
    status, body = api("/api/build", {"dir": str(FIX), "format": "json-pretty", "write": False})
    assert status == 200, body
    assert body["bytes"] > 0 and body["written"] is False
    preview = body["preview_first_500"].lstrip()
    assert preview.startswith("{") and '"name"' in preview[:80]

    out = tmp_path / "built.json"
    status, body = api("/api/build", {"dir": str(FIX), "format": "json", "write": True, "output_path": str(out)})
    assert status == 200 and body["written"] is True
    assert out.is_file() and json.loads(out.read_text())["name"]

    status, body = api("/api/build", {"dir": str(FIX), "format": "protobuf"})
    assert status == 400 and "json" in str(body)


def test_check_api_matches_cli_diagnostic_set() -> None:
    """POST /api/check 与 CLI `tablec check` 对同一目录报同一错误诊断集。"""
    status, body = api("/api/check", {"dir": str(FIX)})
    assert status == 200, body
    api_errors = {
        (d["code"], d["message"])
        for d in body["diagnostics"] if d["severity"] == "Error"
    }
    # d_err.xlsx 里种了 int8 越界值 → 恰好一个 ValueOutOfRange 错误
    assert any(code == "ValueOutOfRange" for code, _ in api_errors)
    assert body["sheets_checked"] >= 1 and isinstance(body["duration_ms"], int)

    if BIN:
        # spec 场景「Check matches CLI semantics」比较的是同一 input directory：
        # API 端把 dir 解析为 config 的 input_dir（FIX/data），CLI 等价物是
        # `tablec check <input_dir>`（显式 path 即扫描根，spec 明文）。
        r = subprocess.run([BIN, "check", str(Path(FIX) / "data")],
                           capture_output=True, text=True, timeout=120)
        assert r.returncode != 0
        for code, msg in api_errors:
            assert msg in r.stderr or code in r.stderr


def test_error_paths_ref_reject_validate_501_plugin_paths() -> None:
    status, body = api("/api/files?right=no-such-branch")
    assert status == 400 and "no-such-branch" in str(body)

    status, body = api("/api/validate", {"dir": str(FIX)})
    assert status == 501 and body["error"] == "not_implemented" and "todo" in body

    status, body = api("/api/build", {"dir": str(FIX), "format": "json", "plugin_paths": ["/tmp/evil.so"]})
    assert status == 400 and "CLI" in str(body)

    status, branches = api("/api/branches")
    assert status == 200
    names = branches["branches"] if isinstance(branches, dict) else branches
    assert "main" in names and names.count("main") == 1  # origin/main 去重
    assert "diverge" in names

    status, _ = api("/api/does-not-exist")
    assert status == 404
