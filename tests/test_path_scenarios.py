"""Integration tests for various path scenarios"""
import os
import sys
import tempfile
import shutil
import unittest
from unittest.mock import patch


class TestPathScenarios(unittest.TestCase):
    """Test real-world scenarios for path handling"""
    
    def setUp(self):
        """Create test directories"""
        self.test_dir = tempfile.mkdtemp()
        self.exe_dir = os.path.join(self.test_dir, 'dist')
        os.makedirs(self.exe_dir)
        
        # Save original state
        self.original_frozen = getattr(sys, 'frozen', None)
        self.original_executable = sys.executable
        
    def tearDown(self):
        """Clean up test directories and restore state"""
        shutil.rmtree(self.test_dir, ignore_errors=True)
        
        if self.original_frozen is None:
            if hasattr(sys, 'frozen'):
                delattr(sys, 'frozen')
        else:
            sys.frozen = self.original_frozen
        sys.executable = self.original_executable
    
    def test_scenario_fresh_install(self):
        """Test fresh installation with no existing files"""
        # Mock frozen exe
        sys.frozen = True
        sys.executable = os.path.join(self.exe_dir, 'winlldp.exe')
        
        from winlldp.config import Config
        config = Config()
        
        # All files should be in exe directory
        self.assertEqual(os.path.dirname(config.neighbors_file), self.exe_dir)
        self.assertEqual(os.path.dirname(config.pid_file), self.exe_dir)
        self.assertEqual(os.path.dirname(config.log_file), self.exe_dir)
        
        # No files should exist yet
        self.assertFalse(os.path.exists(config.neighbors_file))
        self.assertFalse(os.path.exists(config.pid_file))
        self.assertFalse(os.path.exists(config.log_file))
    
    def test_scenario_service_creates_files(self):
        """Test that service creates files in correct location"""
        # Mock frozen exe
        sys.frozen = True
        sys.executable = os.path.join(self.exe_dir, 'winlldp.exe')
        
        from winlldp.config import Config
        config = Config()
        
        # Simulate service creating files
        test_data = {'test': 'data'}
        import json
        
        # Write neighbors file
        with open(config.neighbors_file, 'w') as f:
            json.dump(test_data, f)
        
        # Write pid file
        with open(config.pid_file, 'w') as f:
            f.write('12345')
        
        # Write log file
        with open(config.log_file, 'w') as f:
            f.write('Test log entry\n')
        
        # All files should exist in exe directory
        self.assertTrue(os.path.exists(config.neighbors_file))
        self.assertTrue(os.path.exists(config.pid_file))
        self.assertTrue(os.path.exists(config.log_file))
        
        # Verify they're in exe directory
        for file_path in [config.neighbors_file, config.pid_file, config.log_file]:
            self.assertEqual(os.path.dirname(file_path), self.exe_dir)
    
    def test_scenario_cli_reads_service_files(self):
        """Test that CLI can read files created by service"""
        # Mock frozen exe
        sys.frozen = True
        sys.executable = os.path.join(self.exe_dir, 'winlldp.exe')
        
        # Create test files as if service created them
        neighbors_data = {
            'test_neighbor': {
                'interface': 'eth0',
                'source_mac': '00:11:22:33:44:55',
                'data': {'system_name': 'TestSystem'}
            }
        }
        
        from winlldp.config import Config
        config = Config()
        
        import json
        with open(config.neighbors_file, 'w') as f:
            json.dump(neighbors_data, f)
        
        # CLI should read from same location
        self.assertTrue(os.path.exists(config.neighbors_file))
        
        with open(config.neighbors_file, 'r') as f:
            read_data = json.load(f)
        
        self.assertEqual(read_data, neighbors_data)
    
    @patch('winlldp.config.load_dotenv')
    def test_scenario_pyinstaller_temp_ignored(self, mock_load_dotenv):
        """Test that PyInstaller temp directory is not used"""
        # Mock frozen exe with PyInstaller temp
        sys.frozen = True
        sys.executable = os.path.join(self.exe_dir, 'winlldp.exe')
        pyinstaller_temp = r'C:\WINDOWS\TEMP\_MEI123456'
        
        # Even if CWD is PyInstaller temp, files should go to exe dir
        with patch('os.getcwd', return_value=pyinstaller_temp):
            from winlldp.config import Config
            config = Config()
            
            # Files should NOT be in PyInstaller temp
            self.assertNotIn('_MEI', config.neighbors_file)
            self.assertNotIn('_MEI', config.pid_file)
            self.assertNotIn('_MEI', config.log_file)
            
            # Files should be in exe directory
            self.assertEqual(os.path.dirname(config.neighbors_file), self.exe_dir)
    
    def test_scenario_development_mode(self):
        """Test development mode uses project root"""
        # Ensure not frozen
        if hasattr(sys, 'frozen'):
            delattr(sys, 'frozen')
        
        from winlldp.config import Config
        from winlldp.paths import get_runtime_directory
        
        config = Config()
        runtime_dir = get_runtime_directory()
        
        # Should use project root, not temp
        self.assertNotEqual(runtime_dir, tempfile.gettempdir())
        self.assertNotIn('Temp', runtime_dir)
        self.assertNotIn('tmp', runtime_dir)
        
        # Should be able to find winlldp package
        self.assertTrue(os.path.exists(os.path.join(runtime_dir, 'winlldp')))


if __name__ == '__main__':
    unittest.main()