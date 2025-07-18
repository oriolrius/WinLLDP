"""Tests for path resolution in frozen and development environments"""
import os
import sys
import tempfile
import unittest
from unittest.mock import patch


class TestPathResolution(unittest.TestCase):
    """Test path resolution for both frozen and development environments"""
    
    def setUp(self):
        """Set up test environment"""
        # Save original state
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
    
    def test_development_paths(self):
        """Test path resolution in development environment"""
        # Ensure we're in development mode
        if hasattr(sys, 'frozen'):
            delattr(sys, 'frozen')
        
        # Import after setting environment
        from winlldp.paths import get_runtime_directory, get_neighbors_file, get_pid_file, get_log_file
        
        runtime_dir = get_runtime_directory()
        
        # In development, runtime dir should be project root
        self.assertTrue(os.path.exists(runtime_dir))
        self.assertTrue(os.path.exists(os.path.join(runtime_dir, 'winlldp')))
        
        # Check file paths
        self.assertEqual(get_neighbors_file(), os.path.join(runtime_dir, 'neighbors.json'))
        self.assertEqual(get_pid_file(), os.path.join(runtime_dir, 'capture.pid'))
        self.assertEqual(get_log_file(), os.path.join(runtime_dir, 'winlldp_capture.log'))
    
    def test_frozen_paths(self):
        """Test path resolution in frozen (PyInstaller) environment"""
        # Mock frozen environment
        sys.frozen = True
        test_exe_dir = r'D:\test\dist'
        sys.executable = os.path.join(test_exe_dir, 'winlldp.exe')
        
        # Import after setting environment
        from winlldp.paths import get_runtime_directory, get_neighbors_file, get_pid_file, get_log_file
        
        runtime_dir = get_runtime_directory()
        
        # In frozen mode, runtime dir should be exe directory
        self.assertEqual(runtime_dir, test_exe_dir)
        
        # Check file paths
        self.assertEqual(get_neighbors_file(), os.path.join(test_exe_dir, 'neighbors.json'))
        self.assertEqual(get_pid_file(), os.path.join(test_exe_dir, 'capture.pid'))
        self.assertEqual(get_log_file(), os.path.join(test_exe_dir, 'winlldp_capture.log'))
    
    def test_config_uses_correct_paths(self):
        """Test that Config class uses centralized paths"""
        # Test in development mode
        if hasattr(sys, 'frozen'):
            delattr(sys, 'frozen')
        
        from winlldp.config import Config
        from winlldp.paths import get_neighbors_file, get_pid_file, get_log_file
        
        config = Config()
        
        # Config should use centralized paths
        self.assertEqual(config.neighbors_file, get_neighbors_file())
        self.assertEqual(config.pid_file, get_pid_file())
        self.assertEqual(config.log_file, get_log_file())
    
    def test_config_respects_custom_path(self):
        """Test that Config respects custom absolute path from environment"""
        custom_path = r'C:\custom\neighbors.json'
        
        with patch.dict(os.environ, {'LLDP_NEIGHBORS_FILE': custom_path}):
            from winlldp.config import Config
            config = Config()
            
            # Should use custom path when absolute
            self.assertEqual(config.neighbors_file, custom_path)


if __name__ == '__main__':
    unittest.main()