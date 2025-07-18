import click
import time
import os
import tempfile
from tabulate import tabulate
from .config import Config
from .lldp_sender import LLDPSender
from .lldp_receiver import LLDPReceiver
from .system_info import SystemInfo
from .service import WinLLDPService


@click.group()
def cli():
    """Windows LLDP Service CLI"""
    pass


@cli.group()
def capture():
    """LLDP capture management commands"""
    pass


@capture.command('start')
@click.option('--env-file', '-e', help='Path to .env configuration file')
def capture_start(env_file):
    """Start LLDP packet capture subprocess"""
    config = Config(env_file)
    receiver = LLDPReceiver(config)
    if receiver.start_capture():
        click.echo("LLDP capture started successfully")
        click.echo(f"Neighbors file: {config.neighbors_file}")
    else:
        click.echo("Failed to start LLDP capture", err=True)


@capture.command('stop')
@click.option('--env-file', '-e', help='Path to .env configuration file')
def capture_stop(env_file):
    """Stop LLDP packet capture subprocess"""
    config = Config(env_file)
    receiver = LLDPReceiver(config)
    if receiver.stop_capture():
        click.echo("LLDP capture stopped successfully")
    else:
        click.echo("Failed to stop LLDP capture", err=True)


@capture.command('status')
@click.option('--env-file', '-e', help='Path to .env configuration file')
def capture_status(env_file):
    """Show LLDP capture subprocess status"""
    config = Config(env_file)
    receiver = LLDPReceiver(config)
    status = receiver.get_capture_status()
    
    if status['running']:
        click.echo("LLDP Capture Status: RUNNING")
        click.echo(f"  PID: {status['pid']}")
        click.echo(f"  Uptime: {status['uptime']}")
        click.echo(f"  Memory: {status['memory_mb']:.1f} MB")
        click.echo(f"  CPU: {status['cpu_percent']:.1f}%")
    else:
        click.echo("LLDP Capture Status: NOT RUNNING")


@capture.command('log')
@click.option('--lines', '-n', default=20, help='Number of lines to show')
def capture_log(lines):
    """Show capture process log"""
    import os
    import tempfile
    
    log_file = os.path.join(tempfile.gettempdir(), 'winlldp_capture.log')
    
    if not os.path.exists(log_file):
        click.echo("No log file found. Capture may not have been started yet.")
        return
    
    try:
        with open(log_file, 'r') as f:
            all_lines = f.readlines()
            
        if lines > 0:
            # Show last N lines
            display_lines = all_lines[-lines:]
        else:
            # Show all lines
            display_lines = all_lines
            
        for line in display_lines:
            click.echo(line.rstrip())
            
        click.echo(f"\n[Showing last {len(display_lines)} lines of {len(all_lines)} total]")
        
    except Exception as e:
        click.echo(f"Error reading log file: {e}", err=True)


