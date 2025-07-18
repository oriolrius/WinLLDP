"""Simple non-blocking run command implementation"""

import signal
import sys
import threading
import time
from .lldp_sender import LLDPSender
from .lldp_receiver import LLDPReceiver
from .config import Config


class SimpleRunner:
    def __init__(self, config):
        self.config = config
        self.sender = LLDPSender(config)
        self.receiver = LLDPReceiver(config)
        self.running = False
        self._stop_event = threading.Event()
        
    def signal_handler(self, signum, frame):
        """Handle Ctrl+C"""
        print("\nReceived signal, stopping...")
        self.running = False
        self._stop_event.set()
        
    def run(self):
        """Run the service with proper signal handling"""
        # Set up signal handler
        signal.signal(signal.SIGINT, self.signal_handler)
        signal.signal(signal.SIGTERM, self.signal_handler)
        
        print("Starting Windows LLDP Service...")
        print(f"Configuration: {self.config}")
        print("\nPress Ctrl+C to stop\n")
        
        # Check if capture is already running
        if self.receiver.is_capture_running():
            print("Note: Capture subprocess is already running")
        else:
            print("Starting capture subprocess...")
            if not self.receiver.start_capture():
                print("Warning: Failed to start capture subprocess")
        
        # Start sender
        print("Starting LLDP sender...")
        self.sender.start()
        print("Service is running\n")
        
        self.running = True
        last_status = time.time()
        
        # Main loop - just sleep and check for stop signal
        while self.running:
            # Use wait with timeout for interruptibility
            if self._stop_event.wait(timeout=1.0):
                break
                
            # Show simple status every 30 seconds
            if time.time() - last_status >= 30:
                last_status = time.time()
                print(f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] Service is running...")
        
        # Stop sender
        print("\nStopping LLDP sender...")
        self.sender.stop()
        print("Service stopped")
        print("Note: Capture subprocess continues running in background")
        print("Use 'winlldp capture stop' to stop it")


def run_simple(config):
    """Entry point for simple run"""
    runner = SimpleRunner(config)
    runner.run()