# -*- mode: python ; coding: utf-8 -*-

import sys
import os
from pathlib import Path

block_cipher = None

# Get the paths
spec_dir = Path(SPECPATH).resolve()
project_root = spec_dir

# Find the site-packages directory from the current Python environment
import site
site_packages = site.getsitepackages()[0]

a = Analysis(
    ['winlldp_main.py'],
    pathex=[
        str(project_root),
        site_packages,  # Add site-packages to path
    ],
    binaries=[],
    datas=[
        # Include our package
        ('winlldp', 'winlldp'),
        # Include env.example
        ('env.example', '.'),
        # Include click package data
        (os.path.join(site_packages, 'click'), 'click'),
    ],
    hiddenimports=[
        # Our modules
        'winlldp',
        'winlldp.cli',
        'winlldp.config',
        'winlldp.lldp_packet',
        'winlldp.lldp_receiver', 
        'winlldp.lldp_sender',
        'winlldp.logger',
        'winlldp.service_wrapper',
        'winlldp.system_info',
        # All click modules
        'click',
        'click.core',
        'click.decorators',
        'click.exceptions',
        'click.formatting',
        'click.globals',
        'click.parser',
        'click.termui',
        'click.testing',
        'click.types',
        'click.utils',
        'click._compat',
        'click._termui_impl',
        'click._winconsole',
        # Scapy
        'scapy',
        'scapy.all',
        'scapy.arch',
        'scapy.arch.windows',
        'scapy.arch.windows.native',
        'scapy.base_classes',
        'scapy.compat',
        'scapy.config',
        'scapy.consts',
        'scapy.data',
        'scapy.error',
        'scapy.fields',
        'scapy.interfaces',
        'scapy.layers',
        'scapy.layers.inet',
        'scapy.layers.l2',
        'scapy.packet',
        'scapy.route',
        'scapy.sendrecv',
        'scapy.supersocket',
        'scapy.themes',
        'scapy.utils',
        'scapy.volatile',
        # Other deps
        'dotenv',
        'psutil',
        'psutil._common',
        'psutil._compat',
        'psutil._psutil_windows',
        'psutil._pswindows',
        'tabulate',
        # Win32
        'pythoncom',
        'pywintypes',
        'win32api',
        'win32con', 
        'win32event',
        'win32evtlog',
        'win32evtlogutil',
        'win32service',
        'win32serviceutil',
        'servicemanager',
        # Standard library that might be missed
        'encodings',
        'collections.abc',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'matplotlib',
        'numpy', 
        'pandas',
        'PIL',
        'tkinter',
        'PyQt5',
        'PyQt6',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
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
)