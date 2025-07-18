# Justfile for winlldp project
# use PowerShell instead of sh:
set shell := ["powershell.exe", "-c"]

pyinstaller:
    # stop any running winlldp.exe processes to avoid file lock issues
    Stop-Process -Name winlldp -Force -ErrorAction SilentlyContinue; exit 0;
    # remove folder build/ and dist/ and ignore if they do not exist
    Remove-Item -Recurse -Force .\build\, .\dist\ -ErrorAction SilentlyContinue; exit 0;
    # run pyinstaller with the specified spec file
    uv run pyinstaller --log-level DEBUG --clean pyinstaller.spec;
    
