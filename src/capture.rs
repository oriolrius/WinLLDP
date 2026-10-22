//! LLDP capture: the background process that records neighbors in neighbors.json,
//! and the commands that start, stop and inspect it

use crate::config::Config;
use crate::packet::{format_mac, LldpPacket};
use crate::pcap::{self, Handle, Next};
use crate::pktmon;
use crate::process::{self, ProcessInfo};
use crate::sysinfo::{self, Interface};
use chrono::Local;
use serde_json::{json, Map, Value};
use std::ffi::c_void;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub const CAPTURE_FILTER: &str = "ether proto 0x88cc";

/// LLDP_CAPTURE_BACKEND: `auto` (default: Packet Monitor, else Npcap), `pktmon` or `npcap`
pub const BACKEND_VARIABLE: &str = "LLDP_CAPTURE_BACKEND";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    PacketMonitor,
    Npcap,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::PacketMonitor => "Packet Monitor",
            Backend::Npcap => "Npcap",
        }
    }

    fn available(self) -> bool {
        match self {
            Backend::PacketMonitor => pktmon::api().is_ok(),
            Backend::Npcap => sysinfo::npcap_installed(),
        }
    }
}

/// Backends to try, in order, according to LLDP_CAPTURE_BACKEND
pub fn backend_order(setting: Option<&str>) -> Result<Vec<Backend>, String> {
    match setting.map(|s| s.trim().to_lowercase()).as_deref() {
        None | Some("") | Some("auto") => Ok(vec![Backend::PacketMonitor, Backend::Npcap]),
        Some("pktmon") | Some("packetmonitor") => Ok(vec![Backend::PacketMonitor]),
        Some("npcap") => Ok(vec![Backend::Npcap]),
        Some(other) => Err(format!("Invalid {BACKEND_VARIABLE} '{other}' (use auto, pktmon or npcap)")),
    }
}

fn configured_backends() -> Result<Vec<Backend>, String> {
    backend_order(std::env::var(BACKEND_VARIABLE).ok().as_deref())
}

/// Why no capture backend can be used here, if none can
pub fn capture_unavailable_reason() -> Option<String> {
    let backends = match configured_backends() {
        Ok(backends) => backends,
        Err(e) => return Some(e),
    };
    if backends.iter().any(|b| b.available()) {
        return None;
    }
    Some(match backends.as_slice() {
        [Backend::Npcap] => sysinfo::npcap_missing_message(),
        [Backend::PacketMonitor] => {
            "The Windows Packet Monitor API is not available (it needs Windows 11 24H2+ or Windows Server 2025)".into()
        }
        _ => format!(
            "Npcap is not installed and the Windows Packet Monitor API is not available (it needs Windows 11 \
             24H2+ or Windows Server 2025); one of them is required for packet capture. Install Npcap from {}",
            sysinfo::NPCAP_URL
        ),
    })
}

/// The backend recorded in the capture log by the running capture process
pub fn running_backend(log_file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(log_file).ok()?;
    let line = text.lines().rev().find(|l| l.contains("Capture backend: "))?;
    Some(line.split("Capture backend: ").nth(1)?.trim().to_string())
}

pub fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
}

pub fn read_pid(pid_file: &Path) -> Option<u32> {
    crate::fsdebug::read_to_string(pid_file).ok()?.trim().parse().ok()
}

pub fn is_capture_running(config: &Config) -> bool {
    read_pid(&config.pid_file).is_some_and(process::is_winlldp)
}

