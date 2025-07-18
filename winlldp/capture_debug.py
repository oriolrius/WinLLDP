#!/usr/bin/env python
"""Debug script for LLDP capture - run this directly to test"""

import sys
import os
import json
import traceback
from datetime import datetime

# Log file for debugging
log_file = os.path.join(os.environ.get('TEMP', '/tmp'), 'winlldp_capture_debug.log')

def log(msg):
    with open(log_file, 'a') as f:
        f.write(f"[{datetime.now().isoformat()}] {msg}\n")
        f.flush()

try:
    log("Starting LLDP capture debug script")
    log(f"Python: {sys.executable}")
    log(f"Version: {sys.version}")
    log(f"Path: {sys.path}")
    
    # Try importing required modules
    log("Importing scapy...")
    from scapy.all import sniff, Ether
    log("Scapy imported successfully")
    
    log("Importing winlldp modules...")
    # Add parent directory to path
    parent_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if parent_dir not in sys.path:
        sys.path.insert(0, parent_dir)
    
    from winlldp.lldp_packet import LLDPPacket
    from winlldp.system_info import SystemInfo
    log("WinLLDP modules imported successfully")
    
    # Get interfaces
    log("Getting network interfaces...")
    system_info = SystemInfo()
    interfaces = system_info.get_interfaces()
    iface_names = [iface['name'] for iface in interfaces if iface['is_up'] and iface['mac']]
    log(f"Found interfaces: {iface_names}")
    
    if not iface_names:
        log("ERROR: No active interfaces found!")
        sys.exit(1)
    
    # Define packet handler
    def process_lldp_packet(packet):
        try:
            if packet.haslayer(Ether) and packet[Ether].type == 0x88cc:
                log(f"Received LLDP packet from {packet[Ether].src}")
                interface = packet.sniffed_on if hasattr(packet, 'sniffed_on') else 'unknown'
                source_mac = packet[Ether].src
                
                lldp_data = bytes(packet)[14:]
                lldp_packet = LLDPPacket.decode(lldp_data)
                neighbor_data = lldp_packet.to_dict()
                
                log(f"Decoded neighbor: {neighbor_data.get('system_name', 'Unknown')}")
                
                # Send to parent process
                output = {
                    'interface': interface,
                    'source_mac': source_mac,
                    'neighbor_data': neighbor_data,
                    'timestamp': datetime.now().isoformat()
                }
                
                print(f"NEIGHBOR:{json.dumps(output)}")
                sys.stdout.flush()
                
        except Exception as e:
            log(f"Error processing packet: {e}")
            log(traceback.format_exc())
    
    # Start sniffing
    log("Starting packet capture...")
    log(f"Filter: ether proto 0x88cc")
    log(f"Interfaces: {', '.join(iface_names)}")
    
    sniff(
        filter='ether proto 0x88cc',
        prn=process_lldp_packet,
        iface=iface_names,
        store=False
    )
    
except Exception as e:
    log(f"FATAL ERROR: {e}")
    log(traceback.format_exc())
    sys.exit(1)