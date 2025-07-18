#!/usr/bin/env python
"""
Bootstrap script for PyInstaller to ensure all dependencies are loaded
"""

# Import all dependencies explicitly to help PyInstaller find them
import sys
import os

# Standard library imports - only what's actually needed
import json
import logging
import time
import subprocess
import threading
import socket
import struct
import platform

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
    # Only import what's actually used by the service functionality
    import win32serviceutil
    import servicemanager
except ImportError as e:
    # Not fatal as not all commands need Windows service support
    pass

# Now import our CLI module
from winlldp.cli import cli

if __name__ == '__main__':
    cli()