@cli.command()
@click.option('--env-file', '-e', help='Path to .env configuration file')
@click.option('--watch', '-w', is_flag=True, help='Watch neighbors continuously')
def show_neighbors(env_file, watch):
    """Show discovered LLDP neighbors"""
    import json
    from datetime import datetime, timedelta
    
    config = Config(env_file)
    neighbors_file = config.neighbors_file
    
    def display_neighbors():
        """Display neighbors from JSON file"""
        if not os.path.exists(neighbors_file):
            click.echo("No neighbors file found. Make sure capture is running.")
            return False
        
        try:
            with open(neighbors_file, 'r') as f:
                neighbors_data = json.load(f)
        except json.JSONDecodeError:
            click.echo("Error reading neighbors file. It may be corrupted.")
            return False
        except Exception as e:
            click.echo(f"Error: {e}")
            return False
        
        # Convert to list and calculate age
        neighbors = []
        now = datetime.now()
        
        for key, neighbor in neighbors_data.items():
            # Calculate age
            first_seen = datetime.fromisoformat(neighbor['first_seen'])
            age = now - first_seen
            
            # Format age
            if age.days > 0:
                age_str = f"{age.days}d {age.seconds // 3600}h"
            elif age.seconds >= 3600:
                age_str = f"{age.seconds // 3600}h {(age.seconds % 3600) // 60}m"
            else:
                age_str = f"{age.seconds // 60}m {age.seconds % 60}s"
            
            # Get neighbor data
            data = neighbor['data']
            
            neighbors.append({
                'interface': neighbor['interface'],
                'chassis_id': data.get('chassis_id', 'Unknown'),
                'system_name': data.get('system_name', 'Unknown'),
                'port': data.get('port_description', data.get('port_id', 'Unknown')),
                'age': age_str,
                'ttl': f"{data.get('ttl', 120)}s",
                'management_ip': data.get('management_address', 'N/A')
            })
        
        if neighbors:
            headers = [
                'Interface', 'Neighbor', 'System Name', 
                'Port', 'Age', 'TTL', 'Management IP'
            ]
            
            rows = []
            for n in neighbors:
                rows.append([
                    n['interface'],
                    n['chassis_id'],
                    n['system_name'],
                    n['port'],
                    n['age'],
                    n['ttl'],
                    n['management_ip']
                ])
            
            click.echo("LLDP Neighbors:")
            click.echo(tabulate(rows, headers=headers, tablefmt='grid'))
            click.echo(f"\nTotal neighbors: {len(neighbors)}")
        else:
            click.echo("No LLDP neighbors in file.")
        
        return True
    
    if watch:
        click.echo("Watching LLDP neighbors... (Press Ctrl+C to exit)")
        try:
            while True:
                click.clear()
                display_neighbors()
                click.echo("\nPress Ctrl+C to exit")
                time.sleep(5)
        except KeyboardInterrupt:
            click.echo("\nExiting...")
    else:
        display_neighbors()


@cli.command()
@click.option('--env-file', '-e', help='Path to .env configuration file')
@click.option('--interface', '-i', help='Specific interface to send on')
@click.option('--verbose', '-v', is_flag=True, help='Verbose output')
def send(env_file, interface, verbose):
    """Send LLDP packets immediately"""
    config = Config(env_file)
    
    # Override interface if specified
    if interface:
        config.interface = interface
    
    sender = LLDPSender(config)
    
    if verbose:
        click.echo(f"Configuration:")
        click.echo(f"  Interface: {config.interface}")
        click.echo(f"  System Name: {config.system_name}")
        click.echo(f"  TTL: {config.ttl}")
        click.echo("")
    
    click.echo("Sending LLDP packets...")
    sender.send_once(verbose=verbose)
    click.echo("LLDP packets sent successfully!")


@cli.command()
def show_interfaces():
    """Show network interfaces"""
    system_info = SystemInfo()
    interfaces = system_info.get_interfaces()
    
    headers = ['Interface', 'MAC Address', 'Status', 'IPv4 Addresses']
    rows = []
    
    for interface in interfaces:
        if interface['mac']:
            rows.append([
                interface['name'],
                interface['mac'],
                'UP' if interface['is_up'] else 'DOWN',
                ', '.join(interface['ipv4']) if interface['ipv4'] else 'N/A'
            ])
    
    click.echo("Network Interfaces:")
    click.echo(tabulate(rows, headers=headers, tablefmt='grid'))


@cli.command()
@click.option('--env-file', '-e', help='Path to .env configuration file')
def show_config(env_file):
    """Show current configuration"""
    try:
        config = Config(env_file)
        click.echo("Current Configuration:")
        click.echo(str(config))
    except Exception as e:
        click.echo(f"Error loading configuration: {e}", err=True)


