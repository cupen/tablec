import tablec
import json
import os
import pytest
from pathlib import Path
from openpyxl import Workbook

@pytest.fixture(scope="module")
def temp_dir(tmpdir_factory):
    return tmpdir_factory.mktemp("data")

@pytest.fixture(scope="module")
def excel_file(temp_dir):
    file_path = temp_dir.join("test_data.xlsx")
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "Sheet1"
    sheet.append(["id", "name", "age"])
    sheet.append(["int", "string", "int"])
    sheet.append(["#", "#", "#"])
    sheet.append(["", "", ""]) # Field constraints
    sheet.append(["@nullable", "", ""]) # Table-level constraint row
    sheet.append([1, "Alice", 20])
    sheet.append([2, "Bob", 22])
    workbook.save(file_path)
    return file_path

def test_build_function_json(excel_file, temp_dir):
    output_file = temp_dir.join("output.json")
    tablec.build(str(excel_file), str(output_file), "json")

    with open(output_file, "r") as f:
        data = json.load(f)

    assert isinstance(data, list)
    assert len(data) == 1
    assert data[0]["name"] == "Sheet1"
    assert len(data[0]["data"]) == 2
    assert data[0]["data"][0]["name"] == "Alice"

def test_build_function_msgpack(excel_file, temp_dir):
    output_file = temp_dir.join("output.msgpack")
    tablec.build(str(excel_file), str(output_file), "msgpack")

    assert os.path.exists(output_file)
    assert os.path.getsize(output_file) > 0


def test_check_function(excel_file):
    tablec.check(str(excel_file))


def test_build_json_is_minified_by_default(excel_file, tmp_path):
    """`json` format produces single-line minified output (matches CLI default)."""
    output_file = tmp_path / "output.json"
    tablec.build(str(excel_file), str(output_file), "json")
    text = output_file.read_text()
    assert "\n" not in text, f"minified JSON should have no newlines, got: {text!r}"
    json.loads(text)


def test_build_json_pretty_has_indentation(excel_file, tmp_path):
    """`json-pretty` format produces multi-line indented output."""
    output_file = tmp_path / "output_pretty.json"
    tablec.build(str(excel_file), str(output_file), "json-pretty")
    text = output_file.read_text()
    assert text.count("\n") >= 2, f"pretty JSON should span multiple lines, got: {text!r}"
    assert any(line.startswith("    ") for line in text.splitlines()), \
        f"pretty JSON should contain indented lines, got: {text!r}"
    json.loads(text)