/// Make this process's console handles non-inheritable, so that the detached
/// capture process does not keep the caller's pipes open (a caller reading our
/// output would otherwise wait for the capture process to end)
fn protect_std_handles() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};
        use windows_sys::Win32::System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };
        for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = GetStdHandle(id);
            if !h.is_null() && h as isize != -1 {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// Start the capture process and wait until it is capturing; the error says why it is not
pub fn start_capture(config: &Config) -> Result<u32, String> {
    if is_capture_running(config) {
        crate::logger::info("Capture is already running");
        return Err("Capture is already running".into());
    }
    if let Some(message) = capture_unavailable_reason() {
        crate::logger::error(&message);
        return Err(message);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    protect_std_handles();
    let mut command = Command::new(exe);
    command
        .arg("capture-subprocess")
        .arg(&config.log_file)
        .arg(&config.neighbors_file)
        .arg(&config.pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|e| format!("Cannot start the capture process: {e}"))?;
    let pid = child.id();
    crate::logger::info(&format!("Started LLDP capture subprocess (PID: {pid})"));

    // The capture process writes its PID file once it is capturing
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            let reason = stderr.trim();
            let message = if reason.is_empty() {
                format!("Capture process exited immediately (see the log: {})", config.log_file.display())
            } else {
                reason.to_string()
            };
            crate::logger::error(&format!("Capture process exited immediately: {message}"));
            return Err(message);
        }
        if read_pid(&config.pid_file) == Some(pid) {
            return Ok(pid);
        }
        if Instant::now() > deadline {
            process::kill(pid);
            return Err(format!("Capture process did not start in time (see the log: {})", config.log_file.display()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Stop the capture process; returns its PID
pub fn stop_capture(config: &Config) -> Result<u32, String> {
    if !config.pid_file.exists() {
        crate::logger::info("No capture process found");
        return Err("No capture process found".into());
    }
    let pid = read_pid(&config.pid_file).ok_or_else(|| {
        let message = format!("Error stopping capture: invalid PID file {}", config.pid_file.display());
        crate::logger::error(&message);
        message
    })?;
    // Only kill it if it is still our capture process: the PID may have been reused
    if process::is_winlldp(pid) {
        process::kill(pid);
    }
    let _ = std::fs::remove_file(&config.pid_file);
    crate::logger::info(&format!("Stopped LLDP capture subprocess (PID: {pid})"));
    Ok(pid)
}

pub fn capture_status(config: &Config) -> Option<(u32, Option<ProcessInfo>)> {
    let pid = read_pid(&config.pid_file).filter(|pid| process::is_winlldp(*pid))?;
    Some((pid, process::process_info(pid, Duration::from_millis(100))))
}

/// Writes timestamped lines to the capture log
#[derive(Clone)]
struct CaptureLog(PathBuf);

impl CaptureLog {
    fn log(&self, msg: &str) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.0) {
            let _ = writeln!(f, "[{}] {msg}", now_iso());
        }
    }

    fn file_op(&self, operation: &str, path: &Path, mode: &str) {
        if mode.is_empty() {
            self.log(&format!("FILE_OP: {operation} '{}'", path.display()));
        } else {
            self.log(&format!("FILE_OP: {operation} '{}' mode={mode}", path.display()));
        }
    }
}

/// Add or refresh a neighbor in the neighbors map (keeps `first_seen`)
pub fn update_neighbors(
    neighbors: &mut Map<String, Value>,
    interface: &str,
    source_mac: &str,
    data: Map<String, Value>,
    now: &str,
) {
    let chassis_id = data.get("chassis_id").map(crate::py_str).unwrap_or_default();
    let key = format!("{interface}:{source_mac}:{chassis_id}");
    let first_seen = neighbors.get(&key).and_then(|n| n.get("first_seen")).cloned().unwrap_or_else(|| Value::from(now));
    let ttl = data.get("ttl").cloned().unwrap_or(json!(120));
    neighbors.insert(
        key,
        json!({
            "interface": interface,
            "source_mac": source_mac,
            "data": data,
            "first_seen": first_seen,
            "last_seen": now,
            "ttl": ttl,
        }),
    );
}

fn process_frame(frame: &[u8], interface: &str, neighbors_file: &Path, log: &CaptureLog) {
    if frame.len() < 14 || u16::from_be_bytes([frame[12], frame[13]]) != pcap::LLDP_ETHERTYPE {
        return;
    }
    let source_mac = format_mac(&frame[6..12]);
    log.log(&format!("Received LLDP packet from {source_mac} on {interface}"));
    let data = LldpPacket::decode(&frame[14..]).to_dict();
    log.log(&format!("Saving to file: {}", neighbors_file.display()));

    log.file_op("OPEN", neighbors_file, "r");
    let mut neighbors = match std::fs::read_to_string(neighbors_file).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(Value::Object(map)) => {
            log.file_op("CLOSE", neighbors_file, "");
            map
        }
        _ => {
            log.log("No existing neighbors file, creating new one");
            Map::new()
        }
    };
    update_neighbors(&mut neighbors, interface, &source_mac, data, &now_iso());
    log.file_op("OPEN", neighbors_file, "w");
    let text = serde_json::to_string_pretty(&Value::Object(neighbors.clone())).unwrap_or_else(|_| "{}".into());
    match std::fs::write(neighbors_file, text) {
        Ok(()) => {
            log.file_op("CLOSE", neighbors_file, "");
            log.log(&format!("Saved neighbor data to file ({} total neighbors)", neighbors.len()));
        }
        Err(e) => log.log(&format!("Error processing packet: {e}")),
    }
}

/// What the backends hand to the consumer
pub enum CaptureEvent {
    Frame { interface: String, frame: Vec<u8> },
    Stopped(String),
}

/// Drops the second report of the same frame on the same interface (Packet Monitor
/// reports a frame at every component it crosses; some map to the same interface)
#[derive(Default)]
pub struct Deduplicator {
    recent: Vec<(Instant, String, Vec<u8>)>,
}

impl Deduplicator {
    pub const WINDOW: Duration = Duration::from_secs(1);

    pub fn is_new(&mut self, now: Instant, interface: &str, frame: &[u8]) -> bool {
        self.recent.retain(|(seen, _, _)| now.duration_since(*seen) < Self::WINDOW);
        if self.recent.iter().any(|(_, i, f)| i == interface && f == frame) {
            return false;
        }
        self.recent.push((now, interface.to_string(), frame.to_vec()));
        true
    }
}

/// Shared by the backends: PID file, logging and the neighbors.json updates
struct CaptureContext<'a> {
    log: CaptureLog,
    neighbors_file: &'a Path,
    pid_file: &'a Path,
}

impl CaptureContext<'_> {
    /// Called once a backend is capturing: `capture start` waits for the PID file
    fn started(&self, backend: Backend, interfaces: &[String]) -> Result<(), String> {
        self.log.file_op("OPEN", self.pid_file, "w");
        std::fs::write(self.pid_file, std::process::id().to_string())
            .map_err(|e| format!("Cannot write the PID file {}: {e}", self.pid_file.display()))?;
        self.log.file_op("CLOSE", self.pid_file, "");
        self.log.log("PID file written");
        self.log.log(&format!("Capture backend: {}", backend.name()));
        self.log.log(&format!("Starting packet capture on: {}", interfaces.join(", ")));
        Ok(())
    }

    /// Process frames until the backend stops; returns the exit code
    fn consume(&self, events: Receiver<CaptureEvent>) -> i32 {
        let mut dedup = Deduplicator::default();
        for event in events {
            match event {
                CaptureEvent::Frame { interface, frame } => {
                    if dedup.is_new(Instant::now(), &interface, &frame) {
                        process_frame(&frame, &interface, self.neighbors_file, &self.log);
                    }
                }
                CaptureEvent::Stopped(reason) => {
                    self.log.log(&format!("Capture stopped: {reason}"));
                    return 1;
                }
            }
        }
        0
    }
}

