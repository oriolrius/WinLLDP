//! Sending raw Ethernet frames with XDP for Windows (Microsoft, MIT license): AF_XDP sockets.
//!
//! The signed driver package is embedded in winlldp.exe (`vendor/xdp-for-windows/`) and installed
//! the first time it is needed, the way the package's `xdp-setup.ps1 -Install xdp` does; WinLLDP
//! removes it again when it uninstalls its service, if it was the one that installed it.

use std::ffi::c_void;
use std::mem::size_of;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::ptr::null_mut;
use std::sync::atomic::{fence, AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub const XDP_VERSION: &str = "1.4.0";

macro_rules! vendored {
    ($name:literal) => {
        ($name, include_bytes!(concat!("../vendor/xdp-for-windows/1.4.0/", $name)) as &[u8])
    };
}

/// The files `xdp.inf` installs (the catalog signs them: they are used unmodified)
const PACKAGE: [(&str, &[u8]); 5] = [
    vendored!("xdp.inf"),
    vendored!("xdp.cat"),
    vendored!("xdp.sys"),
    vendored!("xdpapi.dll"),
    vendored!("xdppcw.man"),
];

const MARKER: &str = "installed-by-winlldp";

/// Where WinLLDP extracts the driver package: %ProgramFiles%\WinLLDP\xdp-<version>
pub fn package_dir() -> PathBuf {
    crate::paths::program_directory().join(format!("xdp-{XDP_VERSION}"))
}

fn marker_file() -> PathBuf {
    crate::paths::program_directory().join(MARKER)
}

/// The XDP version WinLLDP installed, if it installed XDP
pub fn installed_by_winlldp() -> Option<String> {
    std::fs::read_to_string(marker_file()).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn system32() -> PathBuf {
    crate::sysinfo::system32()
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(system32().join(program))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(text)
    } else {
        Err(format!("{program} {} failed ({}): {}", args.join(" "), output.status, text.trim()))
    }
}

pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

// ---------------------------------------------------------------------------------------------
// AF_XDP API version 3: talks to the driver directly, as the SDK's header-only implementation
// (xdp/details/ioctldef.h, ioctlfn.h, afxdp.h) does; xdpapi.dll is not used

type Handle = *mut c_void;

const XDP_API_VERSION_3: u32 = 3;
const XDP_OBJECT_TYPE_XSK: u32 = 1;
/// Device for AF_XDP sockets; drivers before 1.4 only have `\Device\xdp`
const XSK_DEVICE_NAMES: [&str; 2] = [r"\Device\xdpapi\xsk", r"\Device\xdp"];
const OPEN_PACKET_NAME: &[u8] = b"XdpOpenPacket000\0";

/// CTL_CODE(FILE_DEVICE_NETWORK, function, method, FILE_WRITE_ACCESS)
const fn xsk_ioctl(function: u32, method: u32) -> u32 {
    (0x12 << 16) | (2 << 14) | (function << 2) | method
}
const METHOD_BUFFERED: u32 = 0;
const METHOD_NEITHER: u32 = 3;
const IOCTL_XSK_BIND: u32 = xsk_ioctl(0, METHOD_BUFFERED);
const IOCTL_XSK_ACTIVATE: u32 = xsk_ioctl(1, METHOD_BUFFERED);
const IOCTL_XSK_GET_SOCKOPT: u32 = xsk_ioctl(2, METHOD_BUFFERED);
const IOCTL_XSK_SET_SOCKOPT: u32 = xsk_ioctl(3, METHOD_BUFFERED);
const IOCTL_XSK_NOTIFY: u32 = xsk_ioctl(4, METHOD_NEITHER);

#[repr(C)]
struct XskBindIn {
    if_index: u32,
    queue_id: u32,
    flags: u32,
}

#[repr(C)]
struct XskSetSockoptIn {
    option: u32,
    input_buffer_length: u32,
    input_buffer: *const c_void,
}

#[repr(C)]
struct XskNotifyIn {
    flags: u32,
    wait_timeout_milliseconds: u32,
}

/// The extended attribute that opens an XSK object for an API version:
/// XDP_FILE_FULL_EA_INFORMATION { NextEntryOffset, Flags, EaNameLength, EaValueLength, EaName }
/// followed by XDP_OPEN_PACKET { MajorVersion 1, MinorVersion 0, ApiVersion, ObjectType }
fn open_packet_ea(api_version: u32, object_type: u32) -> Vec<u8> {
    const HEADER: usize = 12; // sizeof(XDP_FILE_FULL_EA_INFORMATION), EaName[1] included and padded
    const OPEN_PACKET: usize = 12;
    let total = HEADER + OPEN_PACKET_NAME.len() + OPEN_PACKET;
    let mut ea = vec![0u8; total];
    ea[5] = (OPEN_PACKET_NAME.len() - 1) as u8;
    ea[6..8].copy_from_slice(&((total - HEADER - OPEN_PACKET_NAME.len()) as u16).to_le_bytes());
    ea[8..8 + OPEN_PACKET_NAME.len()].copy_from_slice(OPEN_PACKET_NAME);
    let packet = 8 + OPEN_PACKET_NAME.len();
    ea[packet..packet + 2].copy_from_slice(&1u16.to_le_bytes());
    ea[packet + 2..packet + 4].copy_from_slice(&0u16.to_le_bytes());
    ea[packet + 4..packet + 8].copy_from_slice(&api_version.to_le_bytes());
    ea[packet + 8..packet + 12].copy_from_slice(&object_type.to_le_bytes());
    ea
}

fn nt_error(what: &str, status: i32) -> String {
    #[cfg(windows)]
    let detail = {
        let code = unsafe { windows_sys::Win32::Foundation::RtlNtStatusToDosError(status) };
        std::io::Error::from_raw_os_error(code as i32).to_string()
    };
    #[cfg(not(windows))]
    let detail = String::new();
    format!("{what} failed: NTSTATUS 0x{:08X} ({detail})", status as u32)
}

#[cfg(windows)]
mod nt {
    use super::*;
    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::{NtCreateFile, FILE_CREATE};
    use windows_sys::Wdk::System::IO::NtDeviceIoControlFile;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, STATUS_PENDING, UNICODE_STRING, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        SetFileCompletionNotificationModes, FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
    };
    use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const FILE_SKIP_SET_EVENT_ON_HANDLE: u8 = 0x2;
    const STATUS_OBJECT_NAME_NOT_FOUND: i32 = 0xC000_0034_u32 as i32;
    const STATUS_OBJECT_PATH_NOT_FOUND: i32 = 0xC000_003A_u32 as i32;

    fn open_device(name: &str, ea: &[u8]) -> Result<Handle, i32> {
        let wide: Vec<u16> = name.encode_utf16().collect();
        let unicode = UNICODE_STRING {
            Length: (wide.len() * 2) as u16,
            MaximumLength: (wide.len() * 2) as u16,
            Buffer: wide.as_ptr() as *mut u16,
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &unicode,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut handle: Handle = std::ptr::null_mut();
        let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                GENERIC_READ | GENERIC_WRITE | SYNCHRONIZE,
                &attributes,
                &mut iosb,
                std::ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_CREATE,
                0,
                ea.as_ptr() as *const c_void,
                ea.len() as u32,
            )
        };
        if status < 0 {
            return Err(status);
        }
        Ok(handle)
    }

    /// XskCreate: a new AF_XDP socket
    pub fn create_socket() -> Result<Handle, String> {
        let ea = open_packet_ea(XDP_API_VERSION_3, XDP_OBJECT_TYPE_XSK);
        let mut last = 0;
        for name in XSK_DEVICE_NAMES {
            match open_device(name, &ea) {
                Ok(handle) => {
                    if unsafe { SetFileCompletionNotificationModes(handle, FILE_SKIP_SET_EVENT_ON_HANDLE) } == 0 {
                        let error = std::io::Error::last_os_error();
                        close(handle);
                        return Err(format!("XskCreate failed: {error}"));
                    }
                    return Ok(handle);
                }
                Err(status) if status == STATUS_OBJECT_NAME_NOT_FOUND || status == STATUS_OBJECT_PATH_NOT_FOUND => {
                    last = status;
                }
                Err(status) => return Err(nt_error("XskCreate", status)),
            }
        }
        Err(nt_error("XskCreate (XDP driver not loaded)", last))
    }

    /// One IOCTL, waiting for it when the driver completes it later; returns IoStatus.Information
    pub fn ioctl(
        handle: Handle,
        code: u32,
        input: *const c_void,
        input_len: usize,
        output: *mut c_void,
        output_len: usize,
    ) -> Result<usize, i32> {
        let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(STATUS_PENDING);
        }
        let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        iosb.Anonymous.Status = STATUS_PENDING;
        let mut status = unsafe {
            NtDeviceIoControlFile(
                handle,
                event,
                None,
                std::ptr::null(),
                &mut iosb,
                code,
                input,
                input_len as u32,
                output,
                output_len as u32,
            )
        };
        if status == STATUS_PENDING {
            status = if unsafe { WaitForSingleObject(event, INFINITE) } == WAIT_OBJECT_0 {
                unsafe { iosb.Anonymous.Status }
            } else {
                STATUS_PENDING
            };
        }
        unsafe { CloseHandle(event) };
        if status < 0 {
            return Err(status);
        }
        Ok(iosb.Information)
    }

    pub fn close(handle: Handle) {
        unsafe { CloseHandle(handle) };
    }
}

