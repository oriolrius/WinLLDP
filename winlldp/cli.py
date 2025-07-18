import click
import time
import os
import tempfile
import subprocess
import sys
from tabulate import tabulate
from .config import Config
from .lldp_sender import LLDPSender
from .lldp_receiver import LLDPReceiver
from .system_info import SystemInfo
from .file_debug import debug_open as open
from .file_debug import set_verbose as set_file_verbose


def setup_logging(verbose):
    """Set up logging based on verbose flag"""
    import logging
    
    if verbose:
        logging.basicConfig(
            level=logging.DEBUG, 
            format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
            force=True
        )
        set_file_verbose(True)
    else:
        logging.basicConfig(
            level=logging.INFO, 
            format='%(levelname)s:%(name)s:%(message)s',
            force=True
        )
        set_file_verbose(False)


@click.group()
@click.pass_context
def cli(ctx):
    """Windows LLDP Service CLI"""
    # Store context for subcommands
    ctx.ensure_object(dict)
    pass


@cli.command()
def version():
    """Show WinLLDP version"""
    from winlldp import __version__
    click.echo(f"WinLLDP version {__version__}")


@cli.group()
def capture():
    """LLDP capture management commands"""
    pass


@capture.command('start')
@click.option('--env-file', '-e', help='Path to .env configuration file')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def capture_start(env_file, verbose):
    """Start LLDP packet capture subprocess"""
    setup_logging(verbose)
    
    if verbose:
        click.echo("Verbose mode enabled")
    
    config = Config(env_file)
    receiver = LLDPReceiver(config)
    
    if verbose:
        click.echo(f"Configuration loaded:")
        click.echo(f"  Neighbors file: {config.neighbors_file}")
        click.echo(f"  Capture interval: {config.interval}s")
        click.echo(f"  TTL: {config.ttl}s")
    
    if receiver.start_capture():
        click.echo("LLDP capture started successfully")
        if not verbose:
            click.echo(f"Neighbors file: {config.neighbors_file}")
    else:
        click.echo("Failed to start LLDP capture", err=True)


@capture.command('stop')
@click.option('--env-file', '-e', help='Path to .env configuration file')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def capture_stop(env_file, verbose):
    """Stop LLDP packet capture subprocess"""
    setup_logging(verbose)
    config = Config(env_file)
    receiver = LLDPReceiver(config)
    if receiver.stop_capture():
        click.echo("LLDP capture stopped successfully")
    else:
        click.echo("Failed to stop LLDP capture", err=True)


