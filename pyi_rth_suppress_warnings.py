"""
Runtime hook to suppress PyInstaller cleanup warnings
"""
import warnings
import sys

# Suppress the specific PyInstaller warning about temporary directory cleanup
warnings.filterwarnings("ignore", message="Failed to remove temporary directory")

# Also suppress it via sys.warnoptions
if not sys.warnoptions:
    import os
    os.environ["PYTHONWARNINGS"] = "ignore::UserWarning"