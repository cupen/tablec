import tablec
import json
import pytest
from openpyxl import Workbook

@pytest.fixture(scope="module")
def temp_dir(tmpdir_factory):
    return tmpdir_factory.mktemp("data")

@pytest.fixture(scope="module")
def unique_excel_file(temp_dir):
    file_path = temp_dir.join("test_unique_data.xlsx")
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "UniqueSheet"
    sheet.append(["id", "name", "city"])
    sheet.append(["int", "string", "string"])
    sheet.append(["#", "#", "#"])
    sheet.append(["@unique(name, city)", "", ""]) # Composite unique constraint on id, name, city
    sheet.append([1, "Alice", "New York"])
    sheet.append([2, "Bob", "London"])
    sheet.append([3, "Alice", "Paris"])
    workbook.save(file_path)
    return file_path

@pytest.fixture(scope="module")
def duplicate_unique_excel_file(temp_dir):
    file_path = temp_dir.join("test_duplicate_unique_data.xlsx")
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "DuplicateUniqueSheet"
    sheet.append(["id", "name", "city"])
    sheet.append(["int", "string", "string"])
    sheet.append(["#", "#", "#"])
    sheet.append(["@unique(name, city)", "", ""]) # Composite unique constraint on id, name, city
    sheet.append([1, "Alice", "New York"])
    sheet.append([2, "Bob", "London"])
    sheet.append([3, "Alice", "New York"]) # Duplicate combination
    workbook.save(file_path)
    return file_path

def test_composite_unique_constraint_ok(unique_excel_file):
    # This should pass without errors
    tablec.check(str(unique_excel_file))

def test_composite_unique_constraint_fail(duplicate_unique_excel_file):
    # This should raise an exception due to duplicate combination
    with pytest.raises(Exception) as excinfo:
        tablec.check(str(duplicate_unique_excel_file))
    assert "Duplicate combination found for fields" in str(excinfo.value)
