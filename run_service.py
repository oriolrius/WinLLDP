#!/usr/bin/env python
"""Simple script to run the LLDP service for NSSM"""

import sys
import os
import time

# Add the project to path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# Log to a file in the project directory
log_file = os.path.join(os.path.dirname(__file__), "nssm_service.log")

def log(msg):
    """Write to log file"""
    with open(log_file, 'a', encoding='utf-8') as f:
        f.write(f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] {msg}\n")
        f.flush()

try:
    log("=" * 60)
    log("Starting NSSM service wrapper")
    log(f"Python: {sys.executable}")
    log(f"Working directory: {os.getcwd()}")
    
    # Change to script directory
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    log(f"Changed directory to: {os.getcwd()}")
    
    # Import and run
    from winlldp.config import Config
    from winlldp.cli_run import SimpleRunner
    
    log("Creating configuration...")
    config = Config()
    
    log("Creating service runner...")
    runner = SimpleRunner(config)
    
    log("Starting service...")
    
    # Use the simple runner which is more reliable
    runner.run()
        
except KeyboardInterrupt:
    log("Received keyboard interrupt")
    log("Service stopped")
except Exception as e:
    log(f"FATAL ERROR: {e}")
    import traceback
    log(traceback.format_exc())
    sys.exit(1)