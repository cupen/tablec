"""
Type system for tablec fields.
"""

from typing import Dict, Any, Optional, Type
from enum import Enum


class FieldType(Enum):
    """Field type enumeration."""
    Int8 = "Int8"
    Int16 = "Int16"
    Int32 = "Int32"
    Int64 = "Int64"
    UInt8 = "UInt8"
    UInt16 = "UInt16"
    UInt32 = "UInt32"
    UInt64 = "UInt64"
    Float32 = "Float32"
    Float64 = "Float64"
    String = "String"


class Type:
    """Base class for all types."""

    def is_array(self) -> bool:
        """Check if this is an array type."""
        return False

    def is_map(self) -> bool:
        """Check if this is a map type."""
        return False

    def is_struct(self) -> bool:
        """Check if this is a struct type."""
        return False

    def is_primitive(self) -> bool:
        """Check if this is a primitive type."""
        return True


class ArrayType(Type):
    """Represents an array type."""

    def __init__(self, element_type: Type):
        self.element_type = element_type

    def is_array(self) -> bool:
        return True

    def is_primitive(self) -> bool:
        return False

    def __repr__(self):
        return f"ArrayType({self.element_type})"


class MapType(Type):
    """Represents a map type."""

    def __init__(self, key_type: Type, value_type: Type):
        if key_type not in [FieldType.Int32, FieldType.Int64, FieldType.String]:
            raise ValueError("Map key must be int or string type")
        self.key_type = key_type
        self.value_type = value_type

    def is_map(self) -> bool:
        return True

    def is_primitive(self) -> bool:
        return False

    def __repr__(self):
        return f"MapType({self.key_type} -> {self.value_type})"


class StructType(Type):
    """Represents a struct type."""

    def __init__(self, fields: Dict[str, Type]):
        if len(fields) > 32:
            raise ValueError("Struct can have at most 32 fields")
        self.fields = fields

    def is_struct(self) -> bool:
        return True

    def is_primitive(self) -> bool:
        return False

    def __repr__(self):
        return f"StructType({', '.join(f'{k}: {v}' for k, v in self.fields.items())})"


def parse(type_str: str) -> Type:
    """Parse a type string into a Type object.

    Args:
        type_str: Type string (e.g., "int", "int[]", "map<string, int>", "{a:int, b:str}")

    Returns:
        Type object.
    """
    type_str = type_str.strip().lower()

    if type_str == "str":
        return FieldType.String

    if type_str == "int":
        return FieldType.Int32
    if type_str == "uint":
        return FieldType.UInt32
    if type_str == "float":
        return FieldType.Float64

    for field_type in FieldType:
        if field_type.value.lower() == type_str:
            return field_type

    if type_str.endswith("[]"):
        element_str = type_str[:-2].strip()
        return ArrayType(parse(element_str))

    if type_str.startswith("map<") and type_str.endswith(">"):
        inner = type_str[4:-1].strip()
        if "," not in inner:
            raise ValueError(f"Invalid map type: {type_str}")
        key_str, value_str = inner.split(",", 1)
        return MapType(parse(key_str.strip()), parse(value_str.strip()))

    if type_str.startswith("{") and type_str.endswith("}"):
        inner = type_str[1:-1].strip()
        if not inner:
            return StructType({})

        fields = {}
        parts = [p.strip() for p in inner.split(",")]
        for part in parts:
            if ":" not in part:
                raise ValueError(f"Invalid struct field: {part}")
            name, type_part = part.split(":", 1)
            fields[name.strip()] = parse(type_part.strip().strip())

        return StructType(fields)

    if type_str.startswith("struct{") and type_str.endswith("}"):
        inner = type_str[6:-1].strip()
        if not inner:
            return StructType({})

        fields = {}
        parts = [p.strip() for p in inner.split(",")]
        for part in parts:
            if ":" not in part:
                raise ValueError(f"Invalid struct field: {part}")
            name, type_part = part.split(":", 1)
            fields[name.strip()] = parse(type_part.strip().strip())

        return StructType(fields)

    raise ValueError(f"Unknown type: {type_str}")
