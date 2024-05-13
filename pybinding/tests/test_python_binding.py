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
    sheet.append(["", "", ""]) # Dummy row for constraints
    sheet.append([1, "Alice", 20])
    sheet.append([2, "Bob", 22])
    workbook.save(file_path)
    return file_path

def test_build_function_with_fields(excel_file, temp_dir):
    output_file = temp_dir.join("output_with_fields.json")
    tablec.build(str(excel_file), str(output_file), "json", include_fields=True)

    with open(output_file, "r") as f:
        data = json.load(f)

    assert data == [
        {
            "name": "Sheet1",
            "fields": [
                {"name": "id", "t": "Int32", "desc": "#", "constraint": None, "tags": []},
                {"name": "name", "t": "String", "desc": "#", "constraint": None, "tags": []},
                {"name": "age", "t": "Int32", "desc": "#", "constraint": None, "tags": []},
            ],
            "data": [
                {"age": 20, "id": 1, "name": "Alice"},
                {"age": 22, "id": 2, "name": "Bob"},
            ],
            "constraints": []
        }
    ]

def test_build_function_without_fields(excel_file, temp_dir):
    output_file = temp_dir.join("output_without_fields.json")
    tablec.build(str(excel_file), str(output_file), "json", include_fields=False)

    with open(output_file, "r") as f:
        data = json.load(f)

    assert data == [
        {
            "name": "Sheet1",
            "data": [
                {"age": 20, "id": 1, "name": "Alice"},
                {"age": 22, "id": 2, "name": "Bob"},
            ],
            "constraints": []
        }
    ]

def test_check_function(excel_file):
    tablec.check(str(excel_file))
