"""
Python wrapper classes for the tablec native library.
"""

from typing import List, Dict, Any, Optional

try:
    import tablec._native as native
    _native_available = True
except (ImportError, ModuleNotFoundError):
    _native_available = False


class ValidationError(Exception):
    """Raised when table validation fails."""
    def __init__(self, errors: List[str]):
        self.errors = errors
        super().__init__("Validation failed: " + "; ".join(errors))


class Table:
    """Represents a single table with data and schema."""

    def __init__(self, name: str, data: List[Dict[str, Any]],
                 fields: Optional[List[Dict[str, Any]]] = None,
                 constraints: Optional[List[Dict[str, Any]]] = None):
        self.name = name
        self.data = data
        self.fields = fields or []
        self.constraints = constraints or []

    def __iter__(self):
        """Iterate over rows in the table."""
        return iter(self.data)

    def __len__(self):
        """Return the number of rows."""
        return len(self.data)

    def __getitem__(self, index: int):
        """Get a row by index."""
        return self.data[index]

    def __repr__(self):
        return f"Table(name='{self.name}', rows={len(self.data)})"

    def to_dict(self) -> Dict[str, Any]:
        """Convert table to dictionary representation."""
        result = {"name": self.name, "data": self.data}
        if self.fields:
            result["fields"] = self.fields
        if self.constraints:
            result["constraints"] = self.constraints
        return result


class Project:
    """Represents a project containing multiple tables."""

    def __init__(self, tables: List[Table]):
        self.tables = {table.name: table for table in tables}

    @classmethod
    def load(cls, path: str) -> "Project":
        """Load a project from an Excel file."""
        import json
        import tempfile
        import os

        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
            temp_path = f.name

        try:
            native.build(path, temp_path, "json")
            with open(temp_path, 'r') as f:
                data = json.load(f)

            tables = []
            for table_data in data:
                table = Table(
                    name=table_data.get("name", ""),
                    data=table_data.get("data", []),
                    fields=table_data.get("fields"),
                    constraints=table_data.get("constraints")
                )
                tables.append(table)

            return cls(tables)
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)

    def __getitem__(self, name: str) -> Table:
        """Get a table by name."""
        if name not in self.tables:
            raise KeyError(f"Table '{name}' not found")
        return self.tables[name]

    def __contains__(self, name: str) -> bool:
        """Check if a table exists."""
        return name in self.tables

    def __iter__(self):
        """Iterate over table names."""
        return iter(self.tables)

    def __len__(self):
        """Return the number of tables."""
        return len(self.tables)

    def __repr__(self):
        return f"Project(tables={list(self.tables.keys())})"

    def validate(self) -> Optional[List[str]]:
        """Validate all tables in the project.

        Returns:
            List of error messages if validation fails, None otherwise.
        """
        try:
            native.check(self._get_source_path())
            return None
        except Exception as e:
            errors = [str(e)]
            return errors

    def _get_source_path(self) -> str:
        """Get the source file path (placeholder for future implementation)."""
        raise NotImplementedError("Source path tracking not implemented yet")

    def export(self, output: str, format: str = "json") -> None:
        """Export the project to a file.

        Args:
            output: Output file path.
            format: Export format ('json' or 'msgpack').
        """
        if format not in ["json", "msgpack"]:
            raise ValueError(f"Unsupported format '{format}'")
        native.build(self._get_source_path(), output, format)

    def to_dict(self) -> Dict[str, Any]:
        """Convert project to dictionary representation."""
        return {name: table.to_dict() for name, table in self.tables.items()}
