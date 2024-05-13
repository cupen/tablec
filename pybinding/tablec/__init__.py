from . import _native

def build(input: str, output: str, format: str, include_fields: bool = False):
    return _native.Tablec.build(input, output, format, include_fields)

check = _native.Tablec.check
