"""Run tests for the frozen executable"""
import subprocess
import sys
import os
from pathlib import Path


def main():
    """Build executable and run frozen tests"""
    project_root = Path(__file__).parent.parent
    
    print("=" * 60)
    print("Building executable with PyInstaller...")
    print("=" * 60)
    
    # Build executable
    result = subprocess.run(
        ["just", "pyinstaller"],
        cwd=project_root
    )
    
    if result.returncode != 0:
        print("\\nFailed to build executable with 'just'. Trying direct command...")
        result = subprocess.run(
            [sys.executable, "-m", "PyInstaller", "--clean", "pyinstaller.spec"],
            cwd=project_root
        )
    
    if result.returncode != 0:
        print("\\nFailed to build executable!")
        return 1
    
    print("\\n" + "=" * 60)
    print("Running frozen executable tests...")
    print("=" * 60 + "\\n")
    
    # Run frozen executable tests
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "tests/test_frozen_executable.py", "-v"],
        cwd=project_root
    )
    
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())