@capture.command('status')
@click.option('--env-file', '-e', help='Path to .env configuration file')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def capture_status(env_file, verbose):
    """Show LLDP capture subprocess status"""
    setup_logging(verbose)
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
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def capture_log(lines, verbose):
    """Show capture process log"""
    setup_logging(verbose)
    import os
    from .paths import get_log_file
    
    log_file = get_log_file()
    
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
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def show_neighbors(env_file, watch, verbose):
    """Show discovered LLDP neighbors"""
    setup_logging(verbose)
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
    setup_logging(verbose)
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
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def show_interfaces(verbose):
    """Show network interfaces"""
    setup_logging(verbose)
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
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def show_config(env_file, verbose):
    """Show current configuration"""
    setup_logging(verbose)
    try:
        config = Config(env_file)
        click.echo("Current Configuration:")
        click.echo(str(config))
    except Exception as e:
        click.echo(f"Error loading configuration: {e}", err=True)




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
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def service_install(verbose):
    """Install as Windows service"""
    setup_logging(verbose)
    
    try:
        # Check if NSSM is available
        if verbose:
            click.echo("Checking for NSSM installation...")
        result = subprocess.run(['nssm', 'version'], capture_output=True, text=True)
        if result.returncode != 0:
            click.echo("Error: NSSM not found. Please install it first:", err=True)
            click.echo("  winget install NSSM")
            click.echo("Or download from https://nssm.cc/")
            return
        if verbose:
            click.echo(f"NSSM version: {result.stdout.strip()}")
    except FileNotFoundError:
        click.echo("Error: NSSM not found. Please install it first:", err=True)
        click.echo("  winget install NSSM")
        click.echo("Or download from https://nssm.cc/")
        return
    
    # Get paths
    python_exe = sys.executable
    
    # Determine if we're running from a frozen executable
    if getattr(sys, 'frozen', False):
        # Running from PyInstaller bundle
        project_dir = os.path.dirname(python_exe)
        service_args = 'service-run'
    else:
        # Normal Python execution
        project_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        service_args = '-m winlldp.service_wrapper'
    
    if verbose:
        click.echo("Installation details:")
        click.echo(f"  Executable: {python_exe}")
        click.echo(f"  Working directory: {project_dir}")
        click.echo(f"  Service command: {python_exe} {service_args}")
        click.echo(f"  Frozen executable: {getattr(sys, 'frozen', False)}")
    
    click.echo("Installing WinLLDP service...")
    
    # First check if service exists
    if verbose:
        click.echo("Checking if service already exists...")
    check_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
    service_exists = check_result.returncode == 0
    
    if service_exists:
        click.echo("Service already exists, updating configuration...")
        if verbose:
            click.echo("Stopping existing service...")
        # Stop the service first
        result = subprocess.run(['nssm', 'stop', 'WinLLDP'], capture_output=True, text=True)
        if verbose:
            click.echo(f"  Stop command result: {result.returncode}")
            if result.stdout:
                click.echo(f"  Output: {result.stdout.strip()}")
        # Wait for service to stop
        click.echo("Waiting for service to stop...", nl=False)
        for i in range(30):
            status_check = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
            if 'STOPPED' in status_check.stdout or status_check.returncode != 0:
                click.echo(" OK")
                break
            click.echo(".", nl=False)
            time.sleep(0.5)
        else:
            click.echo(" TIMEOUT")
        
        # Remove the service
        if verbose:
            click.echo("Removing existing service...")
        result = subprocess.run(['nssm', 'remove', 'WinLLDP', 'confirm'], capture_output=True, text=True)
        if verbose:
            click.echo(f"  Remove command result: {result.returncode}")
            if result.stderr:
                click.echo(f"  Error: {result.stderr.strip()}")
        # Wait for removal
        click.echo("Waiting for service removal...", nl=False)
        for i in range(20):
            check = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
            if check.returncode != 0:  # Service not found = removed
                click.echo(" OK")
                break
            click.echo(".", nl=False)
            time.sleep(0.5)
        else:
            click.echo(" TIMEOUT")
    
    # Install the service with all parameters
    if getattr(sys, 'frozen', False):
        # For frozen executable, use the service-run command
        install_cmd = [
            'nssm', 'install', 'WinLLDP',
            python_exe,
            'service-run'
        ]
    else:
        # For normal Python, use module execution
        install_cmd = [
            'nssm', 'install', 'WinLLDP',
            python_exe,
            '-m', 'winlldp.service_wrapper'
        ]
    
    if verbose:
        click.echo("Installing service with NSSM...")
        click.echo(f"  Command: {' '.join(install_cmd)}")
    
    result = subprocess.run(install_cmd, capture_output=True, text=True)
    if result.returncode != 0:
        click.echo(f"Error installing service: {result.stderr}", err=True)
        return
    
    if verbose:
        click.echo(f"  Installation result: {result.returncode}")
        if result.stdout:
            click.echo(f"  Output: {result.stdout.strip()}")
    
    # Wait for service to be registered in Windows
    click.echo("Waiting for service registration...", nl=False)
    max_attempts = 30  # 30 seconds timeout
    for i in range(max_attempts):
        check = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
        if check.returncode == 0:
            click.echo(" OK")
            break
        click.echo(".", nl=False)
        time.sleep(1)
    else:
        click.echo(" TIMEOUT")
        click.echo("Error: Service installation timed out", err=True)
        return
    
    if verbose:
        click.echo("Configuring service parameters...")
    
    commands = [
        ['nssm', 'set', 'WinLLDP', 'AppDirectory', project_dir],
        ['nssm', 'set', 'WinLLDP', 'DisplayName', 'Windows LLDP Service'],
        ['nssm', 'set', 'WinLLDP', 'Description', 'Link Layer Discovery Protocol service for Windows'],
        ['nssm', 'set', 'WinLLDP', 'Start', 'SERVICE_AUTO_START'],
        ['nssm', 'set', 'WinLLDP', 'AppNoConsole', '1'],
        ['nssm', 'set', 'WinLLDP', 'AppRestartDelay', '5000'],
        ['nssm', 'set', 'WinLLDP', 'AppThrottle', '1500'],
        ['nssm', 'set', 'WinLLDP', 'AppRotateOnline', '1'],
        ['nssm', 'set', 'WinLLDP', 'AppRotateBytes', str(10 * 1024 * 1024)],
        ['nssm', 'set', 'WinLLDP', 'AppStopMethodSkip', '0'],
]

    for cmd in commands:
        if verbose:
            param_name = cmd[3]
            param_value = cmd[4] if len(cmd) > 4 else ''
            click.echo(f"  Setting {param_name}: {param_value}")
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            # Don't fail on description setting errors (common on some Windows versions)
            if 'Description' not in ' '.join(cmd):
                click.echo(f"Warning: {' '.join(cmd[3:5])}: {result.stderr.strip()}")
    
    click.echo("Service installed successfully!")
    click.echo(f"  Executable: {python_exe}")
    if getattr(sys, 'frozen', False):
        click.echo(f"  Command: service-run")
    else:
        click.echo(f"  Module: winlldp.service_wrapper")
    click.echo(f"  Working directory: {project_dir}")
    click.echo("")
    click.echo("To start the service, run: winlldp service start")


