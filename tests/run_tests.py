#!/usr/bin/env python3
"""Test script to compile Excel files and verify output."""

import json
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Tuple

# Colors for output
RED = '\033[91m'
GREEN = '\033[92m'
YELLOW = '\033[93m'
BLUE = '\033[94m'
RESET = '\033[0m'
BOLD = '\033[1m'

class TestResult:
    """Store test result information."""
    def __init__(self, name: str, passed: bool, message: str = ""):
        self.name = name
        self.passed = passed
        self.message = message

def compile_excel_with_tablec(excel_path: Path, output_path: Path) -> Tuple[bool, str]:
    """Compile Excel file using tablec CLI."""
    try:
        result = subprocess.run(
            ["cargo", "run", "--release", "--", "build",
             "-i", str(excel_path),
             "-o", str(output_path),
             "--format", "json"],
            cwd=Path(__file__).parent.parent,
            capture_output=True,
            text=True,
            timeout=30
        )

        if result.returncode == 0:
            return True, result.stdout
        else:
            return False, f"Compilation failed: {result.stderr}"
    except subprocess.TimeoutExpired:
        return False, "Compilation timeout"
    except Exception as e:
        return False, f"Compilation error: {str(e)}"

def compare_json(actual_path: Path, expected_path: Path) -> Tuple[bool, str]:
    """Compare actual and expected JSON files."""
    try:
        with open(actual_path, 'r') as f:
            actual = json.load(f)

        with open(expected_path, 'r') as f:
            expected = json.load(f)

        if actual == expected:
            return True, "JSON output matches expected"
        else:
            # Find differences
            diff = json.dumps({
                "expected": expected,
                "actual": actual
            }, indent=2)
            return False, f"JSON mismatch:\n{diff[:500]}"  # Limit output
    except Exception as e:
        return False, f"Comparison error: {str(e)}"

def run_single_test(test_name: str, excel_dir: Path, expected_dir: Path, output_dir: Path) -> TestResult:
    """Run a single test case."""
    excel_path = excel_dir / f"{test_name}.xlsx"
    expected_path = expected_dir / f"{test_name}.json"
    output_path = output_dir / f"{test_name}.json"

    # Check if files exist
    if not excel_path.exists():
        return TestResult(test_name, False, f"Excel file not found: {excel_path}")
    if not expected_path.exists():
        return TestResult(test_name, False, f"Expected file not found: {expected_path}")

    # Compile Excel
    print(f"  Compiling {test_name}...", end=" ", flush=True)
    success, message = compile_excel_with_tablec(excel_path, output_path)
    if not success:
        print(f"{RED}✗{RESET}")
        return TestResult(test_name, False, message)

    # Compare output
    print(f"{GREEN}✓{RESET}", end=" ", flush=True)
    success, message = compare_json(output_path, expected_path)
    if not success:
        print(f"{RED}✗{RESET}")
        return TestResult(test_name, False, message)

    print(f"{GREEN}✓{RESET}")
    return TestResult(test_name, True, message)

def main():
    """Main test runner."""
    print(f"\n{BOLD}{BLUE}=== Tablec Integration Tests ==={RESET}\n")

    # Setup paths
    script_dir = Path(__file__).parent
    excel_dir = script_dir / "excel"
    expected_dir = script_dir / "expected"
    output_dir = script_dir / "output"

    # Create output directory
    output_dir.mkdir(exist_ok=True)

    # Test cases
    test_cases = [
        "basic_types",
        "array_types",
        "map_types",
        "struct_types",
        "constraints",
        "composite_types"
    ]

    # Run tests
    results: List[TestResult] = []
    for test_name in test_cases:
        print(f"{BOLD}Testing: {test_name}{RESET}")
        result = run_single_test(test_name, excel_dir, expected_dir, output_dir)
        results.append(result)

    # Print summary
    print(f"\n{BOLD}{BLUE}=== Test Summary ==={RESET}\n")

    passed = sum(1 for r in results if r.passed)
    failed = len(results) - passed

    for result in results:
        status = f"{GREEN}✓ PASS{RESET}" if result.passed else f"{RED}✗ FAIL{RESET}"
        print(f"  {status}: {result.name}")

        if not result.passed:
            print(f"    {RED}{result.message}{RESET}")

    print(f"\n{BOLD}Total: {len(results)} tests{RESET}")
    print(f"  {GREEN}Passed: {passed}{RESET}")
    print(f"  {RED}Failed: {failed}{RESET}")

    if failed > 0:
        print(f"\n{YELLOW}Tip: Check output files in {output_dir} for details{RESET}")
        return 1
    else:
        print(f"\n{GREEN}{BOLD}All tests passed! 🎉{RESET}")
        return 0

if __name__ == "__main__":
    sys.exit(main())
