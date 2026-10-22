//! Sending raw Ethernet frames with XDP for Windows (Microsoft, MIT license): AF_XDP sockets.
//!
//! The signed driver package is embedded in winlldp.exe (`vendor/xdp-for-windows/`) and installed
//! the first time it is needed, the way the package's `xdp-setup.ps1 -Install xdp` does; WinLLDP
//! removes it again when it uninstalls its service, if it was the one that installed it.

use std::ffi::c_void;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{fence, AtomicU32, Ordering};
use std::sync::Mutex;
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
// API (xdpapi.dll, function table of XDP_API_VERSION_2)

type Handle = *mut c_void;

#[repr(C)]
struct ApiTable {
    xdp_open_api: *const c_void,
    xdp_close_api: *const c_void,
    xdp_get_routine: *const c_void,
    xdp_create_program: *const c_void,
    xdp_interface_open: *const c_void,
    xsk_create: unsafe extern "system" fn(*mut Handle) -> i32,
    xsk_bind: unsafe extern "system" fn(Handle, u32, u32, u32) -> i32,
    xsk_activate: unsafe extern "system" fn(Handle, u32) -> i32,
    xsk_notify_socket: unsafe extern "system" fn(Handle, u32, u32, *mut u32) -> i32,
    xsk_notify_async: *const c_void,
    xsk_get_notify_async_result: *const c_void,
    xsk_set_sockopt: unsafe extern "system" fn(Handle, u32, *const c_void, u32) -> i32,
    xsk_get_sockopt: unsafe extern "system" fn(Handle, u32, *mut c_void, *mut u32) -> i32,
    xsk_ioctl: *const c_void,
}

struct Api {
    _lib: libloading::Library,
    table: &'static ApiTable,
}

// The table is immutable function pointers
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

const API_VERSION_2: u32 = 2;

fn load_api() -> Result<Api, String> {
    let path = system32().join("xdpapi.dll");
    let lib = unsafe { libloading::Library::new(&path) }.map_err(|e| format!("{}: {e}", path.display()))?;
    let open: libloading::Symbol<unsafe extern "system" fn(u32, *mut *const ApiTable) -> i32> =
        unsafe { lib.get(b"XdpOpenApi\0") }.map_err(|e| format!("xdpapi.dll: {e}"))?;
    let mut table: *const ApiTable = std::ptr::null();
    let hr = unsafe { open(API_VERSION_2, &mut table) };
    if hr < 0 || table.is_null() {
        return Err(format!("XdpOpenApi failed: HRESULT 0x{:08X}", hr as u32));
    }
    Ok(Api { table: unsafe { &*table }, _lib: lib })
}

/// The API once loaded (not cached while it is missing, so that an install can follow)
fn api() -> Result<&'static Api, String> {
    static API: Mutex<Option<&'static Api>> = Mutex::new(None);
    let mut slot = API.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(api) = *slot {
        return Ok(api);
    }
    let api: &'static Api = Box::leak(Box::new(load_api()?));
    *slot = Some(api);
    Ok(api)
}

fn check(what: &str, hr: i32) -> Result<(), String> {
    if hr < 0 {
        return Err(format!("{what} failed: HRESULT 0x{:08X}", hr as u32));
    }
    Ok(())
}

fn close_handle(handle: Handle) {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(handle);
    }
}

/// The driver is loaded and the API can create sockets
pub fn usable() -> bool {
    let Ok(api) = api() else {
        return false;
    };
    let mut socket: Handle = std::ptr::null_mut();
    if unsafe { (api.table.xsk_create)(&mut socket) } < 0 {
        return false;
    }
    close_handle(socket);
    true
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
    api: &'static Api,
    handle: Handle,
    // Declared after `handle`: freed after the socket is closed
    umem: Umem,
    rings: RingInfoSet,
}

impl Socket {
    pub fn open(if_index: u32) -> Result<Socket, String> {
        let api = api()?;
        let t = api.table;
        let mut handle: Handle = std::ptr::null_mut();
        check("XskCreate", unsafe { (t.xsk_create)(&mut handle) })?;
        let mut socket = Socket { api, handle, umem: Umem::new(), rings: RingInfoSet::default() };
        let set = |option: u32, value: *const c_void, len: usize, what: &str| {
            check(what, unsafe { (t.xsk_set_sockopt)(handle, option, value, len as u32) })
        };
        let reg = UmemReg {
            total_size: UMEM_SIZE as u64,
            chunk_size: CHUNK_SIZE,
            headroom: 0,
            address: socket.umem.0 as *mut c_void,
        };
        set(XSK_SOCKOPT_UMEM_REG, &reg as *const _ as *const c_void, std::mem::size_of::<UmemReg>(), "UMEM")?;
        let size = RING_SIZE;
        set(XSK_SOCKOPT_TX_RING_SIZE, &size as *const _ as *const c_void, 4, "TX ring")?;
        set(XSK_SOCKOPT_TX_COMPLETION_RING_SIZE, &size as *const _ as *const c_void, 4, "completion ring")?;
        check(&format!("XskBind (interface {if_index})"), unsafe {
            (t.xsk_bind)(handle, if_index, 0, XSK_BIND_FLAG_TX | XSK_BIND_FLAG_GENERIC)
        })?;
        check("XskActivate", unsafe { (t.xsk_activate)(handle, 0) })?;
        let mut len = std::mem::size_of::<RingInfoSet>() as u32;
        check("ring info", unsafe {
            (t.xsk_get_sockopt)(handle, XSK_SOCKOPT_RING_INFO, &mut socket.rings as *mut _ as *mut c_void, &mut len)
        })?;
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
            let mut result = 0u32;
            let hr = unsafe {
                (self.api.table.xsk_notify_socket)(
                    self.handle,
                    XSK_NOTIFY_FLAG_POKE_TX | XSK_NOTIFY_FLAG_WAIT_TX,
                    500,
                    &mut result,
                )
            };
            let done =
                completion_producer.load(Ordering::Acquire).wrapping_sub(completion_consumer.load(Ordering::Relaxed));
            if done > 0 {
                completion_consumer.store(completion_producer.load(Ordering::Acquire), Ordering::Release);
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!("XDP did not complete the transmission (HRESULT 0x{:08X})", hr as u32));
            }
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        close_handle(self.handle);
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
        assert_eq!(std::mem::size_of::<ApiTable>(), 14 * 8);
    }

    #[test]
    fn package_lives_in_program_files() {
        assert!(package_dir().starts_with(crate::paths::program_directory()));
        assert!(package_dir().ends_with(format!("xdp-{XDP_VERSION}")));
    }
}
