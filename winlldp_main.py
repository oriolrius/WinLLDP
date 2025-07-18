#!/usr/bin/env python
"""
Bootstrap script for PyInstaller to ensure all dependencies are loaded
"""

# Import all dependencies explicitly to help PyInstaller find them
import sys
import os

# Standard library imports
import json
import logging
import time
import datetime
import pathlib
import signal
import subprocess
import threading
import multiprocessing
import socket
import struct
import platform
import ctypes

# Third-party imports - these need to be imported before our modules
try:
    import click
    import click.core
    import click.decorators
    import click.exceptions
    import click.formatting
    import click.parser
    import click.termui
    import click.types
    import click.utils
except ImportError as e:
    print(f"Failed to import click: {e}")
    sys.exit(1)

try:
    import dotenv
    from dotenv import load_dotenv
except ImportError as e:
    print(f"Failed to import dotenv: {e}")
    sys.exit(1)

try:
    import psutil
except ImportError as e:
    print(f"Failed to import psutil: {e}")
    sys.exit(1)

try:
    import tabulate
except ImportError as e:
    print(f"Failed to import tabulate: {e}")
    sys.exit(1)

try:
    import scapy
    import scapy.all
    import scapy.layers.l2
    import scapy.layers.inet
    import scapy.sendrecv
    import scapy.packet
    import scapy.fields
    import scapy.config
    import scapy.arch.windows
except ImportError as e:
    print(f"Failed to import scapy: {e}")
    sys.exit(1)

try:
    import win32api
    import win32con
    import win32event
    import win32service
    import win32serviceutil
    import servicemanager
    import pywintypes
except ImportError as e:
    print(f"Failed to import pywin32: {e}")
    # Not fatal as not all commands need Windows service support
    pass

# Now import our CLI module
from winlldp.cli import cli

if __name__ == '__main__':
    cli()