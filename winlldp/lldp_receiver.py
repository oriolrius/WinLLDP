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
        
        # Define file paths based on config
        self._neighbors_file = self.config.neighbors_file
        # PID file in same directory as neighbors file
        self._pid_file = os.path.join(
            os.path.dirname(self._neighbors_file), 
            'capture.pid'
        )
        
        # Load existing neighbors if capture is running
        if self.is_capture_running():
            self._load_neighbors()
    
    def _get_neighbor_key(self, interface: str, source_mac: str, chassis_id: str) -> str:
        """Generate unique key for a neighbor"""
        return f"{interface}:{source_mac}:{chassis_id}"
    
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
    
    # Process neighbor data method removed - subprocess writes directly to file
    
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
    
    # Reader loop removed - we use file-based communication now
    
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
        
        # Start the subprocess
        log_file = os.path.join(tempfile.gettempdir(), 'winlldp_capture.log')
        
        # For frozen executables, recalculate the neighbors file path
        if getattr(sys, 'frozen', False):
            neighbors_file_path = os.path.join(tempfile.gettempdir(), 'neighbors.json')
            pid_file_path = os.path.join(tempfile.gettempdir(), 'capture.pid')
        else:
            neighbors_file_path = self._neighbors_file
            pid_file_path = self._pid_file
            
        # Escape backslashes for Windows paths
        log_file_escaped = log_file.replace('\\', '\\\\')
        neighbors_file_path_escaped = neighbors_file_path.replace('\\', '\\\\')
        pid_file_path_escaped = pid_file_path.replace('\\', '\\\\')
        sys_path_escaped = sys.path[0].replace('\\', '\\\\')
        
        capture_script = f"""
import sys
import json
import os
import traceback
from datetime import datetime

# Log file for debugging
log_file = '{log_file_escaped}'

def log(msg):
    try:
        with open(log_file, 'a') as f:
            f.write(f"[{{datetime.now().isoformat()}}] {{msg}}\\n")
            f.flush()
    except:
        pass

def log_file_op(operation, filepath, mode=''):
    # Log file operations in the capture subprocess
    if mode:
        log(f"FILE_OP: {{operation}} '{{filepath}}' mode={{mode}}")
    else:
        log(f"FILE_OP: {{operation}} '{{filepath}}')")

try:
    log("Starting LLDP capture process")
    log(f"PID: {{os.getpid()}}")
    log(f"Python: {{sys.executable}}")
    log(f"Neighbors file will be: {neighbors_file_path_escaped}")
    log(f"PID file will be: {pid_file_path_escaped}")
    
    # Write PID
    log_file_op("OPEN", '{pid_file_path_escaped}', 'w')
    with open('{pid_file_path_escaped}', 'w') as f:
        f.write(str(os.getpid()))
    log_file_op("CLOSE", '{pid_file_path_escaped}')
    log("PID file written")
    
    # Import required modules
    log("Importing modules...")
    from scapy.all import sniff, Ether
    sys.path.insert(0, '{sys_path_escaped}')
    from winlldp.lldp_packet import LLDPPacket
    from winlldp.system_info import SystemInfo
    log("Modules imported successfully")
    
    def process_lldp_packet(packet):
        try:
            if packet.haslayer(Ether) and packet[Ether].type == 0x88cc:
                interface = packet.sniffed_on if hasattr(packet, 'sniffed_on') else 'unknown'
                source_mac = packet[Ether].src
                log(f"Received LLDP packet from {{source_mac}} on {{interface}}")
                
                lldp_data = bytes(packet)[14:]
                lldp_packet = LLDPPacket.decode(lldp_data)
                neighbor_data = lldp_packet.to_dict()
                
                # Save neighbor data directly to file
                neighbors_file = '{neighbors_file_path_escaped}'
                log(f"Saving to file: {{neighbors_file}}")
                
                # Load existing neighbors
                try:
                    log_file_op("OPEN", neighbors_file, 'r')
                    with open(neighbors_file, 'r') as f:
                        neighbors = json.load(f)
                    log_file_op("CLOSE", neighbors_file)
                except:
                    neighbors = {{}}
                    log("No existing neighbors file, creating new one")
                
                # Create neighbor key
                chassis_id = neighbor_data.get('chassis_id', '')
                key = f"{{interface}}:{{source_mac}}:{{chassis_id}}"
                
                # Update neighbor
                neighbors[key] = {{
                    'interface': interface,
                    'source_mac': source_mac,
                    'data': neighbor_data,
                    'first_seen': neighbors.get(key, {{}}).get('first_seen', datetime.now().isoformat()),
                    'last_seen': datetime.now().isoformat(),
                    'ttl': neighbor_data.get('ttl', 120)
                }}
                
                # Save back to file
                log_file_op("OPEN", neighbors_file, 'w')
                with open(neighbors_file, 'w') as f:
                    json.dump(neighbors, f, indent=2)
                log_file_op("CLOSE", neighbors_file)
                
                log(f"Saved neighbor data to file ({{len(neighbors)}} total neighbors)")
                
        except Exception as e:
            log(f"Error processing packet: {{e}}")
            log(traceback.format_exc())
    
    # Get all interfaces
    log("Getting network interfaces...")
    system_info = SystemInfo()
    interfaces = system_info.get_interfaces()
    iface_names = [iface['name'] for iface in interfaces if iface['is_up'] and iface['mac']]
    log(f"Found interfaces: {{iface_names}}")
    
    if iface_names:
        print(f"Capturing on interfaces: {{', '.join(iface_names)}}")
        sys.stdout.flush()
        log(f"Starting packet capture on: {{', '.join(iface_names)}}")
        
        try:
            sniff(
                filter='ether proto 0x88cc',
                prn=process_lldp_packet,
                iface=iface_names,
                store=False
            )
        except KeyboardInterrupt:
            log("Capture stopped by KeyboardInterrupt")
        except Exception as e:
            log(f"Capture error: {{e}}")
            log(traceback.format_exc())
    else:
        log("ERROR: No active interfaces found!")
        
except Exception as e:
    log(f"FATAL ERROR: {{e}}")
    log(traceback.format_exc())
finally:
    # Clean up PID file
    try:
        log_file_op("REMOVE", '{pid_file_path_escaped}')
        os.remove('{pid_file_path_escaped}')
        log("PID file removed")
    except:
        pass
    log("Capture process exiting")
"""
        
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
            # Normal Python execution
            self.subprocess = subprocess.Popen(
                [sys.executable, '-c', capture_script],
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