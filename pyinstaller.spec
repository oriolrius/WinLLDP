# -*- mode: python ; coding: utf-8 -*-

import sys
from pathlib import Path

# Get the directory containing this spec file
spec_dir = Path(SPECPATH).resolve()
project_root = spec_dir

a = Analysis(
    ['winlldp_main.py'],  # Use bootstrap script to avoid relative import issues
    pathex=[str(project_root)],
    binaries=[],
    datas=[
        # Include the entire winlldp package
        ('winlldp', 'winlldp'),
        # Include env.example as a reference
        ('env.example', '.'),
    ],
    hiddenimports=[
        # Core dependencies
        'click',
        'click.core',
        'click.decorators',
        'click.exceptions',
        'click.formatting',
        'click.parser',
        'click.termui',
        'click.types',
        'click.utils',
        # Scapy and networking
        'scapy',
        'scapy.all',
        'scapy.layers',
        'scapy.layers.l2',
        'scapy.layers.inet',
        'scapy.arch',
        'scapy.arch.windows',
        'scapy.arch.windows.native',
        'scapy.config',
        'scapy.sendrecv',
        'scapy.packet',
        'scapy.fields',
        'scapy.base_classes',
        'scapy.volatile',
        'scapy.error',
        'scapy.themes',
        'scapy.consts',
        # Python standard library modules used by scapy
        'ipaddress',
        'socket',
        'struct',
        'subprocess',
        'threading',
        'queue',
        'platform',
        'ctypes',
        'ctypes.wintypes',
        # Other dependencies
        'dotenv',
        'python-dotenv',
        'psutil',
        'psutil._psutil_windows',
        'psutil._pswindows',
        'tabulate',
        # Windows specific
        'win32api',
        'win32con',
        'win32event',
        'win32evtlog',
        'win32evtlogutil',
        'win32service',
        'win32serviceutil',
        'servicemanager',
        'pywintypes',
        # Additional modules that might be dynamically imported
        'encodings',
        'encodings.utf_8',
        'json',
        'logging',
        'logging.handlers',
        'os',
        'sys',
        'time',
        'datetime',
        'pathlib',
        'signal',
        'multiprocessing',
        'multiprocessing.connection',
        'pickle',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=['pyi_rth_suppress_warnings.py'],
    excludes=[
        'tkinter',
        'matplotlib',
        'numpy',
        'pandas',
        'scipy',
        'PIL',
        'PyQt5',
        'PyQt6',
        'PySide2',
        'PySide6',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=None,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=None)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    name='winlldp',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=None,
)
