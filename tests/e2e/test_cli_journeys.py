"""e2e · CLI 旅程（真进程走命令行，断言落在用户可见行为上）。

由 qa/run.py 驱动：QA_TABLEC_BIN 指向已构建二进制；缺环境变量时整文件跳过。
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest
from openpyxl import Workbook

BIN = os.environ.get("QA_TABLEC_BIN")
pytestmark = pytest.mark.skipif(not BIN, reason="QA_TABLEC_BIN 未设置（由 qa/run.py 驱动）")


def run_cli(args: list, cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run([BIN, *args], cwd=cwd, capture_output=True, text=True, timeout=120)


def write_valid_xlsx(path: Path, rows: int = 3) -> None:
    wb = Workbook()
    ws = wb.active
    ws.title = "Item"
    ws.append(["id", "name"])
    ws.append(["int8", "string"])
    ws.append(["", ""])
    ws.append(["", ""])
    ws.append([""])
    for i in range(1, rows + 1):
        ws.append([i, f"row{i}"])
    wb.save(path)


def write_bad_xlsx(path: Path) -> None:
    """id 列（int8）数据行放 200 → ValueOutOfRange。"""
    wb = Workbook()
    ws = wb.active
    ws.title = "Item"
    ws.append(["id", "name"])
    ws.append(["int8", "string"])
    ws.append(["", ""])
    ws.append(["", ""])
    ws.append([""])
    ws.append([1, "ok"])
    ws.append([200, "overflow"])
    wb.save(path)


def test_build_single_file_to_json(tmp_path: Path) -> None:
    src = tmp_path / "in.xlsx"
    write_valid_xlsx(src)
    out = tmp_path / "out.json"
    r = run_cli(["build", "-i", str(src), "-o", str(out)])
    assert r.returncode == 0, r.stderr
    assert f"Exported data to {out}" in r.stdout
    data = json.loads(out.read_text())
    assert data["name"] and isinstance(data["tables"], list) and data["tables"]
    rows = data["tables"][0]["data"]
    assert len(rows) == 3


def test_build_directory_zero_config_writes_default_output(tmp_path: Path) -> None:
    data_dir = tmp_path / "nocfg"
    data_dir.mkdir()
    write_valid_xlsx(data_dir / "t.xlsx")
    r = run_cli(["build", "-i", str(data_dir)], cwd=tmp_path)
    assert r.returncode == 0, r.stderr
    default_out = tmp_path / "output" / "default.json"
    assert default_out.is_file()
    json.loads(default_out.read_text())


def test_build_msgpack_format_and_extension(tmp_path: Path) -> None:
    src_dir = tmp_path / "in"
    src_dir.mkdir()
    write_valid_xlsx(src_dir / "t.xlsx")
    # 显式 -o 原样使用，不追加扩展名
    r = run_cli(["build", "-i", str(src_dir), "-o", str(tmp_path / "out"), "--format", "msgpack"])
    assert r.returncode == 0, r.stderr
    raw = (tmp_path / "out").read_bytes()
    assert raw[:1] == b"\x93"  # msgpack fixarray(3)：project 对象头部
    # 无 -o 的目录模式：默认名带格式扩展名 output/default.msgpack
    r2 = run_cli(["build", "-i", str(src_dir), "--format", "msgpack"], cwd=tmp_path)
    assert r2.returncode == 0, r2.stderr
    out2 = tmp_path / "output" / "default.msgpack"
    assert out2.is_file() and out2.read_bytes()[:1] == b"\x93"


def test_check_exit_codes_clean_vs_error(tmp_path: Path) -> None:
    clean_dir = tmp_path / "clean"
    clean_dir.mkdir()
    write_valid_xlsx(clean_dir / "ok.xlsx")
    r = run_cli(["check", str(clean_dir)])
    assert r.returncode == 0, r.stderr
    assert "No errors found" in r.stdout

    bad_dir = tmp_path / "bad"
    bad_dir.mkdir()
    write_bad_xlsx(bad_dir / "bad.xlsx")
    r = run_cli(["check", str(bad_dir)])
    assert r.returncode != 0
    assert "ValueOutOfRange" in r.stderr
    assert "Found 1 errors." in r.stdout


def test_example_refuses_overwrite_then_force(tmp_path: Path) -> None:
    out = tmp_path / "ex.xlsx"
    r = run_cli(["example", "-o", str(out), "-r", "5"])
    assert r.returncode == 0, r.stderr
    assert out.is_file()
    r2 = run_cli(["example", "-o", str(out), "-r", "5"])
    assert r2.returncode != 0
    assert "already exists" in (r2.stderr + r2.stdout)
    r3 = run_cli(["example", "-o", str(out), "-r", "5", "--force"])
    assert r3.returncode == 0, r3.stderr


def test_check_report_writes_both_formats(tmp_path: Path) -> None:
    src = tmp_path / "in.xlsx"
    write_valid_xlsx(src)
    j = tmp_path / "r.json"
    h = tmp_path / "r.html"
    r = run_cli(["check", str(src), "--report", str(j), "--report", str(h)])
    assert r.returncode == 0, r.stderr
    assert f"Report written: {j}" in r.stdout
    assert f"Report written: {h}" in r.stdout
    report = json.loads(j.read_text())
    assert report["schema_version"] == 1 and report["summary"]["errors"] == 0
    html = h.read_text()
    assert "http://" not in html and "https://" not in html  # 自包含 HTML
    assert "</html>" in html


def test_config_discovery_cwd_wins_over_target(tmp_path: Path) -> None:
    """cwd 与目标目录各有 tablec.toml：cwd 的 [export] 生效（format/output_dir 可分辨）。"""
    cwd_dir = tmp_path / "cwdproj"
    tgt_dir = tmp_path / "tgt"
    cwd_dir.mkdir()
    (tgt_dir / "data").mkdir(parents=True)
    (cwd_dir / "tablec.toml").write_text(
        '[data]\ninput_dir = "data"\n[export]\nformat = "msgpack"\noutput_dir = "out-cwd"\n',
        encoding="utf-8",
    )
    (tgt_dir / "data" / "tablec.toml").write_text(
        '[export]\nformat = "json-pretty"\noutput_dir = "out-tgt"\n', encoding="utf-8"
    )
    write_valid_xlsx(tgt_dir / "data" / "t.xlsx")

    r = run_cli(["build", "-i", str(tgt_dir / "data")], cwd=cwd_dir)
    assert r.returncode == 0, r.stderr
    # cwd 配置胜出：msgpack 格式 + cwd 的输出目录；tgt 配置的 json-pretty/out-tgt 未生效
    out = cwd_dir / "out-cwd" / "default.msgpack"
    assert out.is_file(), f"cwd config should govern output: {r.stdout} {r.stderr}"
    assert out.read_bytes()[:1] == b"\x93"
    assert not (tgt_dir / "out-tgt").exists()


def test_unknown_parser_fails_naming_the_parser(tmp_path: Path) -> None:
    src = tmp_path / "in.xlsx"
    write_valid_xlsx(src)
    r = run_cli(["check", str(src), "--parser", "not-registered"])
    assert r.returncode != 0
    assert "not-registered" in (r.stderr + r.stdout)
