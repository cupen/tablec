"""
tablec - Table Compiler for Game Development

A Python wrapper around the tablec Rust library for compiling Excel/CSV/JSON
files into structured data formats like JSON and MessagePack.
"""

import sys

# Try to import native modules, but handle gracefully if not compiled
try:
    from tablec._native import build, check
    _native_available = True
except (ImportError, ModuleNotFoundError):
    _native_available = False

    def build(*args, **kwargs):
        raise RuntimeError("Native module not compiled. Use 'maturin develop' to build the Python binding.")

    def check(*args, **kwargs):
        raise RuntimeError("Native module not compiled. Use 'maturin develop' to build the Python binding.")

from tablec.core import Project, Table
from tablec.types import parse as parse_type, FieldType, ArrayType, MapType, StructType
from tablec.errors import ValidationError, ParseError, ExportError

__all__ = [
    # Native functions
    "build",
    "check",

    # Core classes
    "Project",
    "Table",

    # Type system
    "parse_type",
    "FieldType",
    "ArrayType",
    "MapType",
    "StructType",

    # Errors
    "ValidationError",
    "ParseError",
    "ExportError",
]

__version__ = "0.1.0"