@service.command('uninstall')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def service_uninstall(verbose):
    """Uninstall Windows service"""
    setup_logging(verbose)
    import subprocess
    
    try:
        # Check if service exists
        if verbose:
            click.echo("Checking if service exists...")
        result = subprocess.run(['nssm', 'status', 'WinLLDP'], capture_output=True, text=True)
        if result.returncode != 0:
            click.echo("Service WinLLDP not found")
            return
        
        if verbose:
            click.echo(f"Current service status: {result.stdout.strip()}")
            
            # Get service configuration before removal
            click.echo("\nService configuration before removal:")
            config_items = ['Application', 'AppDirectory', 'AppParameters']
            for item in config_items:
                cmd_result = subprocess.run(['nssm', 'get', 'WinLLDP', item], capture_output=True, text=True)
                if cmd_result.returncode == 0:
                    click.echo(f"  {item}: {cmd_result.stdout.strip()}")
        
    except FileNotFoundError:
        click.echo("Error: NSSM not found", err=True)
        return
    
    # Confirm uninstallation
    if not click.confirm("Are you sure you want to uninstall the WinLLDP service?"):
        click.echo("Uninstallation cancelled")
        return
    
    # Stop service first if running
    if verbose:
        click.echo("\nStopping service before removal...")
    stop_result = subprocess.run(['nssm', 'stop', 'WinLLDP'], capture_output=True, text=True)
    if verbose:
        click.echo(f"  Stop result: {stop_result.returncode}")
        if stop_result.stderr:
            click.echo(f"  Output: {stop_result.stderr.strip()}")
    
    # Wait for service to stop
    if verbose:
        click.echo("Waiting for service to stop...")
    time.sleep(2)
    
    # Remove the service
    click.echo("Removing service...")
    if verbose:
        click.echo("Executing: nssm remove WinLLDP confirm")
    
    result = subprocess.run(['nssm', 'remove', 'WinLLDP', 'confirm'], capture_output=True, text=True)
    
    if verbose:
        click.echo(f"Remove command result: {result.returncode}")
        if result.stdout:
            click.echo(f"Output: {result.stdout.strip()}")
        if result.stderr:
            click.echo(f"Error output: {result.stderr.strip()}")
    
    if result.returncode == 0:
        click.echo("Service uninstalled successfully")
        
        if verbose:
            # Verify service is removed
            click.echo("\nVerifying service removal...")
            verify_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
            if verify_result.returncode != 0:
                click.echo("  Service successfully removed from Windows")
            else:
                click.echo("  WARNING: Service may still be registered in Windows")
            
            # Check for leftover files
            if getattr(sys, 'frozen', False):
                log_file = os.path.join(os.path.dirname(sys.executable), 'nssm_service.log')
            else:
                log_file = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'nssm_service.log')
            
            if os.path.exists(log_file):
                click.echo(f"\n  Note: Service log file still exists: {log_file}")
                click.echo("  You may want to delete it manually if no longer needed")
    else:
        click.echo(f"Error uninstalling service: {result.stderr}", err=True)


