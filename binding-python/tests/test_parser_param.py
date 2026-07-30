import tablec
import os
import pytest
from openpyxl import Workbook


@pytest.fixture(scope="module")
def temp_dir(tmpdir_factory):
    return tmpdir_factory.mktemp("parser_data")


@pytest.fixture(scope="module")
def parser_excel_file(temp_dir):
    file_path = temp_dir.join("test_parser_data.xlsx")
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
    workbook.save(file_path)
    return file_path


def test_build_with_parser_default(parser_excel_file, temp_dir):
    """Default parser (None) should behave like the standard parser."""
    output_file = temp_dir.join("out_default.json")
    tablec.build(str(parser_excel_file), str(output_file), "json")
    assert os.path.exists(output_file)
    assert os.path.getsize(output_file) > 0


def test_check_with_parser_explicit_standard(parser_excel_file):
    """Explicit parser='standard' should work for check()."""
    tablec.check(str(parser_excel_file), parser="standard")