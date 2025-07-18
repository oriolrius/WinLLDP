# Justfile for winlldp project
# use PowerShell instead of sh:
set shell := ["powershell.exe", "-c"]

pyinstaller:
    # remove folder build/ and dist/ and ignore if they do not exist
    Remove-Item -Recurse -Force .\build\, .\dist\ -ErrorAction SilentlyContinue;
    # run pyinstaller with the specified spec file
    uv run pyinstaller --log-level DEBUG --clean pyinstaller.spec;
    
