import os
import tempfile
from pathlib import Path

import pytest
import tablec
from openpyxl import Workbook


def write_minimal_xlsx(path):
    """Write a minimal tablec-compatible workbook to `path`."""
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "ParserSheet"
    sheet.append(["id", "name"])
    sheet.append(["int", "string"])
    sheet.append(["#", "#"])
    sheet.append(["", ""])  # Field constraints
    sheet.append(["@nullable", ""])  # Table-level constraint row
    sheet.append([1, "Alice"])
    sheet.append([2, "Bob"])
    workbook.save(str(path))
    return path


@pytest.fixture(scope="module")
def temp_dir(tmp_path_factory):
    return tmp_path_factory.mktemp("parser_data")


@pytest.fixture(scope="module")
def parser_excel_file(temp_dir):
    file_path = temp_dir / "test_parser_data.xlsx"
    write_minimal_xlsx(file_path)
    return file_path


def test_build_with_parser_default(parser_excel_file, temp_dir):
    """Default parser (None) should behave like the standard parser."""
    output_file = temp_dir / "out_default.json"
    tablec.build(str(parser_excel_file), str(output_file), "json")
    assert output_file.exists()
    assert output_file.stat().st_size > 0


def test_check_with_parser_explicit_standard(parser_excel_file):
    """Explicit parser='standard' should work for check()."""
    tablec.check(str(parser_excel_file), parser="standard")


def test_build_with_unknown_parser_raises_value_error():
    """Unknown parser name should raise ValueError, not panic the interpreter."""
    with tempfile.TemporaryDirectory() as tmp:
        xlsx = Path(tmp) / "in.xlsx"
        out = Path(tmp) / "out.json"
        write_minimal_xlsx(xlsx)
        with pytest.raises(ValueError, match="not registered"):
            tablec.build(str(xlsx), str(out), "json", parser="does-not-exist")