@cli.command()
@click.option('--env-file', '-e', help='Path to .env configuration file')
@click.option('--monitor', is_flag=True, help='Enable advanced monitoring (may cause issues)')
def run(env_file, monitor):
    """Run full LLDP service (sender + receiver)"""
    config = Config(env_file)
    
    # Use simple runner by default (more reliable)
    if not monitor:
        from .cli_run import run_simple
        run_simple(config)
        return
    
    # Advanced monitoring mode (may have responsiveness issues)
    click.echo("Warning: Monitor mode may become unresponsive. Use --simple for better stability.")
    
    receiver = LLDPReceiver(config)
    sender = LLDPSender(config)
    
    click.echo("Starting Windows LLDP Service...")
    click.echo(f"Configuration: {config}")
    click.echo("\nPress Ctrl+C to stop")
    
    # Check if capture is already running
    if receiver.is_capture_running():
        click.echo("Note: Capture subprocess is already running")
    else:
        # Start capture subprocess
        click.echo("Starting capture subprocess...")
        if not receiver.start_capture():
            click.echo("Warning: Failed to start capture subprocess", err=True)
    
    # Start sender thread
    click.echo("Starting LLDP sender...")
    sender.start()
    click.echo("LLDP sender started")
    
    # Get log file path
    import tempfile
    log_file = os.path.join(tempfile.gettempdir(), 'winlldp_capture.log')
    if not no_log:
        click.echo(f"Monitoring capture log: {log_file}")
    click.echo("\nPress Ctrl+C to stop\n")
    
    try:
        # Keep running and show status periodically
        last_log_pos = 0
        status_counter = 0
        last_status_time = time.time()
        
        while True:
            # Check for log updates (if enabled)
            if not no_log and os.path.exists(log_file):
                try:
                    with open(log_file, 'r') as f:
                        f.seek(last_log_pos)
                        new_lines = f.readlines()
                        if new_lines:
                            last_log_pos = f.tell()
                            
                            for line in new_lines:
                                if "Received LLDP packet" in line or "ERROR" in line or "WARNING" in line:
                                    # Extract timestamp and message
                                    parts = line.strip().split('] ', 1)
                                    if len(parts) == 2:
                                        timestamp = parts[0].replace('[', '')
                                        message = parts[1]
                                        click.echo(f"[{timestamp}] {message}")
                except Exception as e:
                    # Don't let file errors break the loop
                    pass
            
            # Show status every 30 seconds
            current_time = time.time()
            if current_time - last_status_time >= 30:
                last_status_time = current_time
                try:
                    if receiver.is_capture_running():
                        neighbors = receiver.get_neighbors()
                        click.echo(f"\n[Status] Service running... {len(neighbors)} neighbors discovered")
                        
                        # Show summary of neighbors
                        if neighbors:
                            click.echo("[Status] Current neighbors:")
                            for n in neighbors[:5]:  # Show first 5
                                click.echo(f"  - {n.get('system_name', 'Unknown')} ({n.get('chassis_id', 'Unknown')}) on {n.get('interface', 'Unknown')}")
                            if len(neighbors) > 5:
                                click.echo(f"  ... and {len(neighbors) - 5} more")
                    else:
                        click.echo("\n[Status] Service running... (capture not active)")
                except Exception as e:
                    click.echo(f"\n[Status] Error getting status: {e}")
            
            # Sleep briefly to allow Ctrl+C to work
            time.sleep(0.5)
            
    except KeyboardInterrupt:
        click.echo("\nStopping service...")
        sender.stop()
        click.echo("LLDP sender stopped")
        click.echo("Note: Capture subprocess continues running in background")
        click.echo("Use 'winlldp capture stop' to stop it")
    except Exception as e:
        click.echo(f"\nError: {e}")
        sender.stop()


