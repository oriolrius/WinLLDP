//! Raw Ethernet send/capture through Npcap's wpcap.dll, loaded at run time
//! (no SDK needed to build, and a clear error when Npcap is missing)

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::OnceLock;

#[repr(C)]
struct BpfProgram {
    bf_len: u32,
    bf_insns: *mut c_void,
}

#[repr(C)]
struct PktHdr {
    ts_sec: i32,
    ts_usec: i32,
    caplen: u32,
    len: u32,
}

type OpenLive = unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut c_void;
type SendPacket = unsafe extern "C" fn(*mut c_void, *const u8, c_int) -> c_int;
type Compile = unsafe extern "C" fn(*mut c_void, *mut BpfProgram, *const c_char, c_int, u32) -> c_int;
type SetFilter = unsafe extern "C" fn(*mut c_void, *mut BpfProgram) -> c_int;
type FreeCode = unsafe extern "C" fn(*mut BpfProgram);
type NextEx = unsafe extern "C" fn(*mut c_void, *mut *mut PktHdr, *mut *const u8) -> c_int;
type GetErr = unsafe extern "C" fn(*mut c_void) -> *mut c_char;
type Close = unsafe extern "C" fn(*mut c_void);
type LibVersion = unsafe extern "C" fn() -> *const c_char;

pub struct Pcap {
    _lib: libloading::Library,
    open_live: OpenLive,
    sendpacket: SendPacket,
    compile: Compile,
    setfilter: SetFilter,
    freecode: FreeCode,
    next_ex: NextEx,
    geterr: GetErr,
    close: Close,
    lib_version: LibVersion,
}

const ERRBUF_SIZE: usize = 256;

fn load_library() -> Result<libloading::Library, String> {
    if !crate::sysinfo::npcap_installed() {
        return Err(crate::sysinfo::npcap_missing_message());
    }
    let system32 = crate::sysinfo::system32();
    let candidates = [system32.join("Npcap").join("wpcap.dll"), system32.join("wpcap.dll")];
    let mut last_error = String::new();
    for path in candidates.iter().filter(|p| p.exists()) {
        #[cfg(windows)]
        let lib = unsafe {
            // Resolve wpcap.dll's own dependencies (Packet.dll) from its directory
            libloading::os::windows::Library::load_with_flags(
                path,
                libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH,
            )
            .map(libloading::Library::from)
        };
        #[cfg(not(windows))]
        let lib = unsafe { libloading::Library::new(path) };
        match lib {
            Ok(lib) => return Ok(lib),
            Err(e) => last_error = format!("cannot load {}: {e}", path.display()),
        }
    }
    Err(last_error)
}

impl Pcap {
    fn load() -> Result<Pcap, String> {
        let lib = load_library()?;
        unsafe {
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("wpcap.dll: {}: {e}", $name))?
                };
            }
            Ok(Pcap {
                open_live: sym!("pcap_open_live"),
                sendpacket: sym!("pcap_sendpacket"),
                compile: sym!("pcap_compile"),
                setfilter: sym!("pcap_setfilter"),
                freecode: sym!("pcap_freecode"),
                next_ex: sym!("pcap_next_ex"),
                geterr: sym!("pcap_geterr"),
                close: sym!("pcap_close"),
                lib_version: sym!("pcap_lib_version"),
                _lib: lib,
            })
        }
    }

    pub fn version(&self) -> String {
        unsafe { CStr::from_ptr((self.lib_version)()).to_string_lossy().into_owned() }
    }
}

/// The loaded library (loaded once per process)
pub fn pcap() -> Result<&'static Pcap, String> {
    static PCAP: OnceLock<Result<Pcap, String>> = OnceLock::new();
    PCAP.get_or_init(Pcap::load).as_ref().map_err(|e| e.clone())
}

/// An open capture/injection handle on one device
pub struct Handle {
    pcap: &'static Pcap,
    p: *mut c_void,
}

// A handle is only ever used by the thread that owns it
unsafe impl Send for Handle {}

