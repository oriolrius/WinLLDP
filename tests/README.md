# WinLLDP Path Resolution Tests

These tests verify that WinLLDP correctly handles file paths in both development and frozen (PyInstaller) environments.

## Test Files

### `test_paths.py`

Tests the core path resolution logic:

- Development mode paths (project root)
- Frozen exe paths (exe directory)
- Config class integration
- Custom path override support

### `test_service_paths.py`

Tests consistency between service and CLI modes:

- Service wrapper directory handling
- Receiver path configuration
- Subprocess argument passing
- PID file location consistency

### `test_path_scenarios.py`

Integration tests for real-world scenarios:

- Fresh installation
- Service file creation
- CLI reading service files
- PyInstaller temp directory avoidance
- Development vs production modes

### `test_frozen_executable.py`

Integration tests for the actual frozen executable:

- Builds the executable using PyInstaller
- Tests that files are created in exe directory
- Verifies no files are created in temp directories
- Tests all CLI commands with the actual exe
- Requires building the executable first

## Running Tests

### In Development

```bash
# Run all tests
uv run python -m pytest tests/

# Run specific test file
uv run python -m pytest tests/test_paths.py

# Run with verbose output
uv run python -m pytest -v tests/
```

### In Windows with venv activated

```powershell
# Activate virtual environment
.venv\Scripts\activate

# Run all tests with verbose output
python -m pytest tests/ -v

# Run specific test file
python -m pytest tests/test_paths.py -v

# Run specific test
python -m pytest tests/test_path_scenarios.py::TestPathScenarios::test_scenario_pyinstaller_temp_ignored -v
```

### Testing Frozen Executable

```powershell
# Build and test the frozen executable
python tests/run_frozen_tests.py

# Or manually:
# 1. Build the executable
just pyinstaller

# 2. Run frozen executable tests
python -m pytest tests/test_frozen_executable.py -v

# Run specific frozen test
python -m pytest tests/test_frozen_executable.py::TestFrozenExecutable::test_files_created_in_exe_directory -v

# Force rebuild and see progress dots
Remove-Item dist\winlldp.exe -Force
python -m pytest tests/test_frozen_executable.py -v -s

# Run only service tests (requires admin)
python -m pytest tests/test_frozen_executable.py::TestServiceMode -v -s
```

#### Important Flags

- `-v` : Verbose output showing each test
- `-s` : Show print statements and progress dots (disables output capture)
- `--tb=short` : Shorter traceback format for failures

### What Gets Tested

The `test_frozen_executable.py` file:

1. **Automatic Build Management**
   - Checks if executable exists and is less than 1 hour old
   - If missing or old, builds using: `just pyinstaller` → `uv run pyinstaller` → `uvx pyinstaller`
   - Shows progress dots during build (with `-s` flag)

2. **Test Isolation**
   - Copies exe to temporary directory for each test
   - Ensures tests don't interfere with each other
   - Cleans up processes and files after each test

3. **Comprehensive Testing**
   - All CLI commands (version, send, capture, show-interfaces, etc.)
   - File creation in correct directory (exe dir, not temp)
   - Service installation/start/stop/uninstall (requires admin)
   - Dynamic version checking against pyproject.toml

4. **Path Verification**
   - Confirms neighbors.json, capture.pid, and logs are in exe directory
   - Verifies NO files are created in Windows temp directories
   - Tests both CLI and service modes use same paths

## Key Test Scenarios

1. **Development Mode**: Files in project root
2. **Frozen Mode**: Files in exe directory (NOT in temp)
3. **Service Mode**: Creates files in correct location
4. **CLI Mode**: Reads files from same location as service
5. **PyInstaller Temp**: Should be ignored, not used for data files

## Expected Behavior

### Development

- Runtime directory: Project root (parent of winlldp package)
- Files: `./neighbors.json`, `./capture.pid`, `./winlldp_capture.log`

### Frozen (Executable)

- Runtime directory: Directory containing winlldp.exe
- Files: `<exe_dir>/neighbors.json`, `<exe_dir>/capture.pid`, `<exe_dir>/winlldp_capture.log`

### Never Used

- `C:\Windows\Temp\_MEI*` (PyInstaller extraction directory)
- `C:\Users\<user>\AppData\Local\Temp` (System temp directory)

## Troubleshooting

### Tests fail with "PyInstaller not found"

```powershell
# Install PyInstaller
uv pip install pyinstaller
```

### Service tests skip with "Admin privileges required"

- Run PowerShell as Administrator
- Or run: `Start-Process powershell -Verb RunAs`

### Don't see progress dots during build

- Use `-s` flag: `python -m pytest tests/test_frozen_executable.py -v -s`
- Delete exe first to force rebuild: `Remove-Item dist\winlldp.exe -Force`

### Version test fails

- Check pyproject.toml version matches what the exe reports
- The test dynamically reads version from pyproject.toml

## Test Output Example

```text
============================= test session starts =============================
Building executable with PyInstaller...
Running: uv run pyinstaller..........
Executable built successfully: D:\...\dist\winlldp.exe

tests/test_frozen_executable.py::TestFrozenExecutable::test_capture_log_command PASSED [ 11%]
tests/test_frozen_executable.py::TestFrozenExecutable::test_exe_runs_without_error PASSED [ 22%]
...
======================== 9 passed in 140.25s (0:02:20) ========================
```