@cli.command()
@click.option('--env-file', '-e', help='Path to .env configuration file')
def debug_paths(env_file):
    """Show debug information about file paths"""
    config = Config(env_file)
    neighbors_file = config.neighbors_file
    pid_file = os.path.join(os.path.dirname(neighbors_file), 'capture.pid')
    log_file = os.path.join(tempfile.gettempdir(), 'winlldp_capture.log')
    
    click.echo("Configuration:")
    click.echo(f"  Config file: {env_file or '.env (default)'}")
    click.echo(f"  LLDP_NEIGHBORS_FILE: {os.getenv('LLDP_NEIGHBORS_FILE', 'neighbors.json (default)')}")
    
    click.echo("\nFile paths:")
    click.echo(f"  Neighbors file: {neighbors_file}")
    click.echo(f"  Exists: {os.path.exists(neighbors_file)}")
    if os.path.exists(neighbors_file):
        click.echo(f"  Size: {os.path.getsize(neighbors_file)} bytes")
    click.echo(f"\n  PID file: {pid_file}")
    click.echo(f"  Exists: {os.path.exists(pid_file)}")
    click.echo(f"\n  Log file: {log_file}")
    click.echo(f"  Exists: {os.path.exists(log_file)}")


@cli.command()
@click.option('--env-file', '-e', help='Path to .env configuration file')
def clear_neighbors(env_file):
    """Clear all discovered neighbors"""
    config = Config(env_file)
    neighbors_file = config.neighbors_file
    
    try:
        if os.path.exists(neighbors_file):
            # Just write an empty dictionary to the file
            with open(neighbors_file, 'w') as f:
                f.write('{}')
            click.echo(f"Cleared all discovered neighbors from: {neighbors_file}")
        else:
            click.echo(f"No neighbors file found at: {neighbors_file}")
    except Exception as e:
        click.echo(f"Error clearing neighbors: {e}", err=True)


@cli.group()
def service():
    """Windows service management commands"""
    pass


@service.command('install')
def service_install():
    """Install as Windows service"""
    import subprocess
    import sys
    import os
    
    try:
        # Check if NSSM is available
        result = subprocess.run(['nssm', 'version'], capture_output=True, text=True)
        if result.returncode != 0:
            click.echo("Error: NSSM not found. Please install it first:", err=True)
            click.echo("  winget install NSSM")
            click.echo("Or download from https://nssm.cc/")
            return
    except FileNotFoundError:
        click.echo("Error: NSSM not found. Please install it first:", err=True)
        click.echo("  winget install NSSM")
        click.echo("Or download from https://nssm.cc/")
        return
    
    # Get paths
    python_exe = sys.executable
    project_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    run_service_script = os.path.join(project_dir, 'run_service.py')
    
    # Check if run_service.py exists
    if not os.path.exists(run_service_script):
        click.echo(f"Error: {run_service_script} not found", err=True)
        return
    
    click.echo("Installing WinLLDP service...")
    
    # Install the service
    result = subprocess.run(['nssm', 'install', 'WinLLDP'], capture_output=True, text=True)
    if result.returncode != 0 and "already exists" not in result.stderr:
        click.echo(f"Error installing service: {result.stderr}", err=True)
        return
    elif "already exists" in result.stderr:
        click.echo("Service already exists, updating configuration...")
    
    # Configure the service
    commands = [
        ['nssm', 'set', 'WinLLDP', 'Application', python_exe],
        ['nssm', 'set', 'WinLLDP', 'AppParameters', run_service_script],
        ['nssm', 'set', 'WinLLDP', 'AppDirectory', project_dir],
        ['nssm', 'set', 'WinLLDP', 'DisplayName', 'Windows LLDP Service'],
        ['nssm', 'set', 'WinLLDP', 'Description', 'Link Layer Discovery Protocol service for Windows'],
        ['nssm', 'set', 'WinLLDP', 'Start', 'SERVICE_AUTO_START'],
    ]
    
    for cmd in commands:
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            click.echo(f"Error setting parameter: {result.stderr}", err=True)
            return
    
    click.echo("Service installed successfully!")
    click.echo(f"  Python: {python_exe}")
    click.echo(f"  Script: {run_service_script}")
    click.echo(f"  Directory: {project_dir}")
    click.echo("")
    click.echo("To start the service, run: winlldp service start")


