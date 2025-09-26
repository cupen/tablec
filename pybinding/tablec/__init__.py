from . import _native

def build(input: str, output: str, format: str, include_fields: bool = False):
    return _native.Tablec.build(input, output, format, include_fields)

def example(output: str = "example.xlsx", rows: int = 10):
    return _native.Tablec.example(output, rows)

check = _native.Tablec.check