@service.command('start')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def service_start(verbose):
    """Start Windows service"""
    setup_logging(verbose)
    import subprocess
    
    try:
        # First check if service exists
        if verbose:
            click.echo("Checking if service exists...")
        check_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
        if check_result.returncode != 0:
            click.echo("Error: Service WinLLDP not found. Please install it first.", err=True)
            click.echo("Run: winlldp service install")
            return
        
        if verbose:
            click.echo("Current service status:")
            # Parse service status
            for line in check_result.stdout.split('\n'):
                if 'STATE' in line:
                    click.echo(f"  {line.strip()}")
        
        # Check NSSM configuration
        if verbose:
            click.echo("Checking NSSM configuration...")
            config_checks = [
                ('Application', ['nssm', 'get', 'WinLLDP', 'Application']),
                ('AppDirectory', ['nssm', 'get', 'WinLLDP', 'AppDirectory']),
                ('AppParameters', ['nssm', 'get', 'WinLLDP', 'AppParameters']),
                ('Start', ['nssm', 'get', 'WinLLDP', 'Start']),
            ]
            
            for name, cmd in config_checks:
                result = subprocess.run(cmd, capture_output=True, text=True)
                if result.returncode == 0:
                    click.echo(f"  {name}: {result.stdout.strip()}")
                else:
                    click.echo(f"  {name}: Unable to retrieve")
        
        click.echo("Starting WinLLDP service...")
        if verbose:
            click.echo("Executing: nssm start WinLLDP")
        
        result = subprocess.run(['nssm', 'start', 'WinLLDP'], capture_output=True, text=True)
        
        if verbose:
            click.echo(f"Start command result: {result.returncode}")
            if result.stdout:
                click.echo(f"Output: {result.stdout.strip()}")
            if result.stderr:
                click.echo(f"Error output: {result.stderr.strip()}")
        
        if result.returncode == 0:
            click.echo("Service started successfully")
            
            # Wait and check if service is actually running
            if verbose:
                click.echo("Verifying service status...")
                time.sleep(2)
                status_check = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
                for line in status_check.stdout.split('\n'):
                    if 'STATE' in line:
                        click.echo(f"  {line.strip()}")
        else:
            if "already running" in result.stderr:
                click.echo("Service is already running")
            else:
                click.echo(f"Error starting service: {result.stderr}", err=True)
                
                # Get more details about the error
                if verbose:
                    click.echo("\nChecking Windows Event Log for errors...")
                    # Check if service wrapper exists
                    wrapper_path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'winlldp', 'service_wrapper.py')
                    if os.path.exists(wrapper_path):
                        click.echo(f"  Service wrapper exists: {wrapper_path}")
                    else:
                        click.echo(f"  Service wrapper NOT FOUND: {wrapper_path}")
                    
                    # Check service log file
                    if getattr(sys, 'frozen', False):
                        log_path = os.path.join(os.path.dirname(sys.executable), 'nssm_service.log')
                    else:
                        log_path = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'nssm_service.log')
                    
                    if os.path.exists(log_path):
                        click.echo(f"\nService log file: {log_path}")
                        with open(log_path, 'r') as f:
                            lines = f.readlines()
                            if lines:
                                click.echo("  Last 10 lines:")
                                for line in lines[-10:]:
                                    click.echo(f"    {line.rstrip()}")
                    
    except FileNotFoundError:
        click.echo("Error: NSSM not found. Please install the service first.", err=True)
        click.echo("Run: winlldp service install")


@service.command('stop')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def service_stop(verbose):
    """Stop Windows service"""
    setup_logging(verbose)
    import subprocess
    
    try:
        if verbose:
            click.echo("Checking current service status...")
            status_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
            if status_result.returncode == 0:
                for line in status_result.stdout.split('\n'):
                    if 'STATE' in line:
                        click.echo(f"  Current status: {line.strip()}")
        
        click.echo("Stopping WinLLDP service...")
        if verbose:
            click.echo("Executing: nssm stop WinLLDP")
        
        result = subprocess.run(['nssm', 'stop', 'WinLLDP'], capture_output=True, text=True)
        
        if verbose:
            click.echo(f"Stop command result: {result.returncode}")
            if result.stdout:
                click.echo(f"Output: {result.stdout.strip()}")
            if result.stderr:
                click.echo(f"Error output: {result.stderr.strip()}")
        
        if result.returncode == 0:
            click.echo("Service stopped successfully")
            
            if verbose:
                # Wait and verify service is stopped
                click.echo("Verifying service is stopped...")
                time.sleep(2)
                verify_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
                if verify_result.returncode == 0:
                    for line in verify_result.stdout.split('\n'):
                        if 'STATE' in line:
                            click.echo(f"  Final status: {line.strip()}")
        else:
            if "not running" in result.stderr:
                click.echo("Service is not running")
            else:
                click.echo(f"Error stopping service: {result.stderr}", err=True)
    except FileNotFoundError:
        click.echo("Error: NSSM not found", err=True)


