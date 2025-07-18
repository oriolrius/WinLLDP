import time
import json
import subprocess
import sys
import threading
import os
import tempfile
from typing import Dict, List, Optional
from datetime import datetime, timedelta
from .lldp_packet import LLDPPacket
from .system_info import SystemInfo
from .config import Config
from .logger import get_logger
from .file_debug import debug_open as open


class Neighbor:
    def __init__(self, interface: str, source_mac: str, packet_data: Dict):
        self.interface = interface
        self.source_mac = source_mac
        self.data = packet_data
        self.first_seen = datetime.now()
        self.last_seen = datetime.now()
        self.ttl = packet_data.get('ttl', 120)
        
    def update(self, packet_data: Dict):
        """Update neighbor with new packet data"""
        self.data = packet_data
        self.last_seen = datetime.now()
        self.ttl = packet_data.get('ttl', 120)
    
    def is_expired(self) -> bool:
        """Check if neighbor has expired based on TTL"""
        return datetime.now() > self.last_seen + timedelta(seconds=self.ttl)
    
    def get_age(self) -> str:
        """Get age of neighbor in human-readable format"""
        age = datetime.now() - self.first_seen
        if age.days > 0:
            return f"{age.days}d {age.seconds // 3600}h"
        elif age.seconds >= 3600:
            return f"{age.seconds // 3600}h {(age.seconds % 3600) // 60}m"
        else:
            return f"{age.seconds // 60}m {age.seconds % 60}s"