/// Npcap: one handle per active interface, each read by its own thread
fn run_npcap(ctx: &CaptureContext, interfaces: &[Interface]) -> Result<i32, String> {
    ctx.log.log("Loading Npcap...");
    let pcap = pcap::pcap()?;
    ctx.log.log(&format!("Npcap loaded: {}", pcap.version()));

    let mut handles = Vec::new();
    let mut errors = Vec::new();
    for iface in interfaces {
        let opened = Handle::open(&iface.npcap_device(), true, 1000).and_then(|mut h| {
            h.set_filter(CAPTURE_FILTER)?;
            Ok(h)
        });
        match opened {
            Ok(h) => handles.push((iface.name.clone(), h)),
            Err(e) => {
                ctx.log.log(&format!("Cannot capture on {}: {e}", iface.name));
                errors.push(format!("{}: {e}", iface.name));
            }
        }
    }
    if handles.is_empty() {
        return Err(format!("Cannot capture on any interface ({})", errors.join("; ")));
    }
    let names: Vec<String> = handles.iter().map(|(n, _)| n.clone()).collect();
    ctx.started(Backend::Npcap, &names)?;

    let (tx, rx) = mpsc::channel();
    for (name, mut handle) in handles {
        let (tx, log) = (tx.clone(), ctx.log.clone());
        std::thread::spawn(move || loop {
            match handle.next_packet() {
                Ok(Next::Packet(frame)) => {
                    if tx.send(CaptureEvent::Frame { interface: name.clone(), frame }).is_err() {
                        break;
                    }
                }
                Ok(Next::Timeout) => {}
                Err(e) => {
                    log.log(&format!("Capture error on {name}: {e}"));
                    break;
                }
            }
        });
    }
    drop(tx);
    Ok(ctx.consume(rx))
}

/// Passed to the Packet Monitor callbacks as their context
struct StreamContext {
    components: std::collections::HashMap<u32, String>,
    events: Sender<CaptureEvent>,
}

unsafe extern "system" fn on_stream_event(context: *mut c_void, _info: *const c_void, kind: i32) {
    // 1 = stopped, 2 = fatal error
    if kind == 1 || kind == 2 {
        let ctx = &*(context as *const StreamContext);
        let reason = if kind == 2 { "Packet Monitor fatal error" } else { "Packet Monitor stream stopped" };
        let _ = ctx.events.send(CaptureEvent::Stopped(reason.into()));
    }
}

