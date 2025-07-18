#!/usr/bin/env python
"""
LLDP capture subprocess module
This module runs as a separate process to capture LLDP packets
"""
import sys
import json
import os
import traceback
from datetime import datetime

def main():
    # Get arguments
    if len(sys.argv) < 4:
        print("Usage: capture_subprocess.py <log_file> <neighbors_file> <pid_file>", file=sys.stderr)
        sys.exit(1)
    
    log_file = sys.argv[1]
    neighbors_file = sys.argv[2] 
    pid_file = sys.argv[3]
    
    def log(msg):
        try:
            with open(log_file, 'a') as f:
                f.write(f"[{datetime.now().isoformat()}] {msg}\n")
                f.flush()
        except:
            pass
    
    def log_file_op(operation, filepath, mode=''):
        # Log file operations in the capture subprocess
        if mode:
            log(f"FILE_OP: {operation} '{filepath}' mode={mode}")
        else:
            log(f"FILE_OP: {operation} '{filepath}'")
    
    try:
        log("Starting LLDP capture process")
        log(f"PID: {os.getpid()}")
        log(f"Python: {sys.executable}")
        log(f"Neighbors file will be: {neighbors_file}")
        log(f"PID file will be: {pid_file}")
        
        # Write PID
        log_file_op("OPEN", pid_file, 'w')
        with open(pid_file, 'w') as f:
            f.write(str(os.getpid()))
        log_file_op("CLOSE", pid_file)
        log("PID file written")
        
        # Import required modules
        log("Importing modules...")
        from scapy.all import sniff, Ether
        from winlldp.lldp_packet import LLDPPacket
        from winlldp.system_info import SystemInfo
        log("Modules imported successfully")
        
        def process_lldp_packet(packet):
            try:
                if packet.haslayer(Ether) and packet[Ether].type == 0x88cc:
                    interface = packet.sniffed_on if hasattr(packet, 'sniffed_on') else 'unknown'
                    source_mac = packet[Ether].src
                    log(f"Received LLDP packet from {source_mac} on {interface}")
                    
                    lldp_data = bytes(packet)[14:]
                    lldp_packet = LLDPPacket.decode(lldp_data)
                    neighbor_data = lldp_packet.to_dict()
                    
                    # Save neighbor data directly to file
                    log(f"Saving to file: {neighbors_file}")
                    
                    # Load existing neighbors
                    try:
                        log_file_op("OPEN", neighbors_file, 'r')
                        with open(neighbors_file, 'r') as f:
                            neighbors = json.load(f)
                        log_file_op("CLOSE", neighbors_file)
                    except:
                        neighbors = {}
                        log("No existing neighbors file, creating new one")
                    
                    # Create neighbor key
                    chassis_id = neighbor_data.get('chassis_id', '')
                    key = f"{interface}:{source_mac}:{chassis_id}"
                    
                    # Update neighbor
                    neighbors[key] = {
                        'interface': interface,
                        'source_mac': source_mac,
                        'data': neighbor_data,
                        'first_seen': neighbors.get(key, {}).get('first_seen', datetime.now().isoformat()),
                        'last_seen': datetime.now().isoformat(),
                        'ttl': neighbor_data.get('ttl', 120)
                    }
                    
                    # Save back to file
                    log_file_op("OPEN", neighbors_file, 'w')
                    with open(neighbors_file, 'w') as f:
                        json.dump(neighbors, f, indent=2)
                    log_file_op("CLOSE", neighbors_file)
                    
                    log(f"Saved neighbor data to file ({len(neighbors)} total neighbors)")
                    
            except Exception as e:
                log(f"Error processing packet: {e}")
                log(traceback.format_exc())
        
        # Get all interfaces
        log("Getting network interfaces...")
        system_info = SystemInfo()
        interfaces = system_info.get_interfaces()
        iface_names = [iface['name'] for iface in interfaces if iface['is_up'] and iface['mac']]
        log(f"Found interfaces: {iface_names}")
        
        if iface_names:
            print(f"Capturing on interfaces: {', '.join(iface_names)}")
            sys.stdout.flush()
            log(f"Starting packet capture on: {', '.join(iface_names)}")
            
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
                log(f"Capture error: {e}")
                log(traceback.format_exc())
        else:
            log("ERROR: No active interfaces found!")
            
    except Exception as e:
        log(f"FATAL ERROR: {e}")
        log(traceback.format_exc())
    finally:
        # Clean up PID file
        try:
            log_file_op("REMOVE", pid_file)
            os.remove(pid_file)
            log("PID file removed")
        except:
            pass
        log("Capture process exiting")

if __name__ == '__main__':
    main()