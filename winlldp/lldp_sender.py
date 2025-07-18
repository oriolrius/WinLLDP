import time
import threading
import socket
import struct
import platform
from typing import List, Optional
from scapy.all import sendp, Ether
from .lldp_packet import (
    LLDPPacket, 
    ChassisIdSubtype, 
    PortIdSubtype,
    TLVType
)
from .system_info import SystemInfo
from .config import Config
from .logger import get_logger


class LLDPSender:
    LLDP_MULTICAST_MAC = "01:80:c2:00:00:0e"
    LLDP_ETHERTYPE = 0x88cc

    def __init__(self, config: Config):
        self.config = config
        self.running = False
        self.thread = None
        self.system_info = SystemInfo()
        self.logger = get_logger()

    def create_lldp_packet(self, interface_info: dict, minimal: bool = False) -> bytes:
        """Create an LLDP packet for a specific interface. If minimal=True, only required TLVs are included."""
        packet = LLDPPacket()
        # Chassis ID (MAC address)
        if interface_info['mac']:
            mac_bytes = self.system_info.get_mac_address_bytes(interface_info['mac'])
            packet.add_chassis_id(ChassisIdSubtype.MAC_ADDRESS, mac_bytes)
        # Port ID (MAC address)
        if interface_info['mac']:
            mac_bytes = self.system_info.get_mac_address_bytes(interface_info['mac'])
            packet.add_port_id(PortIdSubtype.MAC_ADDRESS, mac_bytes)
        # TTL
        packet.add_ttl(self.config.ttl)
        if not minimal:
            # Port Description (optional, can be used as platform info)
            port_desc = self.config.port_description or interface_info['name']
            packet.add_port_description(port_desc)
            # System Name (Identity)
            system_name = self.config.system_name
            if system_name == 'auto':
                system_name = self.system_info.get_hostname()
            packet.add_system_name(system_name)
            # System Description (Platform)
            # Always use detailed Windows platform/version info for System Description
            system_desc = self.system_info.get_system_description()
            packet.add_system_description(system_desc)
            # Add LLDP-MED TLVs that MikroTik might parse for platform/version
            # LLDP-MED uses TIA OUI: 00-12-BB
            lldp_med_oui = b'\x00\x12\xBB'
            
            # LLDP-MED Capabilities TLV (subtype 1)
            # Indicates this is an endpoint device
            med_caps = struct.pack('!HBB', 
                0x0001,  # Capabilities: LLDP-MED capabilities supported
                0x03,    # Device type: 3 = Endpoint Class III
                0        # Reserved
            )
            packet.add_organizationally_specific(lldp_med_oui, 1, med_caps)
            
            # Hardware Revision TLV (subtype 5) - use for platform
            hw_revision = platform.machine().encode('utf-8')  # e.g., "AMD64"
            packet.add_organizationally_specific(lldp_med_oui, 5, hw_revision)
            
            # Firmware Revision TLV (subtype 6) - use for version
            fw_revision = platform.version().encode('utf-8')  # e.g., "10.0.26100"
            packet.add_organizationally_specific(lldp_med_oui, 6, fw_revision)
            
            # Software Revision TLV (subtype 7) - additional version info
            sw_revision = f"Python {platform.python_version()}".encode('utf-8')
            packet.add_organizationally_specific(lldp_med_oui, 7, sw_revision)
            
            # Model Name TLV (subtype 9) - use Windows edition
            try:
                import subprocess
                result = subprocess.run(['wmic', 'os', 'get', 'Caption', '/value'], 
                                      capture_output=True, text=True, shell=True)
                if result.returncode == 0:
                    for line in result.stdout.split('\n'):
                        if line.startswith('Caption='):
                            model = line.split('=', 1)[1].strip()
                            packet.add_organizationally_specific(lldp_med_oui, 9, model.encode('utf-8'))
                            break
            except:
                pass
            # System Capabilities (Version info)
            capabilities, enabled = self.system_info.get_system_capabilities()
            packet.add_system_capabilities(capabilities, enabled)
            # Management Address TLV enabled
            mgmt_addr = self.config.management_address
            if mgmt_addr == 'auto':
                if interface_info['ipv4']:
                    mgmt_addr = interface_info['ipv4'][0]
                else:
                    mgmt_addr = self.system_info.get_primary_ip()
            if mgmt_addr:
                addr_bytes = self.system_info.ip_to_bytes(mgmt_addr)
                if addr_bytes:
                    interface_idx = self.system_info.get_interface_index(interface_info['name'])
                    packet.add_management_address(1, addr_bytes, interface_idx)  # 1 = IPv4
        # End of LLDPDU
        packet.add_end_of_lldpdu()
        return packet.encode()

    def send_lldp_on_interface(self, interface_info: dict, verbose: bool = False):
        """Send LLDP packet on a specific interface"""
        if not interface_info['mac'] or not interface_info['is_up']:
            return
        try:
            # Use minimal TLVs if configured
            minimal = getattr(self.config, 'minimal_tlv', False)
            lldp_data = self.create_lldp_packet(interface_info, minimal=minimal)
            if verbose:
                self.logger.debug(f"Sending LLDP on {interface_info['name']} ({interface_info['mac']})")
                self.logger.debug(f"  Destination: {self.LLDP_MULTICAST_MAC}")
                self.logger.debug(f"  Packet size: {len(lldp_data)} bytes")
                self.logger.debug(f"  Raw LLDP data: {lldp_data.hex()}")
            ether = Ether(
                dst=self.LLDP_MULTICAST_MAC,
                src=interface_info['mac'],
                type=self.LLDP_ETHERTYPE
            )
            frame = ether / lldp_data
            sendp(frame, iface=interface_info['name'], verbose=verbose)
            if verbose:
                self.logger.debug(f"  Sent successfully!")
        except Exception as e:
            self.logger.error(f"Error sending LLDP on {interface_info['name']}: {e}")

    def send_lldp(self, verbose: bool = False):
        """Send LLDP packets on all configured interfaces"""
        interfaces = self.system_info.get_interfaces()
        
        if self.config.interface == 'all':
            # Send on all active interfaces with MAC addresses
            for interface in interfaces:
                if interface['is_up'] and interface['mac']:
                    self.send_lldp_on_interface(interface, verbose)
        else:
            # Send on specific interface
            interface = self.system_info.get_interface_by_name(self.config.interface)
            if interface:
                self.send_lldp_on_interface(interface, verbose)
            else:
                self.logger.error(f"Interface {self.config.interface} not found")

    def _sender_loop(self):
        """Main sender loop"""
        while self.running:
            try:
                self.send_lldp()
                # Sleep in small intervals to allow quick shutdown
                for _ in range(self.config.interval * 10):
                    if not self.running:
                        break
                    time.sleep(0.1)
            except Exception as e:
                self.logger.error(f"Error in LLDP sender loop: {e}")
                # Wait before retrying, but check running flag
                for _ in range(50):  # 5 seconds in 0.1s intervals
                    if not self.running:
                        break
                    time.sleep(0.1)

    def start(self):
        """Start the LLDP sender thread"""
        if self.running:
            return
        
        self.running = True
        self.thread = threading.Thread(target=self._sender_loop, daemon=True)
        self.thread.start()

    def stop(self):
        """Stop the LLDP sender thread"""
        self.running = False
        if self.thread:
            self.thread.join(timeout=5)

    def send_once(self, verbose: bool = False):
        """Send LLDP packets once (for CLI command)"""
        self.send_lldp(verbose)