#[cfg(not(windows))]
mod nt {
    use super::*;

    pub fn create_socket() -> Result<Handle, String> {
        Err("XDP for Windows is only available on Windows".into())
    }

    pub fn ioctl(_: Handle, _: u32, _: *const c_void, _: usize, _: *mut c_void, _: usize) -> Result<usize, i32> {
        Err(-1)
    }

    pub fn close(_: Handle) {}
}

fn set_sockopt(handle: Handle, option: u32, value: *const c_void, len: usize, what: &str) -> Result<(), String> {
    let input = XskSetSockoptIn { option, input_buffer_length: len as u32, input_buffer: value };
    nt::ioctl(
        handle,
        IOCTL_XSK_SET_SOCKOPT,
        &input as *const _ as *const c_void,
        std::mem::size_of::<XskSetSockoptIn>(),
        std::ptr::null_mut(),
        0,
    )
    .map(|_| ())
    .map_err(|status| nt_error(what, status))
}

/// The driver is loaded and AF_XDP sockets can be created
pub fn usable() -> bool {
    match nt::create_socket() {
        Ok(handle) => {
            nt::close(handle);
            true
        }
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------------------------
// Installation

fn extract_package() -> Result<PathBuf, String> {
    let dir = package_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for (name, bytes) in PACKAGE {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
    }
    Ok(dir)
}

fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if done() {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Install the embedded driver package (administrator rights needed)
pub fn install() -> Result<(), String> {
    if !is_admin() {
        return Err("Installing the XDP driver needs administrator rights: run winlldp as administrator".into());
    }
    let dir = extract_package()?;
    crate::logger::info(&format!("Installing XDP for Windows {XDP_VERSION} from {}", dir.display()));
    let inf = dir.join("xdp.inf").display().to_string();
    run("netcfg.exe", &["-v", "-l", &inf, "-c", "s", "-i", "ms_xdp"])?;
    // Performance counters, as xdp-setup.ps1 registers them (not needed to send)
    let manifest = dir.join("xdppcw.man").display().to_string();
    let drivers = format!("{}\\", system32().join("drivers").display());
    if let Err(e) = run("lodctr.exe", &[&format!("/m:{manifest}"), &drivers]) {
        crate::logger::warning(&format!("XDP performance counters not registered: {e}"));
    }
    std::fs::write(marker_file(), XDP_VERSION).map_err(|e| format!("cannot write {}: {e}", marker_file().display()))?;
    if !wait_until(Duration::from_secs(20), usable) {
        return Err("The XDP driver was installed but does not respond".into());
    }
    crate::logger::info(&format!("XDP for Windows {XDP_VERSION} installed"));
    Ok(())
}

/// The driver store copy of our xdp.inf (oemNN.inf), found by content: pnputil's text is localized
fn staged_inf() -> Option<String> {
    let inf_dir = system32().parent()?.join("INF");
    let ours = PACKAGE[0].1;
    std::fs::read_dir(inf_dir).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        (name.starts_with("oem") && name.ends_with(".inf") && std::fs::read(entry.path()).ok().as_deref() == Some(ours))
            .then_some(name)
    })
}

fn service_exists() -> bool {
    Command::new(system32().join("sc.exe"))
        .args(["query", "xdp"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Remove the driver WinLLDP installed (as xdp-setup.ps1 -Uninstall xdp does)
pub fn uninstall() -> Result<(), String> {
    if installed_by_winlldp().is_none() {
        return Err("XDP was not installed by WinLLDP: leaving it in place".into());
    }
    if !is_admin() {
        return Err("Removing the XDP driver needs administrator rights: run winlldp as administrator".into());
    }
    let dir = extract_package()?;
    let manifest = dir.join("xdppcw.man").display().to_string();
    let _ = run("unlodctr.exe", &[&format!("/m:{manifest}")]);
    run("netcfg.exe", &["-u", "ms_xdp"])?;
    if let Some(oem) = staged_inf() {
        let _ = run("pnputil.exe", &["/delete-driver", &oem]);
    }
    if !wait_until(Duration::from_secs(10), || !service_exists()) {
        let _ = run("sc.exe", &["delete", "xdp"]);
    }
    for leftover in [system32().join("xdpapi.dll"), system32().join("drivers").join("xdp.sys")] {
        let _ = std::fs::remove_file(leftover);
    }
    let _ = std::fs::remove_file(marker_file());
    let _ = std::fs::remove_dir_all(&dir);
    // Leave %ProgramFiles%\WinLLDP only if something else (NSSM) still lives there
    let _ = std::fs::remove_dir(crate::paths::program_directory());
    crate::logger::info("XDP for Windows removed");
    Ok(())
}

/// Make sure XDP can send: use what is installed, install it, or upgrade the copy WinLLDP installed
pub fn ensure_available() -> Result<(), String> {
    let ours = installed_by_winlldp();
    if usable() && ours.as_deref().is_none_or(|v| v == XDP_VERSION) {
        return Ok(());
    }
    if ours.is_some_and(|v| v != XDP_VERSION) {
        uninstall()?;
    }
    install()
}

// ---------------------------------------------------------------------------------------------
// AF_XDP transmit socket

const XSK_SOCKOPT_UMEM_REG: u32 = 1;
const XSK_SOCKOPT_TX_RING_SIZE: u32 = 4;
const XSK_SOCKOPT_TX_COMPLETION_RING_SIZE: u32 = 5;
const XSK_SOCKOPT_RING_INFO: u32 = 6;
const XSK_BIND_FLAG_TX: u32 = 0x2;
const XSK_BIND_FLAG_GENERIC: u32 = 0x4;
const XSK_NOTIFY_FLAG_POKE_TX: u32 = 0x2;
const XSK_NOTIFY_FLAG_WAIT_TX: u32 = 0x8;

#[repr(C)]
struct UmemReg {
    total_size: u64,
    chunk_size: u32,
    headroom: u32,
    address: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct RingInfo {
    ring: usize,
    descriptors_offset: u32,
    producer_index_offset: u32,
    consumer_index_offset: u32,
    flags_offset: u32,
    size: u32,
    element_stride: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct RingInfoSet {
    fill: RingInfo,
    completion: RingInfo,
    rx: RingInfo,
    tx: RingInfo,
}

impl RingInfo {
    fn index(&self, offset: u32) -> &AtomicU32 {
        unsafe { &*((self.ring + offset as usize) as *const AtomicU32) }
    }
}

const UMEM_SIZE: usize = 64 * 1024;
const CHUNK_SIZE: u32 = 4096;
const RING_SIZE: u32 = 8;

/// Page-aligned memory shared with the driver
struct Umem(*mut u8);

impl Umem {
    fn layout() -> std::alloc::Layout {
        std::alloc::Layout::from_size_align(UMEM_SIZE, 4096).unwrap()
    }

    fn new() -> Umem {
        let ptr = unsafe { std::alloc::alloc_zeroed(Self::layout()) };
        assert!(!ptr.is_null(), "out of memory");
        Umem(ptr)
    }
}

impl Drop for Umem {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.0, Self::layout()) };
    }
}

/// A transmit-only AF_XDP socket bound to queue 0 of an interface (generic XDP)
pub struct Socket {
    handle: Handle,
    // Declared after `handle`: freed after the socket is closed
    umem: Umem,
    rings: RingInfoSet,
}

impl Socket {
    pub fn open(if_index: u32) -> Result<Socket, String> {
        let handle = nt::create_socket()?;
        let mut socket = Socket { handle, umem: Umem::new(), rings: RingInfoSet::default() };
        let reg = UmemReg {
            total_size: UMEM_SIZE as u64,
            chunk_size: CHUNK_SIZE,
            headroom: 0,
            address: socket.umem.0 as *mut c_void,
        };
        set_sockopt(handle, XSK_SOCKOPT_UMEM_REG, &reg as *const _ as *const c_void, size_of::<UmemReg>(), "UMEM")?;
        let size = RING_SIZE;
        let size_ptr = &size as *const u32 as *const c_void;
        set_sockopt(handle, XSK_SOCKOPT_TX_RING_SIZE, size_ptr, 4, "TX ring")?;
        set_sockopt(handle, XSK_SOCKOPT_TX_COMPLETION_RING_SIZE, size_ptr, 4, "completion ring")?;
        let bind = XskBindIn { if_index, queue_id: 0, flags: XSK_BIND_FLAG_TX | XSK_BIND_FLAG_GENERIC };
        nt::ioctl(handle, IOCTL_XSK_BIND, &bind as *const _ as *const c_void, size_of::<XskBindIn>(), null_mut(), 0)
            .map_err(|status| nt_error(&format!("XskBind (interface {if_index})"), status))?;
        let activate = 0u32;
        nt::ioctl(handle, IOCTL_XSK_ACTIVATE, &activate as *const _ as *const c_void, 4, null_mut(), 0)
            .map_err(|status| nt_error("XskActivate", status))?;
        let option = XSK_SOCKOPT_RING_INFO;
        let rings = &mut socket.rings as *mut RingInfoSet as *mut c_void;
        let returned = nt::ioctl(
            handle,
            IOCTL_XSK_GET_SOCKOPT,
            &option as *const _ as *const c_void,
            4,
            rings,
            size_of::<RingInfoSet>(),
        )
        .map_err(|status| nt_error("ring info", status))?;
        if returned < size_of::<RingInfoSet>() {
            return Err(format!("ring info: {returned} bytes returned"));
        }
        Ok(socket)
    }

    /// Transmit one frame and wait (up to 2 s) until the driver completes it
    pub fn send(&mut self, frame: &[u8]) -> Result<(), String> {
        if frame.len() > CHUNK_SIZE as usize {
            return Err(format!("frame of {} bytes is too large", frame.len()));
        }
        unsafe { std::ptr::copy_nonoverlapping(frame.as_ptr(), self.umem.0, frame.len()) };

        let tx = self.rings.tx;
        let producer = tx.index(tx.producer_index_offset);
        let index = producer.load(Ordering::Relaxed);
        let slot = (index & (tx.size - 1)) as usize;
        let descriptor = (tx.ring + tx.descriptors_offset as usize + slot * tx.element_stride as usize) as *mut u64;
        unsafe {
            // XSK_BUFFER_DESCRIPTOR { Address: chunk 0, offset 0; Length; Reserved }
            descriptor.write(0);
            (descriptor.add(1) as *mut u32).write(frame.len() as u32);
            (descriptor.add(1) as *mut u32).add(1).write(0);
        }
        fence(Ordering::Release);
        producer.store(index.wrapping_add(1), Ordering::Release);

        let completion = self.rings.completion;
        let completion_producer = completion.index(completion.producer_index_offset);
        let completion_consumer = completion.index(completion.consumer_index_offset);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let notify = XskNotifyIn {
                flags: XSK_NOTIFY_FLAG_POKE_TX | XSK_NOTIFY_FLAG_WAIT_TX,
                wait_timeout_milliseconds: 500,
            };
            let notified = nt::ioctl(
                self.handle,
                IOCTL_XSK_NOTIFY,
                &notify as *const _ as *const c_void,
                size_of::<XskNotifyIn>(),
                null_mut(),
                0,
            );
            let done =
                completion_producer.load(Ordering::Acquire).wrapping_sub(completion_consumer.load(Ordering::Relaxed));
            if done > 0 {
                completion_consumer.store(completion_producer.load(Ordering::Acquire), Ordering::Release);
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(match notified {
                    Err(status) => nt_error("XDP transmission", status),
                    Ok(_) => "XDP did not complete the transmission".into(),
                });
            }
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        nt::close(self.handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_package_matches_sha256sums() {
        // The catalog signature only holds for the unmodified files
        let sums = include_str!("../vendor/xdp-for-windows/1.4.0/SHA256SUMS");
        for (name, bytes) in PACKAGE {
            assert!(sums.contains(name), "{name} missing from SHA256SUMS");
            assert!(!bytes.is_empty(), "{name} is empty");
        }
        assert!(String::from_utf8_lossy(PACKAGE[0].1).contains("DriverVer = 07/29/2026,1.4.0.0"));
    }

    #[test]
    fn layouts_match_afxdp_h() {
        assert_eq!(std::mem::size_of::<UmemReg>(), 24);
        assert_eq!(std::mem::size_of::<RingInfo>(), 40);
        assert_eq!(std::mem::size_of::<RingInfoSet>(), 160);
        assert_eq!(size_of::<XskBindIn>(), 12);
        assert_eq!(size_of::<XskSetSockoptIn>(), 16);
        assert_eq!(size_of::<XskNotifyIn>(), 8);
    }

    #[test]
    fn ioctl_codes_match_afxdp_h() {
        // CTL_CODE(FILE_DEVICE_NETWORK, n, method, FILE_WRITE_ACCESS)
        assert_eq!(IOCTL_XSK_BIND, 0x0012_8000);
        assert_eq!(IOCTL_XSK_ACTIVATE, 0x0012_8004);
        assert_eq!(IOCTL_XSK_GET_SOCKOPT, 0x0012_8008);
        assert_eq!(IOCTL_XSK_SET_SOCKOPT, 0x0012_800C);
        assert_eq!(IOCTL_XSK_NOTIFY, 0x0012_8013);
    }

    #[test]
    fn open_packet_extended_attribute() {
        // XDP_OPEN_EA_LENGTH = 12 + sizeof("XdpOpenPacket000") + 12
        let ea = open_packet_ea(XDP_API_VERSION_3, XDP_OBJECT_TYPE_XSK);
        assert_eq!(ea.len(), 41);
        assert_eq!(&ea[0..4], &[0, 0, 0, 0]); // NextEntryOffset
        assert_eq!((ea[4], ea[5]), (0, 16)); // Flags, EaNameLength (without the NUL)
        assert_eq!(u16::from_le_bytes([ea[6], ea[7]]), 12); // EaValueLength
        assert_eq!(&ea[8..25], b"XdpOpenPacket000\0");
        assert_eq!(&ea[25..37], &[1, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0]); // 1.0, API 3, XSK
                                                                        // sizeof(XDP_FILE_FULL_EA_INFORMATION) counts EaName[1] and its padding, as the C macro does
        assert_eq!(&ea[37..], &[0, 0, 0, 0]);
    }

    #[test]
    fn package_lives_in_program_files() {
        assert!(package_dir().starts_with(crate::paths::program_directory()));
        assert!(package_dir().ends_with(format!("xdp-{XDP_VERSION}")));
    }
}
