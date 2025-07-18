import os
from typing import Optional
from dotenv import load_dotenv


class Config:
    def __init__(self, env_file: Optional[str] = None):
        if env_file:
            load_dotenv(env_file)
        else:
            load_dotenv()
        
        # LLDP Configuration
        self.interval = int(os.getenv('LLDP_INTERVAL', '30'))
        self.interface = os.getenv('LLDP_INTERFACE', 'all')
        self.system_name = os.getenv('LLDP_SYSTEM_NAME', 'auto')
        self.system_description = os.getenv('LLDP_SYSTEM_DESCRIPTION', 'Windows LLDP Service')
        self.port_description = os.getenv('LLDP_PORT_DESCRIPTION', 'Ethernet Port')
        self.management_address = os.getenv('LLDP_MANAGEMENT_ADDRESS', 'auto')
        self.ttl = int(os.getenv('LLDP_TTL', '120'))
        
        # File Configuration
        self.neighbors_file = os.getenv('LLDP_NEIGHBORS_FILE', 'neighbors.json')
        
        # If it's not an absolute path, make it relative to the project root
        if not os.path.isabs(self.neighbors_file):
            # Get project root (parent of winlldp package)
            project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            self.neighbors_file = os.path.join(project_root, self.neighbors_file)
        
        # Validate configuration
        self._validate()
    
    def _validate(self):
        """Validate configuration values"""
        if self.interval < 5:
            raise ValueError("LLDP_INTERVAL must be at least 5 seconds")
        
        if self.ttl < self.interval:
            raise ValueError("LLDP_TTL must be greater than LLDP_INTERVAL")
        
        if self.ttl > 65535:
            raise ValueError("LLDP_TTL must be less than 65536")
    
    def __str__(self):
        return (
            f"Config(\n"
            f"  interval={self.interval}s,\n"
            f"  interface={self.interface},\n"
            f"  system_name={self.system_name},\n"
            f"  system_description={self.system_description},\n"
            f"  port_description={self.port_description},\n"
            f"  management_address={self.management_address},\n"
            f"  ttl={self.ttl}s,\n"
            f"  neighbors_file={self.neighbors_file}\n"
            f")"
        )