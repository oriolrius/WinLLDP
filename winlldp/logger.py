"""Logging wrapper for Windows service compatibility"""

import logging
import os
import sys
from datetime import datetime


class ServiceLogger:
    """Logger that works both in console and Windows service mode"""
    
    def __init__(self, name='WinLLDP', log_file=None):
        self.name = name
        self.log_file = log_file
        self._setup_logger()
    
    def _setup_logger(self):
        """Setup logging configuration"""
        self.logger = logging.getLogger(self.name)
        self.logger.setLevel(logging.DEBUG)
        
        # Clear existing handlers
        self.logger.handlers = []
        
        # Format
        formatter = logging.Formatter(
            '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
        )
        
        # File handler if log file specified
        if self.log_file:
            fh = logging.FileHandler(self.log_file)
            fh.setLevel(logging.DEBUG)
            fh.setFormatter(formatter)
            self.logger.addHandler(fh)
        
        # Console handler only if we have a console
        if hasattr(sys.stdout, 'isatty') and sys.stdout.isatty():
            ch = logging.StreamHandler()
            ch.setLevel(logging.INFO)
            ch.setFormatter(formatter)
            self.logger.addHandler(ch)
    
    def info(self, message):
        """Log info message"""
        self.logger.info(message)
        # Also try to log to Windows Event Log if available
        try:
            import servicemanager
            servicemanager.LogInfoMsg(f"{self.name}: {message}")
        except:
            pass
    
    def error(self, message):
        """Log error message"""
        self.logger.error(message)
        # Also try to log to Windows Event Log if available
        try:
            import servicemanager
            servicemanager.LogErrorMsg(f"{self.name}: {message}")
        except:
            pass
    
    def debug(self, message):
        """Log debug message"""
        self.logger.debug(message)
    
    def warning(self, message):
        """Log warning message"""
        self.logger.warning(message)


# Create a default logger instance
default_logger = None

def get_logger(log_file=None):
    """Get the default logger instance"""
    global default_logger
    if default_logger is None:
        # Default log file in temp directory
        if log_file is None:
            import tempfile
            log_file = os.path.join(tempfile.gettempdir(), 'winlldp_service.log')
        default_logger = ServiceLogger(log_file=log_file)
    return default_logger