class LLDPReceiver:
    """LLDP receiver that runs packet capture in a subprocess for better signal handling"""
    
    # Store subprocess info in a class variable for persistence across instances
    _subprocess_info = None
    _config = None
    
    def __init__(self, config: Optional[Config] = None):
        self.neighbors: Dict[str, Neighbor] = {}
        self.running = False
        self.subprocess = None
        self.reader_thread = None
        self.lock = threading.Lock()
        self.system_info = SystemInfo()
        self.logger = get_logger()
        
        # Load or create config
        if config:
            self.config = config
            LLDPReceiver._config = config
        else:
            # Try to use cached config or create new one
            self.config = LLDPReceiver._config or Config()
            LLDPReceiver._config = self.config
        
        # Define file paths from config
        self._neighbors_file = self.config.neighbors_file
        self._pid_file = self.config.pid_file
        self._log_file = self.config.log_file
        
        # Load existing neighbors if capture is running
        if self.is_capture_running():
            self._load_neighbors()
    
    
    def _save_neighbors(self):
        """Save neighbors to file for persistence"""
        try:
            data = {}
            with self.lock:
                for key, neighbor in self.neighbors.items():
                    data[key] = {
                        'interface': neighbor.interface,
                        'source_mac': neighbor.source_mac,
                        'data': neighbor.data,
                        'first_seen': neighbor.first_seen.isoformat(),
                        'last_seen': neighbor.last_seen.isoformat(),
                        'ttl': neighbor.ttl
                    }
            
            with open(self._neighbors_file, 'w') as f:
                json.dump(data, f)
        except Exception:
            pass
    
    def _load_neighbors(self):
        """Load neighbors from file"""
        try:
            if os.path.exists(self._neighbors_file):
                with open(self._neighbors_file, 'r') as f:
                    data = json.load(f)
                
                with self.lock:
                    for key, neighbor_data in data.items():
                        neighbor = Neighbor(
                            neighbor_data['interface'],
                            neighbor_data['source_mac'],
                            neighbor_data['data']
                        )
                        neighbor.first_seen = datetime.fromisoformat(neighbor_data['first_seen'])
                        neighbor.last_seen = datetime.fromisoformat(neighbor_data['last_seen'])
                        neighbor.ttl = neighbor_data['ttl']
                        
                        # Only load non-expired neighbors
                        if not neighbor.is_expired():
                            self.neighbors[key] = neighbor
        except Exception:
            pass
    
    
    def _cleanup_expired_neighbors(self):
        """Remove expired neighbors"""
        with self.lock:
            expired_keys = [
                key for key, neighbor in self.neighbors.items() 
                if neighbor.is_expired()
            ]
            for key in expired_keys:
                del self.neighbors[key]
        
        if expired_keys:
            self._save_neighbors()
    
    
    def is_capture_running(self) -> bool:
        """Check if capture subprocess is running"""
        try:
            if os.path.exists(self._pid_file):
                with open(self._pid_file, 'r') as f:
                    pid = int(f.read().strip())
                
                # Check if process exists on Windows
                try:
                    import psutil
                    return psutil.pid_exists(pid)
                except:
                    # Fallback method
                    os.kill(pid, 0)
                    return True
        except:
            return False
        
        return False
    
    def start_capture(self) -> bool:
        """Start the LLDP capture subprocess"""
        if self.is_capture_running():
            self.logger.info("Capture is already running")
            return False
        
        # Use config paths for all files
        log_file = self._log_file
        neighbors_file_path = self._neighbors_file
        pid_file_path = self._pid_file
            
        
        # Start subprocess detached
        # Keep stderr pipe for error checking, but not stdout
        
        # Check if running from PyInstaller bundle
        if getattr(sys, 'frozen', False):
            # We're running from a PyInstaller bundle
            # Use the capture-subprocess command
            self.subprocess = subprocess.Popen(
                [sys.executable, 'capture-subprocess', 
                 log_file, neighbors_file_path, pid_file_path],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0
            )
        else:
            # Normal Python execution - use the capture_subprocess module
            self.subprocess = subprocess.Popen(
                [sys.executable, '-m', 'winlldp.capture_subprocess',
                 log_file, neighbors_file_path, pid_file_path],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0
            )
        
        # Save subprocess info
        LLDPReceiver._subprocess_info = self.subprocess
        
        self.logger.info(f"Started LLDP capture subprocess (PID: {self.subprocess.pid})")
        
        # Check if process started successfully
        time.sleep(0.5)
        if self.subprocess.poll() is not None:
            # Process already exited, show error
            stderr = self.subprocess.stderr.read()
            self.logger.error(f"Capture process exited immediately")
            if stderr:
                self.logger.error(f"Error output: {stderr}")
            return False
            
        return True
    
    def stop_capture(self) -> bool:
        """Stop the LLDP capture subprocess"""
        try:
            if os.path.exists(self._pid_file):
                with open(self._pid_file, 'r') as f:
                    pid = int(f.read().strip())
                
                # Terminate the process
                if os.name == 'nt':
                    subprocess.run(['taskkill', '/F', '/PID', str(pid)], capture_output=True)
                else:
                    os.kill(pid, 9)
                
                # Clean up files
                try:
                    os.remove(self._pid_file)
                except:
                    pass
                
                self.logger.info(f"Stopped LLDP capture subprocess (PID: {pid})")
                return True
            else:
                self.logger.info("No capture process found")
                return False
                
        except Exception as e:
            self.logger.error(f"Error stopping capture: {e}")
            return False
    
    def get_capture_status(self) -> Dict[str, any]:
        """Get status of capture subprocess"""
        if self.is_capture_running():
            try:
                with open(self._pid_file, 'r') as f:
                    pid = int(f.read().strip())
                
                # Get process info
                try:
                    import psutil
                    process = psutil.Process(pid)
                    create_time = datetime.fromtimestamp(process.create_time())
                    uptime = datetime.now() - create_time
                    
                    return {
                        'running': True,
                        'pid': pid,
                        'uptime': str(uptime).split('.')[0],
                        'memory_mb': process.memory_info().rss / 1024 / 1024,
                        'cpu_percent': process.cpu_percent(interval=0.1)
                    }
                except:
                    return {
                        'running': True,
                        'pid': pid,
                        'uptime': 'unknown',
                        'memory_mb': 0,
                        'cpu_percent': 0
                    }
            except:
                pass
        
        return {
            'running': False,
            'pid': None,
            'uptime': None,
            'memory_mb': 0,
            'cpu_percent': 0
        }
    
    def start(self):
        """Start the LLDP receiver (just starts cleanup thread for file-based operation)"""
        if self.running:
            return
        
        self.running = True
        
        # Don't try to connect to subprocess stdout - we use file-based communication
        # Just start cleanup thread
        cleanup_thread = threading.Thread(target=self._cleanup_loop, daemon=True)
        cleanup_thread.start()
    
    def _cleanup_loop(self):
        """Periodic cleanup of expired neighbors"""
        while self.running:
            time.sleep(5)
            self._cleanup_expired_neighbors()
    
    def stop(self):
        """Stop the LLDP receiver (keeps capture running)"""
        self.running = False
        
        # Note: We don't stop the capture subprocess here
        # Use stop_capture() to actually stop the capture
    
    def get_neighbors(self) -> List[Dict]:
        """Get list of current neighbors"""
        # Reload from file to get latest data
        if self.is_capture_running():
            self._load_neighbors()
        
        with self.lock:
            # Cleanup expired first
            self._cleanup_expired_neighbors()
            
            # Return neighbor information
            neighbors = []
            for neighbor in self.neighbors.values():
                neighbor_info = {
                    'interface': neighbor.interface,
                    'source_mac': neighbor.source_mac,
                    'age': neighbor.get_age(),
                    'ttl': neighbor.ttl,
                    'expires_in': max(0, (neighbor.last_seen + timedelta(seconds=neighbor.ttl) - datetime.now()).seconds),
                    **neighbor.data
                }
                neighbors.append(neighbor_info)
            
            return neighbors
    
    def clear_neighbors(self):
        """Clear all discovered neighbors"""
        with self.lock:
            self.neighbors.clear()
        
        # Clear the file too
        try:
            if os.path.exists(self._neighbors_file):
                os.remove(self._neighbors_file)
        except:
            pass