pub enum Next {
    Packet(Vec<u8>),
    Timeout,
}

impl Handle {
    pub fn open(device: &str, promiscuous: bool, timeout_ms: i32) -> Result<Handle, String> {
        let pcap = pcap()?;
        let device_c = CString::new(device).map_err(|e| e.to_string())?;
        let mut errbuf = [0 as c_char; ERRBUF_SIZE];
        let p = unsafe {
            (pcap.open_live)(device_c.as_ptr(), 65535, promiscuous as c_int, timeout_ms, errbuf.as_mut_ptr())
        };
        if p.is_null() {
            let msg = unsafe { CStr::from_ptr(errbuf.as_ptr()) }.to_string_lossy().into_owned();
            return Err(format!("cannot open {device}: {msg}"));
        }
        Ok(Handle { pcap, p })
    }

    fn last_error(&self) -> String {
        unsafe { CStr::from_ptr((self.pcap.geterr)(self.p)) }.to_string_lossy().into_owned()
    }

    pub fn set_filter(&mut self, expression: &str) -> Result<(), String> {
        let expr = CString::new(expression).map_err(|e| e.to_string())?;
        let mut program = BpfProgram { bf_len: 0, bf_insns: std::ptr::null_mut() };
        unsafe {
            if (self.pcap.compile)(self.p, &mut program, expr.as_ptr(), 1, 0xFFFF_FFFF) != 0 {
                return Err(format!("cannot compile filter '{expression}': {}", self.last_error()));
            }
            let rc = (self.pcap.setfilter)(self.p, &mut program);
            (self.pcap.freecode)(&mut program);
            if rc != 0 {
                return Err(format!("cannot set filter '{expression}': {}", self.last_error()));
            }
        }
        Ok(())
    }

    pub fn send(&mut self, frame: &[u8]) -> Result<(), String> {
        let rc = unsafe { (self.pcap.sendpacket)(self.p, frame.as_ptr(), frame.len() as c_int) };
        if rc != 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    pub fn next_packet(&mut self) -> Result<Next, String> {
        let mut header: *mut PktHdr = std::ptr::null_mut();
        let mut data: *const u8 = std::ptr::null();
        match unsafe { (self.pcap.next_ex)(self.p, &mut header, &mut data) } {
            1 => {
                let caplen = unsafe { (*header).caplen } as usize;
                Ok(Next::Packet(unsafe { std::slice::from_raw_parts(data, caplen) }.to_vec()))
            }
            0 => Ok(Next::Timeout),
            _ => Err(self.last_error()),
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { (self.pcap.close)(self.p) };
    }
}

pub const LLDP_MULTICAST_MAC: [u8; 6] = [0x01, 0x80, 0xc2, 0x00, 0x00, 0x0e];
pub const LLDP_ETHERTYPE: u16 = 0x88cc;

/// Ethernet II frame, padded to the 60-byte minimum
pub fn ethernet_frame(dst: &[u8; 6], src: &[u8], ethertype: u16, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(14 + payload.len().max(46));
    frame.extend_from_slice(dst);
    frame.extend_from_slice(src);
    frame.extend_from_slice(&ethertype.to_be_bytes());
    frame.extend_from_slice(payload);
    if frame.len() < 60 {
        frame.resize(60, 0);
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_layout_and_padding() {
        let src = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let frame = ethernet_frame(&LLDP_MULTICAST_MAC, &src, LLDP_ETHERTYPE, &[1, 2, 3]);
        assert_eq!(frame.len(), 60);
        assert_eq!(&frame[0..6], &LLDP_MULTICAST_MAC);
        assert_eq!(&frame[6..12], &src);
        assert_eq!(&frame[12..14], &[0x88, 0xcc]);
        assert_eq!(&frame[14..17], &[1, 2, 3]);
        let big = ethernet_frame(&LLDP_MULTICAST_MAC, &src, LLDP_ETHERTYPE, &[0; 100]);
        assert_eq!(big.len(), 114);
    }
}
