import win32serviceutil
import win32service
import win32event
import servicemanager
import socket
import sys
import os
from .service import WinLLDPService
from .config import Config


class WinLLDPWindowsService(win32serviceutil.ServiceFramework):
    _svc_name_ = "WinLLDP"
    _svc_display_name_ = "Windows LLDP Service"
    _svc_description_ = "Link Layer Discovery Protocol service for Windows"
    
    def __init__(self, args):
        win32serviceutil.ServiceFramework.__init__(self, args)
        self.hWaitStop = win32event.CreateEvent(None, 0, 0, None)
        socket.setdefaulttimeout(60)
        self.service = None
    
    def SvcStop(self):
        """Called when the service is being stopped"""
        self.ReportServiceStatus(win32service.SERVICE_STOP_PENDING)
        win32event.SetEvent(self.hWaitStop)
        
        if self.service:
            self.service.stop()
    
    def SvcDoRun(self):
        """Called when the service is started"""
        servicemanager.LogMsg(
            servicemanager.EVENTLOG_INFORMATION_TYPE,
            servicemanager.PYS_SERVICE_STARTED,
            (self._svc_name_, '')
        )
        
        self.main()
    
    def main(self):
        """Main service loop"""
        try:
            # Report that we're starting
            self.ReportServiceStatus(win32service.SERVICE_START_PENDING)
            
            # Load configuration
            config = Config()
            
            # Create and start service
            self.service = WinLLDPService(config)
            self.service.start()
            
            # Report that we're running
            self.ReportServiceStatus(win32service.SERVICE_RUNNING)
            
            # Main service loop - check for stop signal periodically
            while True:
                # Wait for stop signal with timeout
                rc = win32event.WaitForSingleObject(self.hWaitStop, 5000)  # 5 second timeout
                
                # Check if stop was signaled
                if rc == win32event.WAIT_OBJECT_0:
                    # Stop was signaled
                    break
                    
                # Service is still running, continue
                # This allows Windows to see that the service is responsive
                
        except Exception as e:
            servicemanager.LogErrorMsg(f"Service error: {str(e)}")
            self.ReportServiceStatus(win32service.SERVICE_STOPPED)


def install_service():
    """Install the Windows service"""
    if len(sys.argv) == 1:
        # If no arguments, assume install
        sys.argv.append('install')
    
    win32serviceutil.HandleCommandLine(WinLLDPWindowsService)


def main():
    """Entry point for Windows service"""
    if len(sys.argv) > 1:
        # Service control commands
        win32serviceutil.HandleCommandLine(WinLLDPWindowsService)
    else:
        # Run as console application
        print("Running in console mode...")
        from .service import main as service_main
        service_main()


if __name__ == '__main__':
    main()