@service.command('uninstall')
def service_uninstall():
    """Uninstall Windows service"""
    import subprocess
    
    try:
        # Check if service exists
        result = subprocess.run(['nssm', 'status', 'WinLLDP'], capture_output=True, text=True)
        if result.returncode != 0:
            click.echo("Service WinLLDP not found")
            return
    except FileNotFoundError:
        click.echo("Error: NSSM not found", err=True)
        return
    
    # Confirm uninstallation
    if not click.confirm("Are you sure you want to uninstall the WinLLDP service?"):
        click.echo("Uninstallation cancelled")
        return
    
    # Stop service first if running
    subprocess.run(['nssm', 'stop', 'WinLLDP'], capture_output=True)
    
    # Remove the service
    result = subprocess.run(['nssm', 'remove', 'WinLLDP', 'confirm'], capture_output=True, text=True)
    if result.returncode == 0:
        click.echo("Service uninstalled successfully")
    else:
        click.echo(f"Error uninstalling service: {result.stderr}", err=True)


@service.command('start')
def service_start():
    """Start Windows service"""
    import subprocess
    
    try:
        click.echo("Starting WinLLDP service...")
        result = subprocess.run(['nssm', 'start', 'WinLLDP'], capture_output=True, text=True)
        
        if result.returncode == 0:
            click.echo("Service started successfully")
        else:
            if "already running" in result.stderr:
                click.echo("Service is already running")
            else:
                click.echo(f"Error starting service: {result.stderr}", err=True)
    except FileNotFoundError:
        click.echo("Error: NSSM not found. Please install the service first.", err=True)
        click.echo("Run: winlldp service install")


@service.command('stop')
def service_stop():
    """Stop Windows service"""
    import subprocess
    
    try:
        click.echo("Stopping WinLLDP service...")
        result = subprocess.run(['nssm', 'stop', 'WinLLDP'], capture_output=True, text=True)
        
        if result.returncode == 0:
            click.echo("Service stopped successfully")
        else:
            if "not running" in result.stderr:
                click.echo("Service is not running")
            else:
                click.echo(f"Error stopping service: {result.stderr}", err=True)
    except FileNotFoundError:
        click.echo("Error: NSSM not found", err=True)


@service.command('status')
def service_status():
    """Show Windows service status"""
    import subprocess
    import os
    
    try:
        result = subprocess.run(['nssm', 'status', 'WinLLDP'], capture_output=True, text=True)
        
        if result.returncode == 0:
            status = result.stdout.strip()
            click.echo(f"WinLLDP service status: {status}")
            
            if status == "SERVICE_RUNNING":
                click.echo("  The service is running")
                
                # Show additional info from log
                project_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
                log_file = os.path.join(project_dir, 'nssm_service.log')
                if os.path.exists(log_file):
                    # Get last line with status
                    try:
                        with open(log_file, 'r', encoding='utf-8') as f:
                            lines = f.readlines()
                            for line in reversed(lines):
                                if "Service status:" in line:
                                    click.echo(f"  Last status: {line.strip()}")
                                    break
                    except:
                        pass
            elif status == "SERVICE_STOPPED":
                click.echo("  The service is stopped")
            elif status == "SERVICE_PAUSED":
                click.echo("  The service is paused")
            else:
                click.echo(f"  Unknown status: {status}")
        else:
            click.echo("Service WinLLDP not found")
            click.echo("Run 'winlldp service install' to install the service")
    except FileNotFoundError:
        click.echo("Error: NSSM not found. Please install it first:", err=True)
        click.echo("  winget install NSSM")


@service.command('restart')
def service_restart():
    """Restart Windows service"""
    import subprocess
    
    try:
        click.echo("Restarting WinLLDP service...")
        result = subprocess.run(['nssm', 'restart', 'WinLLDP'], capture_output=True, text=True)
        
        if result.returncode == 0:
            click.echo("Service restarted successfully")
        else:
            click.echo(f"Error restarting service: {result.stderr}", err=True)
    except FileNotFoundError:
        click.echo("Error: NSSM not found", err=True)


if __name__ == '__main__':
    cli()