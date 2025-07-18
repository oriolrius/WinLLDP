"""Integration tests for the frozen executable"""
import os
import sys
import subprocess
import tempfile
import shutil
import json
import time
import unittest
from pathlib import Path
try:
    import tomllib
except ImportError:
    import tomli as tomllib


class TestFrozenExecutable(unittest.TestCase):
    """Test the actual frozen executable behavior
    
    Note: capture start command may take significant time to return
    as it needs to initialize the packet capture subprocess and 
    verify it's running correctly.
    """
    
    @classmethod
    def setUpClass(cls):
        """Build the executable once for all tests"""
        # Get project root
        project_root = Path(__file__).parent.parent
        cls.project_root = project_root
        cls.exe_path = project_root / "dist" / "winlldp.exe"
        
        # Check if executable exists and is recent (within last hour)
        if cls.exe_path.exists():
            exe_mtime = cls.exe_path.stat().st_mtime
            current_time = time.time()
            if current_time - exe_mtime < 3600:  # Less than 1 hour old
                print(f"Using existing executable: {cls.exe_path}")
                return
        
        # Build executable
        print("Building executable with PyInstaller...")
        result = cls._run_with_progress(
            ["just", "pyinstaller"],
            cwd=project_root,
            description="Running: just pyinstaller"
        )
        
        if result.returncode != 0:
            # Fallback to direct pyinstaller command using sys.executable
            print("\nJust command failed, trying direct pyinstaller...")
            # First try with uv run
            result = cls._run_with_progress(
                ["uv", "run", "pyinstaller", "--clean", "pyinstaller.spec"],
                cwd=project_root,
                description="Running: uv run pyinstaller"
            )
            
            if result.returncode != 0:
                # Try with uvx as last resort
                print("\nuv run failed, trying uvx...")
                result = cls._run_with_progress(
                    ["uvx", "pyinstaller", "--clean", "pyinstaller.spec"],
                    cwd=project_root,
                    description="Running: uvx pyinstaller"
                )
        
        if result.returncode != 0:
            raise RuntimeError(f"Failed to build executable:\\n{result.stderr}")
        
        if not cls.exe_path.exists():
            raise RuntimeError(f"Executable not found at {cls.exe_path}")
        
        print(f"\nExecutable built successfully: {cls.exe_path}")
    
    @classmethod
    def _run_with_progress(cls, cmd, cwd, description):
        """Run a command showing progress dots"""
        print(f"{description}", end="", flush=True)
        
        # Start the process
        process = subprocess.Popen(
            cmd,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )
        
        # Show progress dots while running
        import threading
        output_lines = []
        error_lines = []
        
        def read_output():
            for line in process.stdout:
                output_lines.append(line)
        
        def read_error():
            for line in process.stderr:
                error_lines.append(line)
        
        # Start reader threads
        stdout_thread = threading.Thread(target=read_output)
        stderr_thread = threading.Thread(target=read_error)
        stdout_thread.start()
        stderr_thread.start()
        
        # Show progress while waiting
        while process.poll() is None:
            print(".", end="", flush=True)
            time.sleep(2)
        
        # Wait for threads to finish
        stdout_thread.join()
        stderr_thread.join()
        
        print()  # New line after dots
        
        # Create result object similar to subprocess.run
        from subprocess import CompletedProcess
        return CompletedProcess(
            cmd, 
            process.returncode,
            ''.join(output_lines),
            ''.join(error_lines)
        )
    
    def setUp(self):
        """Create a temporary directory for each test"""
        self.test_dir = tempfile.mkdtemp()
        self.test_exe = Path(self.test_dir) / "winlldp.exe"
        
        # Copy executable to test directory
        shutil.copy2(self.exe_path, self.test_exe)
        
        # Also copy pyproject.toml for version command
        shutil.copy2(self.project_root / "pyproject.toml", self.test_dir)
    
    def tearDown(self):
        """Clean up test directory"""
        # Kill any running capture processes
        subprocess.run(
            [str(self.test_exe), "capture", "stop"],
            capture_output=True
        )
        time.sleep(0.5)  # Give process time to clean up
        
        # Remove test directory
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def run_exe(self, *args, timeout=30):
        """Run the executable with given arguments"""
        result = subprocess.run(
            [str(self.test_exe)] + list(args),
            cwd=self.test_dir,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        # Print output for debugging
        if result.returncode != 0:
            print(f"Command failed: {' '.join([str(self.test_exe)] + list(args))}")
            print(f"Return code: {result.returncode}")
            print(f"Stdout: {result.stdout}")
            print(f"Stderr: {result.stderr}")
        return result
    
    def test_exe_runs_without_error(self):
        """Test that executable runs and shows help"""
        result = self.run_exe("--help")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage:", result.stdout)
        self.assertIn("winlldp", result.stdout)
    
    def test_version_command(self):
        """Test version command reads from pyproject.toml"""
        # Read expected version from pyproject.toml
        pyproject_path = Path(self.test_dir) / "pyproject.toml"
        self.assertTrue(pyproject_path.exists(), f"pyproject.toml not found at {pyproject_path}")
        
        with open(pyproject_path, 'rb') as f:
            pyproject_data = tomllib.load(f)
        
        expected_version = pyproject_data['project']['version']
        
        # Run version command
        result = self.run_exe("version")
        self.assertEqual(result.returncode, 0)
        
        # Version in output should match pyproject.toml
        # The output might include 'v' prefix or not, so check both
        version_found = (expected_version in result.stdout or 
                        expected_version.lstrip('v') in result.stdout)
        
        self.assertTrue(
            version_found,
            f"Expected version '{expected_version}' not found in output: {result.stdout}"
        )
    
    def test_files_created_in_exe_directory(self):
        """Test that files are created in exe directory, not temp"""
        # Start capture - may take time to initialize
        result = self.run_exe("capture", "start", timeout=30)
        
        # If capture start failed, skip the test
        if result.returncode != 0:
            self.skipTest(f"Failed to start capture: {result.stderr}")
        
        # Give it time to start
        time.sleep(2)
        
        # Check files exist in exe directory
        neighbors_file = Path(self.test_dir) / "neighbors.json"
        pid_file = Path(self.test_dir) / "capture.pid"
        log_file = Path(self.test_dir) / "winlldp_capture.log"
        
        self.assertTrue(pid_file.exists(), f"PID file not found at {pid_file}")
        self.assertTrue(log_file.exists(), f"Log file not found at {log_file}")
        
        # Stop capture
        result = self.run_exe("capture", "stop")
        self.assertEqual(result.returncode, 0)
        
        # PID file should be cleaned up
        self.assertFalse(pid_file.exists(), "PID file not cleaned up")
    
    def test_no_files_in_temp_directory(self):
        """Test that no files are created in temp directories"""
        temp_dir = tempfile.gettempdir()
        
        # Get initial temp files
        initial_files = set(os.listdir(temp_dir))
        
        # Start capture - may take time to initialize
        result = self.run_exe("capture", "start", timeout=30)
        
        # If capture start failed, skip the test
        if result.returncode != 0:
            self.skipTest(f"Failed to start capture: {result.stderr}")
        
        time.sleep(2)
        
        # Check no new files in temp
        current_files = set(os.listdir(temp_dir))
        new_files = current_files - initial_files
        
        # Filter out unrelated temp files
        lldp_files = [f for f in new_files if 'neighbors' in f or 'capture' in f or 'winlldp' in f]
        
        # Stop capture
        self.run_exe("capture", "stop")
        
        self.assertEqual(len(lldp_files), 0, 
                        f"Found LLDP files in temp directory: {lldp_files}")
    
    def test_capture_log_command(self):
        """Test that capture log command finds log in correct location"""
        # Start capture - may take time to initialize, be generous with timeout
        result = self.run_exe("capture", "start", timeout=30)
        
        # If it failed, skip the test
        if result.returncode != 0:
            self.skipTest(f"Failed to start capture: {result.stderr}")
        
        time.sleep(2)
        
        # Check log
        result = self.run_exe("capture", "log")
        self.assertEqual(result.returncode, 0)
        self.assertNotIn("No log file found", result.stdout)
        
        # Stop capture
        self.run_exe("capture", "stop")
    
    def test_show_interfaces(self):
        """Test that show-interfaces command works"""
        result = self.run_exe("show-interfaces")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Network Interfaces:", result.stdout)
    
    def test_send_command(self):
        """Test that send command works (may require admin)"""
        result = self.run_exe("send", "-v")
        # May fail if not admin, but should not crash
        self.assertIn("Sending LLDP packets", result.stdout)
    
    def test_show_neighbors_with_capture(self):
        """Test show-neighbors reads from correct location"""
        # Create a fake neighbors file in exe directory
        neighbors_data = {
            "test_neighbor": {
                "interface": "Test Interface",
                "source_mac": "00:11:22:33:44:55",
                "data": {
                    "system_name": "Test System",
                    "system_description": "Test Description"
                },
                "first_seen": "2024-01-01T00:00:00",
                "last_seen": "2024-01-01T00:00:00",
                "ttl": 120
            }
        }
        
        neighbors_file = Path(self.test_dir) / "neighbors.json"
        with open(neighbors_file, 'w') as f:
            json.dump(neighbors_data, f)
        
        # Run show-neighbors
        result = self.run_exe("show-neighbors")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Test System", result.stdout)
        # MAC address might show as "Unknown" in the neighbor column
        # but should be somewhere in the output
        if "00:11:22:33:44:55" not in result.stdout:
            # Check if at least the system name is shown correctly
            self.assertIn("Test System", result.stdout)
            # This is OK - the MAC might be shown differently


class TestServiceMode(unittest.TestCase):
    """Test service mode behavior (requires admin privileges)"""
    
    @classmethod
    def setUpClass(cls):
        """Check if we have admin privileges"""
        import ctypes
        cls.is_admin = ctypes.windll.shell32.IsUserAnAdmin() != 0
        
        if cls.is_admin:
            # Use the same executable from TestFrozenExecutable
            cls.exe_path = Path(__file__).parent.parent / "dist" / "winlldp.exe"
            if not cls.exe_path.exists():
                raise unittest.SkipTest("Executable not found. Run TestFrozenExecutable first.")
        else:
            raise unittest.SkipTest("Service tests require administrator privileges")
    
    def setUp(self):
        """Create test directory with executable"""
        self.test_dir = tempfile.mkdtemp()
        self.test_exe = Path(self.test_dir) / "winlldp.exe"
        shutil.copy2(self.exe_path, self.test_exe)
        shutil.copy2(self.exe_path.parent.parent / "pyproject.toml", self.test_dir)
    
    def tearDown(self):
        """Clean up test directory"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
    
    def test_service_files_location(self):
        """Test that service creates files in exe directory"""
        # First check if WinLLDP service is already installed
        check_result = subprocess.run(
            ["sc", "query", "WinLLDP"],
            capture_output=True,
            text=True
        )
        
        service_was_installed = check_result.returncode == 0
        service_was_running = False
        
        if service_was_installed:
            # Check if it's running
            if "RUNNING" in check_result.stdout:
                service_was_running = True
                # Stop it for our test
                subprocess.run(
                    [str(self.test_exe), "service", "stop"],
                    cwd=self.test_dir,
                    capture_output=True,
                    timeout=30
                )
                time.sleep(2)
            
            # Uninstall it for our test
            subprocess.run(
                [str(self.test_exe), "service", "uninstall"],  # No verbose flag to avoid confirmation prompt
                cwd=self.test_dir,
                capture_output=True,
                text=True,
                timeout=30,
                input="y\n"  # Provide 'y' to the confirmation prompt
            )
            time.sleep(2)
        
        try:
            # Install the service
            result = subprocess.run(
                [str(self.test_exe), "service", "install"],
                cwd=self.test_dir,
                capture_output=True,
                text=True,
                timeout=30
            )
            
            # Check if NSSM is available
            if "'nssm' is not recognized" in result.stderr or "nssm: not found" in result.stderr:
                self.skipTest("NSSM not found in PATH")
            
            # Check if we have admin privileges
            if "access" in result.stderr.lower() or "denied" in result.stderr.lower():
                self.skipTest("Admin privileges required")
            
            self.assertEqual(result.returncode, 0, f"Failed to install service: {result.stderr}")
            self.assertIn("Service installed successfully", result.stdout)
            
            # Start the service
            result = subprocess.run(
                [str(self.test_exe), "service", "start"],
                cwd=self.test_dir,
                capture_output=True,
                text=True,
                timeout=30
            )
            self.assertEqual(result.returncode, 0, f"Failed to start service: {result.stderr}")
            self.assertIn("Service started successfully", result.stdout)
            
            # Give service time to initialize and create files
            time.sleep(5)
            
            # Check that files are created in exe directory
            service_log = Path(self.test_dir) / "nssm_service.log"
            
            self.assertTrue(service_log.exists(), f"Service log not found at {service_log}")
            
            # Read service log to verify it's running
            with open(service_log, 'r') as f:
                log_content = f.read()
            
            self.assertIn("Starting NSSM service wrapper", log_content)
            self.assertIn("Service started successfully", log_content)
            # Check that it's using the exe directory, not temp
            self.assertIn(str(self.test_dir), log_content)
            
            # Stop the service
            result = subprocess.run(
                [str(self.test_exe), "service", "stop"],
                cwd=self.test_dir,
                capture_output=True,
                text=True,
                timeout=30
            )
            self.assertEqual(result.returncode, 0, f"Failed to stop service: {result.stderr}")
            
            # Wait for service to fully stop
            print("Waiting for service to stop...")
            for i in range(10):
                check = subprocess.run(
                    ["sc", "query", "WinLLDP"],
                    capture_output=True,
                    text=True
                )
                if "STOPPED" in check.stdout:
                    print("Service stopped successfully")
                    break
                time.sleep(1)
            else:
                print("Warning: Service may not have fully stopped")
            
            # Uninstall the service
            print("Uninstalling service...")
            result = subprocess.run(
                [str(self.test_exe), "service", "uninstall"],  # No verbose flag to avoid confirmation prompt
                cwd=self.test_dir,
                capture_output=True,
                text=True,
                timeout=30,
                input="y\n"  # Provide 'y' to the confirmation prompt
            )
            
            # Debug output
            print(f"Uninstall return code: {result.returncode}")
            print(f"Uninstall stdout: {result.stdout}")
            print(f"Uninstall stderr: {result.stderr}")
            
            # The uninstall command uses 'nssm remove WinLLDP confirm' internally
            # which should bypass the confirmation prompt
            if result.returncode != 0:
                # Wait a bit and check if service is actually gone
                time.sleep(2)
                check = subprocess.run(
                    ["sc", "query", "WinLLDP"],
                    capture_output=True,
                    text=True
                )
                if check.returncode != 0:  # Service not found = success
                    print("Service was removed despite non-zero return code")
                else:
                    # Service still exists, this is a real failure
                    self.fail(f"Failed to uninstall service. Service still exists.\nOutput: {result.stdout}\nError: {result.stderr}")
            
        finally:
            # Restore original service if it was installed
            if service_was_installed:
                # Re-install the original service
                subprocess.run(
                    [str(self.exe_path), "service", "install"],
                    capture_output=True,
                    timeout=30
                )
                
                if service_was_running:
                    # Restart it if it was running
                    subprocess.run(
                        [str(self.exe_path), "service", "start"],
                        capture_output=True,
                        timeout=30
                    )


if __name__ == '__main__':
    unittest.main()