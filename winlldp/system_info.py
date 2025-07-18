import socket
import platform
import psutil
import struct
from typing import Dict, List, Optional, Tuple
import ipaddress
import subprocess
import re


class SystemInfo:
    @staticmethod
    def get_hostname() -> str:
        return socket.gethostname()

    @staticmethod
    def get_system_description() -> str:
        system = platform.system()
        version = platform.version()
        machine = platform.machine()
        return f"{system} {version} {machine}"

    @staticmethod
    def get_interfaces() -> List[Dict[str, any]]:
        interfaces = []
        
        # Get network interface information using psutil
        addrs = psutil.net_if_addrs()
        stats = psutil.net_if_stats()
        
        for interface_name, addr_list in addrs.items():
            try:
                interface_info = {
                    'name': interface_name,
                    'is_up': False,
                    'mac': None,
                    'ipv4': [],
                    'ipv6': []
                }
                
                # Process addresses
                for addr in addr_list:
                    if addr.family == psutil.AF_LINK:
                        # MAC address
                        if addr.address:
                            interface_info['mac'] = addr.address.lower().replace('-', ':')
                    elif addr.family == socket.AF_INET:
                        # IPv4 address
                        interface_info['ipv4'].append(addr.address)
                    elif addr.family == socket.AF_INET6:
                        # IPv6 address
                        # Remove scope id from IPv6 address
                        ipv6_addr = addr.address.split('%')[0]
                        interface_info['ipv6'].append(ipv6_addr)
                
                # Check if interface is up
                if interface_name in stats:
                    interface_info['is_up'] = stats[interface_name].isup
                
                # Only include interfaces with MAC addresses
                if interface_info['mac']:
                    interfaces.append(interface_info)
                
            except Exception:
                continue
        
        return interfaces

    @staticmethod
    def get_mac_address_bytes(mac_str: str) -> bytes:
        """Convert MAC address string to bytes"""
        return bytes.fromhex(mac_str.replace(':', '').replace('-', ''))

    @staticmethod
    def get_primary_ip() -> Optional[str]:
        """Get the primary IP address used for internet connectivity"""
        try:
            # Create a dummy connection to determine the primary interface
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("8.8.8.8", 80))
            ip = s.getsockname()[0]
            s.close()
            return ip
        except Exception:
            return None

    @staticmethod
    def get_interface_by_ip(ip: str) -> Optional[Dict[str, any]]:
        """Find interface information by IP address"""
        interfaces = SystemInfo.get_interfaces()
        for interface in interfaces:
            if ip in interface['ipv4']:
                return interface
        return None

    @staticmethod
    def get_interface_by_name(name: str) -> Optional[Dict[str, any]]:
        """Find interface information by interface name"""
        interfaces = SystemInfo.get_interfaces()
        for interface in interfaces:
            if interface['name'] == name:
                return interface
        return None

    @staticmethod
    def get_system_capabilities() -> Tuple[int, int]:
        """
        Get system capabilities bitmap.
        Bit 0: Other
        Bit 1: Repeater
        Bit 2: Bridge
        Bit 3: WLAN Access Point
        Bit 4: Router
        Bit 5: Telephone
        Bit 6: DOCSIS cable device
        Bit 7: Station Only
        """
        # For a Windows host, we'll set it as Station Only (bit 7)
        capabilities = 0x80  # Station Only
        enabled = 0x80      # Station Only enabled
        return capabilities, enabled

    @staticmethod
    def get_interface_index(interface_name: str) -> int:
        """Get the interface index for a given interface name"""
        try:
            import win32com.client
            wmi = win32com.client.GetObject("winmgmts:")
            interfaces = wmi.InstancesOf("Win32_NetworkAdapter")
            
            for interface in interfaces:
                if interface.NetConnectionID == interface_name or interface.Name == interface_name:
                    return interface.InterfaceIndex or 0
        except Exception:
            pass
        
        # Fallback: use the order in the interface list
        interfaces = SystemInfo.get_interfaces()
        for idx, interface in enumerate(interfaces):
            if interface['name'] == interface_name:
                return idx + 1
        
        return 0

    @staticmethod
    def format_mac_address(mac_bytes: bytes) -> str:
        """Format MAC address bytes as a string"""
        return ':'.join(f'{b:02x}' for b in mac_bytes)

    @staticmethod
    def ip_to_bytes(ip_str: str) -> bytes:
        """Convert IP address string to bytes"""
        try:
            # Try IPv4 first
            return socket.inet_aton(ip_str)
        except socket.error:
            try:
                # Try IPv6
                return socket.inet_pton(socket.AF_INET6, ip_str)
            except socket.error:
                return b''