@service.command('status')
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def service_status(verbose):
    """Show Windows service status"""
    setup_logging(verbose)
    import subprocess
    import os
    
    try:
        if verbose:
            click.echo("Checking service status...")
        
        result = subprocess.run(['nssm', 'status', 'WinLLDP'], capture_output=True, text=True)
        
        if result.returncode == 0:
            status = result.stdout.strip()
            click.echo(f"WinLLDP service status: {status}")
            
            if verbose:
                # Get detailed service info
                click.echo("\nDetailed service information:")
                
                # Get Windows service status
                sc_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
                if sc_result.returncode == 0:
                    click.echo("  Windows Service Control:")
                    for line in sc_result.stdout.split('\n'):
                        line = line.strip()
                        if line and any(keyword in line for keyword in ['SERVICE_NAME', 'STATE', 'TYPE', 'WIN32_EXIT_CODE']):
                            click.echo(f"    {line}")
                
                # Get NSSM configuration
                click.echo("\n  NSSM Configuration:")
                config_items = [
                    ('Application', 'Application'),
                    ('AppDirectory', 'Working Directory'),
                    ('AppParameters', 'Parameters'),
                    ('DisplayName', 'Display Name'),
                    ('Start', 'Startup Type'),
                    ('ObjectName', 'Run As User'),
                ]
                
                for nssm_param, display_name in config_items:
                    cmd = ['nssm', 'get', 'WinLLDP', nssm_param]
                    config_result = subprocess.run(cmd, capture_output=True, text=True)
                    if config_result.returncode == 0:
                        value = config_result.stdout.strip()
                        click.echo(f"    {display_name}: {value}")
            
            if status == "SERVICE_RUNNING":
                click.echo("  The service is running")
                
                # Show additional info from log
                if getattr(sys, 'frozen', False):
                    log_file = os.path.join(os.path.dirname(sys.executable), 'nssm_service.log')
                else:
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
                        
                        if verbose and lines:
                            click.echo(f"\n  Service log file: {log_file}")
                            click.echo(f"  Log file size: {os.path.getsize(log_file)} bytes")
                            click.echo("  Last 5 log entries:")
                            for line in lines[-5:]:
                                click.echo(f"    {line.rstrip()}")
                    except:
                        pass
                elif verbose:
                    click.echo(f"\n  Service log file not found: {log_file}")
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
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose debug output')
def service_restart(verbose):
    """Restart Windows service"""
    setup_logging(verbose)
    import subprocess
    
    try:
        if verbose:
            click.echo("Checking current service status...")
            status_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
            if status_result.returncode == 0:
                for line in status_result.stdout.split('\n'):
                    if 'STATE' in line:
                        click.echo(f"  Current status: {line.strip()}")
        
        click.echo("Restarting WinLLDP service...")
        if verbose:
            click.echo("Executing: nssm restart WinLLDP")
        
        result = subprocess.run(['nssm', 'restart', 'WinLLDP'], capture_output=True, text=True)
        
        if verbose:
            click.echo(f"Restart command result: {result.returncode}")
            if result.stdout:
                click.echo(f"Output: {result.stdout.strip()}")
            if result.stderr:
                click.echo(f"Error output: {result.stderr.strip()}")
        
        if result.returncode == 0:
            click.echo("Service restarted successfully")
            
            if verbose:
                # Wait and verify service is running
                click.echo("\nVerifying service is running...")
                time.sleep(3)
                verify_result = subprocess.run(['sc', 'query', 'WinLLDP'], capture_output=True, text=True)
                if verify_result.returncode == 0:
                    for line in verify_result.stdout.split('\n'):
                        if 'STATE' in line:
                            click.echo(f"  Final status: {line.strip()}")
        else:
            click.echo(f"Error restarting service: {result.stderr}", err=True)
    except FileNotFoundError:
        click.echo("Error: NSSM not found", err=True)


@cli.command('capture-subprocess', hidden=True)
@click.argument('log_file')
@click.argument('neighbors_file')
@click.argument('pid_file')
def capture_subprocess_cmd(log_file, neighbors_file, pid_file):
    """Internal command for running capture subprocess from frozen executable"""
    # This is a hidden command used internally when running from PyInstaller bundle
    from .capture_subprocess import main
    # Update sys.argv to match what the subprocess expects
    sys.argv = [sys.argv[0], log_file, neighbors_file, pid_file]
    main()


@cli.command('service-run', hidden=True)
def service_run():
    """Internal command to run the service from frozen executable"""
    # This is a hidden command used internally when running as a Windows service
    from .service_wrapper import main
    main()


if __name__ == '__main__':
    cli()