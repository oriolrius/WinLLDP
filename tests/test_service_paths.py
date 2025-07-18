"""Tests for service and capture path consistency"""
import os
import sys
import tempfile
import unittest
from unittest.mock import patch, MagicMock, mock_open


class TestServicePaths(unittest.TestCase):
    """Test that service and capture modes use consistent paths"""
    
    def setUp(self):
        """Set up test environment"""
        self.original_frozen = getattr(sys, 'frozen', None)
        self.original_executable = sys.executable
        
    def tearDown(self):
        """Restore original state"""
        if self.original_frozen is None:
            if hasattr(sys, 'frozen'):
                delattr(sys, 'frozen')
        else:
            sys.frozen = self.original_frozen
        sys.executable = self.original_executable
    
    @patch('os.chdir')
    def test_service_wrapper_stays_in_exe_directory(self, mock_chdir):
        """Test that service wrapper stays in exe directory when frozen"""
        # Mock frozen environment
        sys.frozen = True
        test_exe_dir = r'D:\test\dist'
        sys.executable = os.path.join(test_exe_dir, 'winlldp.exe')
        
        # Mock current directory as PyInstaller temp
        pyinstaller_temp = r'C:\WINDOWS\TEMP\_MEI123456'
        with patch('os.getcwd', return_value=pyinstaller_temp):
            # Import and test service wrapper behavior
            from winlldp.paths import get_runtime_directory
            runtime_dir = get_runtime_directory()
            
            # Service should want to change to exe directory
            self.assertEqual(runtime_dir, test_exe_dir)
            self.assertNotEqual(runtime_dir, pyinstaller_temp)
    
    def test_receiver_uses_config_paths(self):
        """Test that LLDP receiver uses paths from config"""
        from winlldp.config import Config
        from winlldp.lldp_receiver import LLDPReceiver
        
        config = Config()
        receiver = LLDPReceiver(config)
        
        # Receiver should use config paths
        self.assertEqual(receiver._neighbors_file, config.neighbors_file)
        self.assertEqual(receiver._pid_file, config.pid_file)
        self.assertEqual(receiver._log_file, config.log_file)
    
    @patch('subprocess.Popen')
    def test_capture_subprocess_receives_correct_paths(self, mock_popen):
        """Test that capture subprocess is started with correct paths"""
        # Mock process
        mock_process = MagicMock()
        mock_process.pid = 12345
        mock_process.poll.return_value = None
        mock_popen.return_value = mock_process
        
        from winlldp.config import Config
        from winlldp.lldp_receiver import LLDPReceiver
        
        config = Config()
        receiver = LLDPReceiver(config)
        
        # Start capture
        result = receiver.start_capture()
        self.assertTrue(result)
        
        # Check subprocess was called with config paths
        mock_popen.assert_called_once()
        args = mock_popen.call_args[0][0]
        
        # Should pass config paths as arguments
        self.assertIn(config.log_file, args)
        self.assertIn(config.neighbors_file, args)
        self.assertIn(config.pid_file, args)
    
    def test_cli_uses_same_paths_as_service(self):
        """Test that CLI commands use same paths as service"""
        from winlldp.config import Config
        from winlldp.paths import get_log_file, get_neighbors_file
        
        config = Config()
        
        # CLI should use same paths
        self.assertEqual(get_log_file(), config.log_file)
        self.assertEqual(get_neighbors_file(), config.neighbors_file)
    
    @patch('builtins.open', new_callable=mock_open, read_data='12345')
    @patch('os.path.exists', return_value=True)
    def test_pid_file_location_consistency(self, mock_exists, mock_file):
        """Test that PID file is checked in correct location"""
        from winlldp.config import Config
        from winlldp.lldp_receiver import LLDPReceiver
        
        config = Config()
        receiver = LLDPReceiver(config)
        
        # Check if capture is running
        is_running = receiver.is_capture_running()
        
        # Should check config.pid_file location
        mock_file.assert_called_with(config.pid_file, 'r')


class TestPathMigration(unittest.TestCase):
    """Test handling of existing files in old locations"""
    
    def test_old_temp_files_ignored(self):
        """Test that old files in temp directory are ignored"""
        # Create mock temp files
        temp_neighbors = os.path.join(tempfile.gettempdir(), 'neighbors.json')
        
        from winlldp.config import Config
        config = Config()
        
        # New config should not use temp directory
        self.assertNotEqual(os.path.dirname(config.neighbors_file), tempfile.gettempdir())


if __name__ == '__main__':
    unittest.main()