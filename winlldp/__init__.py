import os
import sys

def get_version():
    """Get version from pyproject.toml, works for both dev and frozen executable"""
    try:
        import tomllib
    except ImportError:
        # Python < 3.11
        import tomli as tomllib
    
    try:
        if getattr(sys, 'frozen', False):
            # Running from frozen executable - look for pyproject.toml in exe directory
            exe_dir = os.path.dirname(sys.executable)
            toml_path = os.path.join(exe_dir, 'pyproject.toml')
        else:
            # Running in development - read from project root
            root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            toml_path = os.path.join(root_dir, 'pyproject.toml')
        
        with open(toml_path, 'rb') as f:
            data = tomllib.load(f)
        
        return data['project']['version']
    except Exception:
        # Fallback - try to read from bundled data
        try:
            # This will work if pyproject.toml is included as data in PyInstaller
            if getattr(sys, '_MEIPASS', None):
                toml_path = os.path.join(sys._MEIPASS, 'pyproject.toml')
                with open(toml_path, 'rb') as f:
                    data = tomllib.load(f)
                return data['project']['version']
        except:
            pass
        return "unknown"

__version__ = get_version()
