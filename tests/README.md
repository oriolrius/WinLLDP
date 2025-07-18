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
After building with PyInstaller, the paths should automatically adjust to use the exe directory.

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