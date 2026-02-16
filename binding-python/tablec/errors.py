"""
Error handling classes for tablec.
"""

from typing import List, Dict, Any


class ValidationError(Exception):
    """Raised when table validation fails.

    Attributes:
        errors: List of detailed error messages.
    """

    def __init__(self, errors: List[Dict[str, Any]]):
        self.errors = errors
        message = "Validation failed:\n" + "\n".join(
            f"  - {err.get('message', str(err))}" for err in errors
        )
        super().__init__(message)


class ParseError(Exception):
    """Raised when parsing Excel or other data fails.

    Attributes:
        file: The file path that failed to parse.
        message: Detailed error message.
    """

    def __init__(self, file: str, message: str):
        self.file = file
        self.message = message
        super().__init__(f"Failed to parse '{file}': {message}")


class ExportError(Exception):
    """Raised when exporting data fails.

    Attributes:
        format: The export format that failed.
        message: Detailed error message.
    """

    def __init__(self, format: str, message: str):
        self.format = format
        self.message = message
        super().__init__(f"Export to '{format}' failed: {message}")
