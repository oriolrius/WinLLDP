//! Integration tests: run the real winlldp.exe from a temporary directory
//! (ported from v1's tests/test_frozen_executable.py, plus the commands it did not cover)

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The executable cargo built next to this test binary (target/<profile>/winlldp.exe)
fn built_exe() -> PathBuf {
    let deps = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let exe = deps.parent().unwrap().join(format!("winlldp{}", std::env::consts::EXE_SUFFIX));
    assert!(exe.exists(), "{} not found: run `cargo build` first", exe.display());
    exe
}

fn npcap_installed() -> bool {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let system32 = Path::new(&root).join("System32");
    system32.join("Npcap").join("wpcap.dll").exists() || system32.join("wpcap.dll").exists()
}

fn is_admin() -> bool {
    Command::new("net").arg("session").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// A copy of the executable in its own temporary directory
struct Sandbox {
    dir: PathBuf,
    exe: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!("winlldp-it-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("winlldp.exe");
        std::fs::copy(built_exe(), &exe).unwrap();
        Sandbox { dir, exe }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(&self.exe);
        c.args(args).current_dir(&self.dir);
        c
    }

    fn run(&self, args: &[&str]) -> Result {
        self.run_with(args, &[])
    }

    fn run_with(&self, args: &[&str], env: &[(&str, &str)]) -> Result {
        let mut c = self.command(args);
        c.stdin(Stdio::null());
        for (k, v) in env {
            c.env(k, v);
        }
        Result::from(c.output().unwrap())
    }

    fn run_input(&self, args: &[&str], input: &str) -> Result {
        use std::io::Write;
        let mut child =
            self.command(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        Result::from(child.wait_with_output().unwrap())
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = self.command(&["capture", "stop"]).output();
        std::thread::sleep(Duration::from_millis(300));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Debug)]
struct Result {
    code: i32,
    stdout: String,
    stderr: String,
}

impl From<Output> for Result {
    fn from(o: Output) -> Self {
        Result {
            code: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }
}

fn wait_for(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    done()
}

#[test]
fn help_lists_every_command() {
    let s = Sandbox::new("help");
    let r = s.run(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("Usage:") && r.stdout.contains("winlldp"), "{r:?}");
    for command in [
        "version",
        "capture",
        "show-neighbors",
        "send",
        "show-interfaces",
        "show-config",
        "debug-paths",
        "clear-neighbors",
        "service",
    ] {
        assert!(r.stdout.contains(command), "{command} missing from help");
    }
    assert!(!r.stdout.contains("capture-subprocess") && !r.stdout.contains("service-run"));
    let r = s.run(&["capture", "--help"]);
    for command in ["start", "stop", "status", "log"] {
        assert!(r.stdout.contains(command));
    }
    let r = s.run(&["service", "--help"]);
    for command in ["install", "uninstall", "start", "stop", "status", "restart"] {
        assert!(r.stdout.contains(command));
    }
}

#[test]
fn version_command() {
    let r = Sandbox::new("version").run(&["version"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout.trim(), format!("WinLLDP version {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_command_fails() {
    let r = Sandbox::new("unknown").run(&["no-such-command"]);
    assert_ne!(r.code, 0);
}

#[test]
fn show_config_defaults_and_files_in_exe_directory() {
    let s = Sandbox::new("config");
    let r = s.run(&["show-config"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.starts_with("Current Configuration:\nConfig(\n  interval=30s,\n  interface=all,\n"), "{r:?}");
    assert!(r.stdout.contains(&format!("neighbors_file={}", s.path("neighbors.json").display())));
}

#[test]
fn env_file_option_and_dotenv_in_working_directory() {
    let s = Sandbox::new("envfile");
    std::fs::write(s.path("custom.env"), "LLDP_INTERVAL=45\nLLDP_TTL=200\nLLDP_SYSTEM_NAME=from-file\n").unwrap();
    let r = s.run(&["show-config", "-e", "custom.env"]);
    assert!(
        r.stdout.contains("interval=45s")
            && r.stdout.contains("ttl=200s")
            && r.stdout.contains("system_name=from-file"),
        "{r:?}"
    );

    // v1's env.example format: unquoted values with spaces, comments
    std::fs::write(
        s.path(".env"),
        "# LLDP\nLLDP_INTERFACE=Ethernet 9\nLLDP_SYSTEM_DESCRIPTION=Windows LLDP Service # note\n",
    )
    .unwrap();
    let r = s.run(&["show-config"]);
    assert!(
        r.stdout.contains("interface=Ethernet 9") && r.stdout.contains("system_description=Windows LLDP Service,"),
        "{r:?}"
    );

    // The .env file is also found in a parent of the working directory
    std::fs::create_dir_all(s.path("sub")).unwrap();
    let r = Result::from(s.command(&["show-config"]).current_dir(s.path("sub")).output().unwrap());
    assert!(r.stdout.contains("interface=Ethernet 9"), "{r:?}");

    // Environment variables win over the .env file
    let r = s.run_with(&["show-config"], &[("LLDP_INTERFACE", "Wi-Fi")]);
    assert!(r.stdout.contains("interface=Wi-Fi"), "{r:?}");

    // A custom neighbors file must be an absolute path
    let custom = s.path("data.json");
    let r = s.run_with(&["show-config"], &[("LLDP_NEIGHBORS_FILE", custom.to_str().unwrap())]);
    assert!(r.stdout.contains(&format!("neighbors_file={}", custom.display())), "{r:?}");
}

#[test]
fn invalid_configuration_is_reported() {
    let s = Sandbox::new("badconfig");
    let r = s.run_with(&["show-config"], &[("LLDP_INTERVAL", "2")]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("Error loading configuration: LLDP_INTERVAL must be at least 5 seconds"), "{r:?}");
    let r = s.run_with(&["show-config"], &[("LLDP_TTL", "10")]);
    assert!(r.stderr.contains("LLDP_TTL must be greater than LLDP_INTERVAL"), "{r:?}");
    let r = s.run_with(&["send"], &[("LLDP_TTL", "70000")]);
    assert!(r.stderr.contains("LLDP_TTL must be less than 65536"), "{r:?}");
    let r = s.run_with(&["show-config"], &[("LLDP_TTL", "abc")]);
    assert!(r.stderr.contains("'abc'"), "{r:?}");
}

#[test]
fn debug_paths_point_to_exe_directory() {
    let s = Sandbox::new("debugpaths");
    let r = s.run(&["debug-paths"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("Config file: .env (default)"));
    assert!(r.stdout.contains(&format!("Neighbors file: {}", s.path("neighbors.json").display())));
    assert!(r.stdout.contains(&format!("PID file: {}", s.path("capture.pid").display())));
    assert!(r.stdout.contains(&format!("Log file: {}", s.path("winlldp_capture.log").display())));
    std::fs::write(s.path("neighbors.json"), "{}").unwrap();
    let r = s.run(&["debug-paths"]);
    assert!(r.stdout.contains("Exists: True") && r.stdout.contains("Size: 2 bytes"), "{r:?}");
}

#[test]
fn show_interfaces() {
    let r = Sandbox::new("interfaces").run(&["show-interfaces"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("Network Interfaces:"));
    assert!(
        r.stdout.contains("| Interface") && r.stdout.contains("| MAC Address") && r.stdout.contains("IPv4 Addresses")
    );
}

#[test]
fn show_neighbors_reads_exe_directory() {
    let s = Sandbox::new("neighbors");
    let r = s.run(&["show-neighbors"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("No neighbors file found. Make sure capture is running."));

    std::fs::write(
        s.path("neighbors.json"),
        r#"{"test_neighbor": {"interface": "Test Interface", "source_mac": "00:11:22:33:44:55",
            "data": {"system_name": "Test System", "system_description": "Test Description"},
            "first_seen": "2024-01-01T00:00:00", "last_seen": "2024-01-01T00:00:00", "ttl": 120},
          "full": {"interface": "Ethernet 3", "source_mac": "6c:3b:6b:09:9c:9a",
            "data": {"chassis_id": "6c:3b:6b:09:9c:90", "port_id": "ether10", "port_description": "BR-MGMNT",
                     "ttl": 90, "system_name": "MikroTik", "management_address": "10.2.99.16"},
            "first_seen": "2025-07-18T17:25:38.656100", "last_seen": "2025-07-18T17:52:24.226834", "ttl": 90}}"#,
    )
    .unwrap();
    let r = s.run(&["show-neighbors"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with("LLDP Neighbors:\n+"), "{r:?}");
    assert!(r.stdout.contains("| Interface") && r.stdout.contains("| Management IP"));
    assert!(r.stdout.contains("Test System") && r.stdout.contains("Test Interface"));
    // Missing fields fall back to v1's defaults
    let row = r.stdout.lines().find(|l| l.contains("Test System")).unwrap();
    assert!(row.contains("Unknown") && row.contains("120s") && row.contains("N/A"), "{row}");
    let row = r.stdout.lines().find(|l| l.contains("MikroTik")).unwrap();
    assert!(
        row.contains("BR-MGMNT")
            && row.contains("90s")
            && row.contains("10.2.99.16")
            && row.contains("6c:3b:6b:09:9c:90")
    );
    assert!(row.contains("d "), "age in days: {row}");
    assert!(r.stdout.contains("Total neighbors: 2"));

    std::fs::write(s.path("neighbors.json"), "{}").unwrap();
    assert!(s.run(&["show-neighbors"]).stdout.contains("No LLDP neighbors in file."));
    std::fs::write(s.path("neighbors.json"), "{not json").unwrap();
    assert!(s.run(&["show-neighbors"]).stdout.contains("Error reading neighbors file. It may be corrupted."));
}

#[test]
fn show_neighbors_watch_refreshes() {
    let s = Sandbox::new("watch");
    std::fs::write(s.path("neighbors.json"), "{}").unwrap();
    let mut child =
        s.command(&["show-neighbors", "--watch"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    std::thread::sleep(Duration::from_secs(7));
    let _ = child.kill();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("Watching LLDP neighbors... (Press Ctrl+C to exit)"), "{stdout}");
    assert_eq!(stdout.matches("No LLDP neighbors in file.").count(), 2, "refreshes every 5 seconds: {stdout}");
}

#[test]
fn clear_neighbors() {
    let s = Sandbox::new("clear");
    let r = s.run(&["clear-neighbors"]);
    assert!(r.stdout.contains("No neighbors file found at:"));
    std::fs::write(s.path("neighbors.json"), r#"{"a": {}}"#).unwrap();
    let r = s.run(&["clear-neighbors"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("Cleared all discovered neighbors from:"));
    assert_eq!(std::fs::read_to_string(s.path("neighbors.json")).unwrap(), "{}");
}

#[test]
fn capture_log_command() {
    let s = Sandbox::new("log");
    assert!(s.run(&["capture", "log"]).stdout.contains("No log file found. Capture may not have been started yet."));
    let lines: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
    std::fs::write(s.path("winlldp_capture.log"), lines.join("\n") + "\n").unwrap();
    let r = s.run(&["capture", "log"]);
    assert!(r.stdout.starts_with("line 11\n") && r.stdout.contains("[Showing last 20 lines of 30 total]"), "{r:?}");
    let r = s.run(&["capture", "log", "-n", "3"]);
    assert!(r.stdout.starts_with("line 28\nline 29\nline 30\n\n[Showing last 3 lines of 30 total]"), "{r:?}");
    let r = s.run(&["capture", "log", "--lines", "0"]);
    assert!(r.stdout.contains("[Showing last 30 lines of 30 total]"));
}

#[test]
fn capture_status_and_stop_when_not_running() {
    let s = Sandbox::new("status");
    let r = s.run(&["capture", "status"]);
    assert_eq!(r.stdout.trim(), "LLDP Capture Status: NOT RUNNING");
    let r = s.run(&["capture", "stop"]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("Failed to stop LLDP capture"));
}

#[test]
fn stale_pid_file_never_kills_another_program() {
    let s = Sandbox::new("stalepid");
    let mut other = Command::new("ping").args(["-n", "30", "127.0.0.1"]).stdout(Stdio::null()).spawn().unwrap();
    std::fs::write(s.path("capture.pid"), other.id().to_string()).unwrap();
    assert!(s.run(&["capture", "status"]).stdout.contains("NOT RUNNING"));
    let r = s.run(&["capture", "stop"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(!s.path("capture.pid").exists());
    assert!(other.try_wait().unwrap().is_none(), "the unrelated process was killed");
    let _ = other.kill();
}

#[test]
fn capture_start_fails_without_npcap() {
    // Point Windows' directory somewhere without Npcap to simulate a machine without it
    let s = Sandbox::new("nonpcap");
    let fake_root = s.path("FakeWindows");
    std::fs::create_dir_all(fake_root.join("System32")).unwrap();
    let r = s.run_with(&["capture", "start"], &[("SystemRoot", fake_root.to_str().unwrap())]);
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("Npcap is not installed"), "{r:?}");
    assert!(!r.stdout.contains("started successfully"));
    assert!(!s.path("capture.pid").exists());
    if !npcap_installed() {
        let r = s.run(&["capture", "start"]);
        assert!(r.code != 0 && r.stderr.contains("Npcap is not installed"), "{r:?}");
    }
}

#[test]
fn send_command() {
    let s = Sandbox::new("send");
    let r = s.run(&["send", "-v"]);
    assert!(r.stdout.contains("Configuration:\n  Interface: all\n  System Name: auto\n  TTL: 120"), "{r:?}");
    assert!(r.stdout.contains("Sending LLDP packets"), "{r:?}");
    if npcap_installed() {
        assert_eq!(r.code, 0, "{r:?}");
        assert!(r.stdout.contains("LLDP packets sent successfully!"));
        if r.stdout.contains("Sending LLDP on ") {
            assert!(r.stdout.contains("  Destination: 01:80:c2:00:00:0e"));
            assert!(r.stdout.contains("  Raw LLDP data: 0207"));
            assert!(r.stdout.contains("    System Description: Windows 10.0."));
            assert!(r.stdout.contains("  Sent successfully!"));
        }
    } else if r.stdout.contains("Sending LLDP on ") {
        assert_ne!(r.code, 0);
        assert!(r.stderr.contains("Npcap is not installed"), "{r:?}");
    }
    let r = s.run(&["send", "-i", "No Such Interface"]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("Interface No Such Interface not found"), "{r:?}");
}

#[test]
fn capture_files_live_in_exe_directory_not_temp() {
    if !npcap_installed() {
        eprintln!("skipped: Npcap is not installed");
        return;
    }
    let s = Sandbox::new("capture");
    let temp_before: Vec<_> =
        std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().map(|e| e.file_name()).collect();

    // `capture start` must return quickly even when the caller reads its output
    let started = Instant::now();
    let r = s.run(&["capture", "start"]);
    assert!(started.elapsed() < Duration::from_secs(20), "capture start blocked the caller");
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("LLDP capture started successfully"));
    assert!(s.path("capture.pid").exists() && s.path("winlldp_capture.log").exists());

    let r = s.run(&["capture", "start"]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("Capture is already running"), "{r:?}");

    let r = s.run(&["capture", "status"]);
    assert!(
        r.stdout.contains("LLDP Capture Status: RUNNING") && r.stdout.contains("PID:") && r.stdout.contains("Memory:"),
        "{r:?}"
    );

    let r = s.run(&["capture", "log", "-n", "0"]);
    assert!(r.stdout.contains("Starting packet capture on:"), "{r:?}");

    let temp_after: Vec<_> =
        std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().map(|e| e.file_name()).collect();
    let new: Vec<_> = temp_after
        .iter()
        .filter(|f| !temp_before.contains(f))
        .map(|f| f.to_string_lossy().to_string())
        .filter(|f| {
            (f.contains("neighbors") || f.contains("capture") || f.contains("winlldp")) && !f.starts_with("winlldp-it-")
        })
        .collect();
    assert!(new.is_empty(), "files created in the temp directory: {new:?}");

    let r = s.run(&["capture", "stop"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("LLDP capture stopped successfully"));
    assert!(!s.path("capture.pid").exists(), "PID file not cleaned up");
    assert!(s.run(&["capture", "status"]).stdout.contains("NOT RUNNING"));
}

#[test]
fn captured_lldp_becomes_a_neighbor() {
    if !npcap_installed() {
        eprintln!("skipped: Npcap is not installed");
        return;
    }
    let s = Sandbox::new("e2e");
    let r = s.run(&["capture", "start"]);
    assert_eq!(r.code, 0, "{r:?}");
    let sent = s.run(&["send", "-v"]);
    if !sent.stdout.contains("Sending LLDP on ") {
        eprintln!("skipped: no active interface to send on");
        return;
    }
    // Npcap hands our own outgoing frames to the capture too
    let found = wait_for(Duration::from_secs(10), || {
        std::fs::read_to_string(s.path("neighbors.json"))
            .is_ok_and(|t| t.contains("\"system_description\": \"Windows 10.0."))
    });
    assert!(found, "no neighbor recorded; log:\n{}", s.run(&["capture", "log", "-n", "40"]).stdout);
    let neighbors: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(s.path("neighbors.json")).unwrap()).unwrap();
    let (_, n) = neighbors.as_object().unwrap().iter().next().unwrap();
    for key in ["interface", "source_mac", "data", "first_seen", "last_seen", "ttl"] {
        assert!(n.get(key).is_some(), "{key} missing: {n}");
    }
    assert_eq!(n["data"]["chassis_id_subtype"], "MAC_ADDRESS");
    assert_eq!(n["data"]["ttl"], 120);
    assert_eq!(n["data"]["system_capabilities"], 128);
    let r = s.run(&["show-neighbors"]);
    assert!(r.stdout.contains("Ethernet Port") && r.stdout.contains("Total neighbors:"), "{r:?}");
}

fn sc_query() -> String {
    String::from_utf8_lossy(&Command::new("sc").args(["query", "WinLLDP"]).output().unwrap().stdout).into_owned()
}

#[test]
fn service_lifecycle() {
    let nssm = Command::new("nssm").arg("version").output().is_ok_and(|o| o.status.success());
    if !is_admin() || !nssm {
        eprintln!("skipped: needs administrator privileges and NSSM");
        return;
    }
    if Command::new("sc").args(["query", "WinLLDP"]).output().unwrap().status.success() {
        eprintln!("skipped: a WinLLDP service is already installed on this machine");
        return;
    }
    let s = Sandbox::new("service");
    let r = s.run(&["service", "status"]);
    assert!(r.stdout.contains("Service WinLLDP not found"), "{r:?}");

    let r = s.run(&["service", "install", "-v"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("Service installed successfully!") && r.stdout.contains("Command: service-run"), "{r:?}");

    let cleanup = || {
        let _ = Command::new("nssm").args(["stop", "WinLLDP"]).output();
        let _ = Command::new("nssm").args(["remove", "WinLLDP", "confirm"]).output();
    };
    let result = std::panic::catch_unwind(|| {
        let get = |p: &str| {
            String::from_utf8_lossy(&Command::new("nssm").args(["get", "WinLLDP", p]).output().unwrap().stdout)
                .trim()
                .to_string()
        };
        assert_eq!(get("AppParameters"), "service-run");
        assert_eq!(get("AppDirectory"), s.dir.display().to_string());
        assert_eq!(get("Start"), "SERVICE_AUTO_START");

        let r = s.run(&["service", "status"]);
        assert!(
            r.stdout.contains("WinLLDP service status: SERVICE_STOPPED") && r.stdout.contains("The service is stopped"),
            "{r:?}"
        );

        let r = s.run(&["service", "start", "-v"]);
        assert_eq!(r.code, 0, "{r:?}");
        assert!(r.stdout.contains("Service started successfully"));
        assert!(
            wait_for(Duration::from_secs(15), || s.path("capture.pid").exists()),
            "the service did not start the capture"
        );
        assert!(wait_for(Duration::from_secs(10), || std::fs::read_to_string(s.path("nssm_service.log"))
            .is_ok_and(|l| l.contains("Service started successfully"))));
        let r = s.run(&["service", "status", "-v"]);
        assert!(r.stdout.contains("SERVICE_RUNNING") && r.stdout.contains("The service is running"), "{r:?}");
        assert!(s.run(&["capture", "status"]).stdout.contains("RUNNING"));

        let r = s.run(&["service", "start"]);
        assert!(r.stdout.contains("already running") || r.code == 0, "{r:?}");

        let r = s.run(&["service", "restart"]);
        assert_eq!(r.code, 0, "{r:?}");
        assert!(r.stdout.contains("Service restarted successfully"));

        let r = s.run(&["service", "stop", "-v"]);
        assert_eq!(r.code, 0, "{r:?}");
        assert!(r.stdout.contains("Service stopped successfully"));
        assert!(sc_query().contains("STOPPED"));
        let log = std::fs::read_to_string(s.path("nssm_service.log")).unwrap();
        assert!(log.contains("Starting NSSM service wrapper") && log.contains("Starting LLDP sender..."), "{log}");
        // NSSM stops the service with Ctrl+C: the sender stops cleanly
        assert!(log.contains("Stopping sender...") && log.contains("Service stopped"), "{log}");

        // Installing again replaces the existing service
        let r = s.run(&["service", "install"]);
        assert!(
            r.stdout.contains("Service already exists, updating configuration...")
                && r.stdout.contains("Service installed successfully!"),
            "{r:?}"
        );

        let r = s.run_input(&["service", "uninstall"], "n\n");
        assert!(r.stdout.contains("Uninstallation cancelled"), "{r:?}");
        let r = s.run_input(&["service", "uninstall", "-v"], "y\n");
        assert_eq!(r.code, 0, "{r:?}");
        assert!(
            r.stdout.contains("Service uninstalled successfully")
                && r.stdout.contains("Service successfully removed from Windows"),
            "{r:?}"
        );
        let r = s.run(&["service", "status"]);
        assert!(r.stdout.contains("Service WinLLDP not found"), "{r:?}");
    });
    cleanup();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