unsafe extern "system" fn on_stream_data(context: *mut c_void, data: *const pktmon::StreamDataDescriptor) {
    // Keep this short: copy the frame and hand it to the consumer
    let ctx = &*(context as *const StreamContext);
    let (frame, metadata) = (*data).packet();
    if let Some(interface) = ctx.components.get(&(metadata.component_id as u32)) {
        let _ = ctx.events.send(CaptureEvent::Frame { interface: interface.clone(), frame: frame.to_vec() });
    }
}

/// Packet Monitor (Windows 11 24H2+ / Server 2025): one live session on the active interfaces
fn run_pktmon(ctx: &CaptureContext, interfaces: &[Interface]) -> Result<i32, String> {
    ctx.log.log("Starting Windows Packet Monitor...");
    let monitor = pktmon::PacketMonitor::new()?;
    let all: Vec<pktmon::SourceInfo> =
        monitor.data_sources(pktmon::DATA_SOURCE_KIND_ALL, true)?.iter().map(pktmon::SourceInfo::from).collect();
    let targets: Vec<(String, String)> = interfaces.iter().map(|i| (i.name.clone(), i.description.clone())).collect();
    let components = pktmon::component_interfaces(&all, &targets);

    let mut session = monitor.session("WinLLDP")?;
    let mut names: Vec<String> = Vec::new();
    for source in monitor.data_sources(pktmon::DATA_SOURCE_KIND_NETWORK_INTERFACE, false)?.iter() {
        if let Some(interface) = components.get(&source.id) {
            session.add_data_source(source)?;
            if !names.contains(interface) {
                names.push(interface.clone());
            }
        }
    }
    if names.is_empty() {
        return Err("Packet Monitor does not see any of the active interfaces".into());
    }
    session.add_constraint(&pktmon::ProtocolConstraint::ether_type("LLDP", pcap::LLDP_ETHERTYPE))?;

    let (tx, rx) = mpsc::channel();
    // Lives as long as the session (the callbacks use it until the stream is closed)
    let context = Box::new(StreamContext { components, events: tx });
    let config = pktmon::RealtimeStreamConfiguration {
        user_context: &*context as *const StreamContext as *mut c_void,
        event_callback: Some(on_stream_event),
        data_callback: Some(on_stream_data),
        buffer_size_multiplier: 4,
        truncation_size: 9000,
    };
    session.attach_stream(&config)?;
    session.set_active(true)?;
    names.sort_by_key(|n| interfaces.iter().position(|i| &i.name == n));
    ctx.started(Backend::PacketMonitor, &names)?;
    let code = ctx.consume(rx);
    drop(session);
    drop(context);
    Ok(code)
}

