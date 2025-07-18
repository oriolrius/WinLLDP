# File operations debug wrapper for WinLLDP
# Logs all file operations with caller information
import os
import sys
import inspect
from contextlib import contextmanager
try:
    from .logger import get_logger
    logger = get_logger()
except ImportError:
    # Fallback for when running in frozen executable or different context
    import logging
    logger = logging.getLogger('WinLLDP')
    if not logger.handlers:
        handler = logging.StreamHandler()
        handler.setFormatter(logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s'))
        logger.addHandler(handler)
        logger.setLevel(logging.DEBUG)

def get_caller_info():
    # Get information about the caller of the file operation
    frame = inspect.currentframe()
    try:
        # Go up the stack to find the actual caller (skip our wrapper functions)
        caller_frame = frame.f_back.f_back.f_back
        if caller_frame:
            filename = os.path.basename(caller_frame.f_code.co_filename)
            function = caller_frame.f_code.co_name
            line_no = caller_frame.f_lineno
            return f"{filename}:{function}:{line_no}"
        return "unknown"
    except:
        return "unknown"
    finally:
        del frame

def get_file_mode_description(mode):
    # Convert file mode to human-readable description
    if 'r' in mode and 'w' in mode:
        return "READ/WRITE"
    elif 'r' in mode:
        return "READ"
    elif 'w' in mode:
        return "WRITE"
    elif 'a' in mode:
        return "APPEND"
    else:
        return mode.upper()

# Global flag to control debug output
_verbose_enabled = False

def set_verbose(enabled):
    global _verbose_enabled
    _verbose_enabled = enabled

@contextmanager
def debug_open(filename, mode='r', *args, **kwargs):
    # Wrapper for open() that logs file operations
    global _verbose_enabled
    
    if _verbose_enabled:
        caller = get_caller_info()
        mode_desc = get_file_mode_description(mode)
        abs_path = os.path.abspath(filename)
        logger.debug(f"FILE_OPEN: {mode_desc} '{abs_path}' from {caller}")
    
    try:
        file_obj = open(filename, mode, *args, **kwargs)
        if _verbose_enabled:
            logger.debug(f"FILE_OPEN_SUCCESS: {mode_desc} '{abs_path}'")
        yield file_obj
    except Exception as e:
        if _verbose_enabled:
            logger.debug(f"FILE_OPEN_ERROR: {mode_desc} '{abs_path}' - {str(e)}")
        raise
    finally:
        if _verbose_enabled:
            logger.debug(f"FILE_CLOSE: '{abs_path}'")
        try:
            file_obj.close()
        except:
            pass

