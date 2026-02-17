
import tablec
import json
import pytest
from openpyxl import Workbook

@pytest.fixture(scope="module")
def temp_dir(tmpdir_factory):
    return tmpdir_factory.mktemp("data")

@pytest.fixture(scope="module")
def complex_excel_file(temp_dir):
    file_path = temp_dir.join("test_complex_data.xlsx")
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "ComplexSheet"
    sheet.append(["map_data", "struct_data"])
    sheet.append(["map<string,int>", "struct{id:int,name:string}"])
    sheet.append(["#", "#"])
    sheet.append(["", ""]) # Constraints
    sheet.append(["a:1,b:2", "{101, 'test_name'}"])
    workbook.save(file_path)
    return file_path

def test_complex_types(complex_excel_file, temp_dir):
    output_file = temp_dir.join("complex_output.json")
    tablec.build(str(complex_excel_file), str(output_file), "json")

    with open(output_file, "r") as f:
        data = json.load(f)

    assert len(data) == 1
    assert data[0]["name"] == "ComplexSheet"
    assert len(data[0]["data"]) == 1
    assert "map_data" in data[0]["data"][0]
    assert data[0]["data"][0]["map_data"] == {"a": 1, "b": 2}
