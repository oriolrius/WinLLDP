# pyinstaller

```bash
usage: pyinstaller [-h] [-v] [-D] [-F] [--specpath DIR] [-n NAME]
                   [--contents-directory CONTENTS_DIRECTORY]
                   [--add-data SOURCE:DEST]
                   [--add-binary SOURCE:DEST] [-p DIR]
                   [--hidden-import MODULENAME]
                   [--collect-submodules MODULENAME]
                   [--collect-data MODULENAME]
                   [--collect-binaries MODULENAME]
                   [--collect-all MODULENAME]
                   [--copy-metadata PACKAGENAME]
                   [--recursive-copy-metadata PACKAGENAME]
                   [--additional-hooks-dir HOOKSPATH]
                   [--runtime-hook RUNTIME_HOOKS]
                   [--exclude-module EXCLUDES]
                   [--splash IMAGE_FILE]
                   [-d {all,imports,bootloader,noarchive}]
                   [--optimize LEVEL]
                   [--python-option PYTHON_OPTION] [-s] [--noupx]    
                   [--upx-exclude FILE] [-c] [-w]
                   [--hide-console {hide-late,hide-early,minimize-late,minimize-early}]
                   [-i <FILE.ico or FILE.exe,ID or FILE.icns or Image or "NONE">]
                   [--disable-windowed-traceback]
                   [--version-file FILE] [--manifest <FILE or XML>]  
                   [-m <FILE or XML>] [-r RESOURCE] [--uac-admin]    
                   [--uac-uiaccess] [--argv-emulation]
                   [--osx-bundle-identifier BUNDLE_IDENTIFIER]       
                   [--target-architecture ARCH]
                   [--codesign-identity IDENTITY]
                   [--osx-entitlements-file FILENAME]
                   [--runtime-tmpdir PATH]
                   [--bootloader-ignore-signals] [--distpath DIR]    
                   [--workpath WORKPATH] [-y] [--upx-dir UPX_DIR]    
                   [--clean] [--log-level LEVEL]
                   scriptname [scriptname ...]

positional arguments:
  scriptname            Name of scriptfiles to be processed or       
                        exactly one .spec file. If a .spec file is   
                        specified, most options are unnecessary and  
                        are ignored.

options:
  -h, --help            show this help message and exit
  -v, --version         Show program version info and exit.
  --distpath DIR        Where to put the bundled app (default:       
                        ./dist)
  --workpath WORKPATH   Where to put all the temporary work files,   
                        .log, .pyz and etc. (default: ./build)       
  -y, --noconfirm       Replace output directory (default:
                        SPECPATH\dist\SPECNAME) without asking for   
                        confirmation
  --upx-dir UPX_DIR     Path to UPX utility (default: search the     
                        execution path)
  --clean               Clean PyInstaller cache and remove
                        temporary files before building.
  --log-level LEVEL     Amount of detail in build-time console       
                        messages. LEVEL may be one of TRACE, DEBUG,  
                        INFO, WARN, DEPRECATION, ERROR, FATAL        
                        (default: INFO). Also settable via and       
                        overrides the PYI_LOG_LEVEL environment      
                        variable.

... (output truncated for brevity) ...
```
