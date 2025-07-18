#!/usr/bin/env python
"""Windows service wrapper for NSSM - provides a simple entry point for the service"""

import sys
import os
import time
import traceback

# Only add to path if not frozen
if not getattr(sys, 'frozen', False):
    # Add the parent directory to Python path so we can import winlldp
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from winlldp.config import Config
from winlldp.lldp_sender import LLDPSender
from winlldp.lldp_receiver import LLDPReceiver
from winlldp.file_debug import debug_open as open


def main():
    """Main entry point for NSSM service wrapper"""
    # Import paths module
    from winlldp.paths import get_service_log_file, get_runtime_directory
    
    # Use centralized path for log file
    log_file = get_service_log_file()
    
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
        
        # Ensure we're in the runtime directory (exe directory when frozen)
        runtime_dir = get_runtime_directory()
        if os.getcwd() != runtime_dir:
            os.chdir(runtime_dir)
            log(f"Changed directory to: {os.getcwd()}")
        else:
            log(f"Already in runtime directory: {runtime_dir}")
        
        # Create configuration
        log("Creating configuration...")
        config = Config()
        
        # Create components
        log("Creating components...")
        receiver = LLDPReceiver(config)
        sender = LLDPSender(config)
        
        # Check/start capture
        if receiver.is_capture_running():
            log("Capture subprocess already running")
        else:
            log("Starting capture subprocess...")
            if receiver.start_capture():
                log("Capture started successfully")
            else:
                log("WARNING: Failed to start capture subprocess")
        
        # Start sender
        log("Starting LLDP sender...")
        sender.start()
        log("Service started successfully")
        
        # Keep running until interrupted
        try:
            while True:
                time.sleep(60)
                log("Service is running...")
        except KeyboardInterrupt:
            log("Stopping sender...")
            sender.stop()
            log("Service stopped")
            
    except KeyboardInterrupt:
        log("Received keyboard interrupt")
        log("Service stopped")
    except Exception as e:
        log(f"FATAL ERROR: {e}")
        log(traceback.format_exc())
        sys.exit(1)


if __name__ == '__main__':
    main()