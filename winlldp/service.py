import time
import signal
import sys
from typing import Optional
from .config import Config
from .lldp_sender import LLDPSender
from .lldp_receiver import LLDPReceiver
from .logger import get_logger


class WinLLDPService:
    """Main LLDP service that combines sender and receiver
    
    This is a thin wrapper around the sender and receiver components
    that manages their lifecycle for Windows service usage.
    """
    
    def __init__(self, config: Optional[Config] = None):
        self.config = config or Config()
        self.sender = LLDPSender(self.config)
        self.receiver = LLDPReceiver(self.config)  # Pass config to receiver
        self.running = False
        self.logger = get_logger()
        
        # Setup signal handlers
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
    
    def _signal_handler(self, signum, frame):
        """Handle shutdown signals"""
        self.logger.info(f"Received signal {signum}, shutting down...")
        self.stop()
        sys.exit(0)
    
    def start(self):
        """Start the LLDP service"""
        if self.running:
            return
        
        self.running = True
        self.logger.info("Starting LLDP service...")
        
        # Start capture subprocess if not already running
        if not self.receiver.is_capture_running():
            self.logger.info("Starting capture subprocess...")
            if self.receiver.start_capture():
                self.logger.info(f"Capture started, writing to: {self.config.neighbors_file}")
            else:
                self.logger.warning("Failed to start capture subprocess")
                # Continue anyway - sender will still work
        else:
            self.logger.info("Capture subprocess already running")
        
        # Start receiver (cleanup thread)
        self.logger.debug("Starting LLDP receiver cleanup thread...")
        self.receiver.start()
        self.logger.info("LLDP receiver cleanup thread started")
        
        # Start sender
        self.logger.debug("Starting LLDP sender...")
        self.sender.start()
        self.logger.info(f"LLDP sender started (interval: {self.config.interval}s)")
        
        self.logger.info("LLDP service is running")
    
    def stop(self):
        """Stop the LLDP service"""
        if not self.running:
            return
        
        self.running = False
        self.logger.info("Stopping LLDP service...")
        
        # Stop sender
        self.sender.stop()
        self.logger.info("LLDP sender stopped")
        
        # Stop receiver cleanup thread
        self.receiver.stop()
        self.logger.info("LLDP receiver cleanup thread stopped")
        
        # Note: We don't stop the capture subprocess here
        # It continues running in the background for other tools
        self.logger.info("LLDP service stopped (capture subprocess continues running)")
    
    def get_status(self):
        """Get service status including capture info"""
        capture_status = self.receiver.get_capture_status()
        neighbors = self.receiver.get_neighbors()
        
        return {
            'service_running': self.running,
            'capture_running': capture_status['running'],
            'capture_pid': capture_status.get('pid'),
            'sender_running': self.sender.running if hasattr(self.sender, 'running') else False,
            'neighbor_count': len(neighbors),
            'neighbors_file': self.config.neighbors_file
        }
    
    def run_forever(self):
        """Run the service until interrupted"""
        self.start()
        
        try:
            while self.running:
                # Show status periodically
                if int(time.time()) % 60 == 0:  # Every minute
                    status = self.get_status()
                    self.logger.info(f"Service status: {status['neighbor_count']} neighbors discovered")
                time.sleep(1)
        except KeyboardInterrupt:
            pass
        finally:
            self.stop()


def main():
    """Main entry point for the service"""
    config = Config()
    service = WinLLDPService(config)
    service.run_forever()


if __name__ == '__main__':
    main()