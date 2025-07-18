"""Centralized path resolution for WinLLDP"""
import os
import sys


def get_runtime_directory():
    """Get the appropriate runtime directory for data files
    
    Returns:
        str: Directory path where runtime files should be stored
    """
    if getattr(sys, 'frozen', False):
        # When frozen, use the directory where the exe is located
        return os.path.dirname(sys.executable)
    else:
        # In development, use project root
        return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def get_neighbors_file():
    """Get the path to the neighbors.json file"""
    return os.path.join(get_runtime_directory(), 'neighbors.json')


def get_pid_file():
    """Get the path to the capture.pid file"""
    return os.path.join(get_runtime_directory(), 'capture.pid')


def get_log_file():
    """Get the path to the capture log file"""
    return os.path.join(get_runtime_directory(), 'winlldp_capture.log')


def get_service_log_file():
    """Get the path to the NSSM service log file"""
    return os.path.join(get_runtime_directory(), 'nssm_service.log')