/// Body of the hidden `capture-subprocess` command; returns the exit code
pub fn run_capture_process(log_file: &Path, neighbors_file: &Path, pid_file: &Path) -> i32 {
    let ctx = CaptureContext { log: CaptureLog(log_file.to_path_buf()), neighbors_file, pid_file };
    let log = &ctx.log;
    // Reported back to `capture start` through our stderr pipe
    let fatal = |message: &str| {
        log.log(&format!("FATAL ERROR: {message}"));
        let _ = writeln!(std::io::stderr(), "{message}");
    };
    log.log("Starting LLDP capture process");
    log.log(&format!("PID: {}", std::process::id()));
    log.log(&format!("Executable: {}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()));
    log.log(&format!("Neighbors file will be: {}", neighbors_file.display()));
    log.log(&format!("PID file will be: {}", pid_file.display()));

    let code = (|| {
        let backends = match configured_backends() {
            Ok(backends) => backends,
            Err(e) => {
                fatal(&e);
                return 1;
            }
        };
        log.log("Getting network interfaces...");
        let interfaces: Vec<Interface> =
            sysinfo::interfaces().into_iter().filter(|i| i.is_up && i.mac.is_some()).collect();
        let names: Vec<&str> = interfaces.iter().map(|i| i.name.as_str()).collect();
        log.log(&format!("Found interfaces: {names:?}"));
        if interfaces.is_empty() {
            log.log("ERROR: No active interfaces found!");
            fatal("No active interfaces found");
            return 1;
        }

        let mut errors = Vec::new();
        for backend in backends {
            let result = match backend {
                Backend::PacketMonitor => run_pktmon(&ctx, &interfaces),
                Backend::Npcap => run_npcap(&ctx, &interfaces),
            };
            match result {
                Ok(code) => return code,
                Err(e) => {
                    log.log(&format!("{} unavailable: {e}", backend.name()));
                    errors.push((backend, e));
                }
            }
        }
        let message = match errors.as_slice() {
            [(_, e)] => e.clone(),
            _ => errors.iter().map(|(b, e)| format!("{}: {e}", b.name())).collect::<Vec<_>>().join("; "),
        };
        fatal(&message);
        1
    })();

    if read_pid(pid_file) == Some(std::process::id()) {
        log.file_op("REMOVE", pid_file, "");
        if std::fs::remove_file(pid_file).is_ok() {
            log.log("PID file removed");
        }
    }
    log.log("Capture process exiting");
    code
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_keep_first_seen_and_refresh_the_rest() {
        let mut neighbors = Map::new();
        let data = |name: &str| {
            let mut m = Map::new();
            m.insert("chassis_id".into(), json!("6c:3b:6b:09:9c:90"));
            m.insert("system_name".into(), json!(name));
            m.insert("ttl".into(), json!(120));
            m
        };
        update_neighbors(
            &mut neighbors,
            "Ethernet 3",
            "6c:3b:6b:09:9c:9a",
            data("MikroTik"),
            "2025-07-18T17:25:38.656100",
        );
        update_neighbors(
            &mut neighbors,
            "Ethernet 3",
            "6c:3b:6b:09:9c:9a",
            data("Renamed"),
            "2025-07-18T17:52:24.226834",
        );
        let key = "Ethernet 3:6c:3b:6b:09:9c:9a:6c:3b:6b:09:9c:90";
        assert_eq!(neighbors.len(), 1);
        let n = &neighbors[key];
        assert_eq!(n["first_seen"], "2025-07-18T17:25:38.656100");
        assert_eq!(n["last_seen"], "2025-07-18T17:52:24.226834");
        assert_eq!(n["data"]["system_name"], "Renamed");
        assert_eq!(n["ttl"], 120);
        let keys: Vec<&String> = n.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["interface", "source_mac", "data", "first_seen", "last_seen", "ttl"]);
    }

    #[test]
    fn backend_selection() {
        let both = vec![Backend::PacketMonitor, Backend::Npcap];
        assert_eq!(backend_order(None), Ok(both.clone()));
        assert_eq!(backend_order(Some(" Auto ")), Ok(both));
        assert_eq!(backend_order(Some("pktmon")), Ok(vec![Backend::PacketMonitor]));
        assert_eq!(backend_order(Some("NPCAP")), Ok(vec![Backend::Npcap]));
        assert!(backend_order(Some("wireshark")).unwrap_err().contains("LLDP_CAPTURE_BACKEND"));
    }

    #[test]
    fn duplicate_reports_are_dropped_within_a_second() {
        let mut d = Deduplicator::default();
        let t0 = Instant::now();
        assert!(d.is_new(t0, "Ethernet", b"frame"));
        assert!(!d.is_new(t0 + Duration::from_millis(10), "Ethernet", b"frame"));
        assert!(d.is_new(t0 + Duration::from_millis(10), "WiFi", b"frame"));
        assert!(d.is_new(t0 + Duration::from_millis(20), "Ethernet", b"other"));
        assert!(d.is_new(t0 + Duration::from_millis(1500), "Ethernet", b"frame"));
    }

    #[test]
    fn running_backend_is_read_from_the_log() {
        let dir = std::env::temp_dir().join(format!("winlldp-backend-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("log.txt");
        assert_eq!(running_backend(&file), None);
        std::fs::write(&file, "[t] Capture backend: Npcap\n[t] x\n[t] Capture backend: Packet Monitor\n[t] y\n")
            .unwrap();
        assert_eq!(running_backend(&file).as_deref(), Some("Packet Monitor"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn non_lldp_frames_are_ignored() {
        let dir = std::env::temp_dir().join(format!("winlldp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("neighbors.json");
        let log = CaptureLog(dir.join("log.txt"));
        process_frame(&[0u8; 60], "x", &file, &log);
        assert!(!file.exists());

        let mut p = LldpPacket::new();
        p.add_chassis_id(4, &[1, 2, 3, 4, 5, 6]);
        p.add_ttl(90);
        p.add_end_of_lldpdu();
        let frame = pcap::ethernet_frame(
            &pcap::LLDP_MULTICAST_MAC,
            &[0xa, 0xb, 0xc, 0xd, 0xe, 0xf],
            0x88cc,
            &p.encode().unwrap(),
        );
        process_frame(&frame, "Ethernet", &file, &log);
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let n = &saved["Ethernet:0a:0b:0c:0d:0e:0f:01:02:03:04:05:06"];
        assert_eq!(n["ttl"], 90);
        assert_eq!(n["data"]["chassis_id_subtype"], "MAC_ADDRESS");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
