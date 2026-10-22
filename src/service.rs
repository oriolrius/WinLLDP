//! Windows service management through NSSM, and the service process itself

use crate::{capture, config::Config, paths, sender};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const SERVICE_NAME: &str = "WinLLDP";

pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Console output as text (NSSM may write UTF-16)
fn decode(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count() * 2 >= bytes.len() / 2 {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Run a program; Err when it cannot be found (v1's FileNotFoundError)
pub fn run(program: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> Result<Output, std::io::Error> {
    let out = Command::new(program).args(args).output()?;
    Ok(Output { code: out.status.code().unwrap_or(-1), stdout: decode(&out.stdout), stderr: decode(&out.stderr) })
}

/// NSSM (public domain) runs WinLLDP as a service; it is bundled, see vendor/nssm/
const NSSM_VERSION: &str = "2.24-101-g897c7ad";
const NSSM_EXE: &[u8] = include_bytes!("../vendor/nssm/2.24-101-g897c7ad/nssm.exe");

fn nssm_dir() -> PathBuf {
    paths::program_directory().join(format!("nssm-{NSSM_VERSION}"))
}

/// The bundled nssm.exe, extracted to %ProgramFiles%\WinLLDP (the service's image path points there)
pub fn nssm_path() -> Result<PathBuf, std::io::Error> {
    let path = nssm_dir().join("nssm.exe");
    if std::fs::read(&path).ok().as_deref() != Some(NSSM_EXE) {
        std::fs::create_dir_all(nssm_dir())?;
        std::fs::write(&path, NSSM_EXE)?;
    }
    Ok(path)
}

fn nssm(args: &[&str]) -> Result<Output, std::io::Error> {
    // Only `install` (and `version`, its check) puts NSSM on disk: asking about a service that is
    // not installed must not leave nssm.exe behind
    let installing = matches!(args.first(), Some(&"install") | Some(&"version"));
    if !installing && !nssm_dir().join("nssm.exe").exists() && sc_query().code != 0 {
        return Ok(Output { code: 3, stdout: String::new(), stderr: "Can't open service!".into() });
    }
    run(nssm_path()?, args)
}

/// Remove the extracted NSSM once the service no longer uses it
fn remove_bundled_nssm() {
    let _ = std::fs::remove_dir_all(nssm_dir());
    let _ = std::fs::remove_dir(paths::program_directory());
}

fn sc_query() -> Output {
    run("sc", &["query", SERVICE_NAME]).unwrap_or(Output { code: -1, stdout: String::new(), stderr: String::new() })
}

fn state_lines(output: &str) -> impl Iterator<Item = &str> {
    output.lines().filter(|l| l.contains("STATE")).map(str::trim)
}

fn print_nssm_missing(error: &std::io::Error) {
    eprintln!("Error: cannot run the bundled NSSM ({error}): run winlldp as administrator");
}

fn flush() {
    let _ = std::io::stdout().flush();
}

fn wait_until(attempts: usize, delay: Duration, done: impl Fn() -> bool) -> bool {
    for _ in 0..attempts {
        if done() {
            println!(" OK");
            return true;
        }
        print!(".");
        flush();
        std::thread::sleep(delay);
    }
    println!(" TIMEOUT");
    false
}

pub fn install(verbose: bool) -> i32 {
    if verbose {
        println!("Checking for NSSM installation...");
    }
    match nssm(&["version"]) {
        Ok(out) if out.code == 0 => {
            if verbose {
                println!("NSSM version: {}", out.stdout.trim());
            }
        }
        Ok(out) => {
            eprintln!("Error: the bundled NSSM does not run: {}", out.stderr.trim());
            return 1;
        }
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
    }

    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let project_dir = paths::runtime_directory().display().to_string();
    if verbose {
        println!("Installation details:");
        println!("  Executable: {exe}");
        println!("  Working directory: {project_dir}");
        println!("  Service command: {exe} service-run");
    }

    println!("Installing WinLLDP service...");
    if verbose {
        println!("Checking if service already exists...");
    }
    if sc_query().code == 0 {
        println!("Service already exists, updating configuration...");
        if verbose {
            println!("Stopping existing service...");
        }
        if let Ok(out) = nssm(&["stop", SERVICE_NAME]) {
            if verbose {
                println!("  Stop command result: {}", out.code);
                if !out.stdout.trim().is_empty() {
                    println!("  Output: {}", out.stdout.trim());
                }
            }
        }
        print!("Waiting for service to stop...");
        flush();
        wait_until(30, Duration::from_millis(500), || {
            let q = sc_query();
            q.stdout.contains("STOPPED") || q.code != 0
        });

        if verbose {
            println!("Removing existing service...");
        }
        if let Ok(out) = nssm(&["remove", SERVICE_NAME, "confirm"]) {
            if verbose {
                println!("  Remove command result: {}", out.code);
                if !out.stderr.trim().is_empty() {
                    println!("  Error: {}", out.stderr.trim());
                }
            }
        }
        print!("Waiting for service removal...");
        flush();
        wait_until(20, Duration::from_millis(500), || sc_query().code != 0);
    }

    let install_cmd = ["install", SERVICE_NAME, exe.as_str(), "service-run"];
    if verbose {
        println!("Installing service with NSSM...");
        println!("  Command: nssm {}", install_cmd.join(" "));
    }
    match nssm(&install_cmd) {
        Ok(out) if out.code == 0 => {
            if verbose {
                println!("  Installation result: {}", out.code);
                if !out.stdout.trim().is_empty() {
                    println!("  Output: {}", out.stdout.trim());
                }
            }
        }
        Ok(out) => {
            eprintln!("Error installing service: {}", out.stderr);
            return 1;
        }
        Err(e) => {
            eprintln!("Error installing service: {e}");
            return 1;
        }
    }

    print!("Waiting for service registration...");
    flush();
    if !wait_until(30, Duration::from_secs(1), || sc_query().code == 0) {
        eprintln!("Error: Service installation timed out");
        return 1;
    }

    if verbose {
        println!("Configuring service parameters...");
    }
    let rotate_bytes = (10 * 1024 * 1024).to_string();
    let settings: [(&str, &str); 10] = [
        ("AppDirectory", &project_dir),
        ("DisplayName", "Windows LLDP Service"),
        ("Description", "Link Layer Discovery Protocol service for Windows"),
        ("Start", "SERVICE_AUTO_START"),
        ("AppNoConsole", "1"),
        ("AppRestartDelay", "5000"),
        ("AppThrottle", "1500"),
        ("AppRotateOnline", "1"),
        ("AppRotateBytes", &rotate_bytes),
        ("AppStopMethodSkip", "0"),
    ];
    for (name, value) in settings {
        if verbose {
            println!("  Setting {name}: {value}");
        }
        if let Ok(out) = nssm(&["set", SERVICE_NAME, name, value]) {
            // Setting the description fails on some Windows versions: not an error
            if out.code != 0 && name != "Description" {
                println!("Warning: {name} {value}: {}", out.stderr.trim());
            }
        }
    }

    println!("Service installed successfully!");
    println!("  Executable: {exe}");
    println!("  Command: service-run");
    println!("  Working directory: {project_dir}");
    println!();
    println!("To start the service, run: winlldp service start");
    0
}

/// click.confirm: "[y/N]" prompt, None when stdin is closed
fn confirm(question: &str) -> Option<bool> {
    print!("{question} [y/N]: ");
    flush();
    let mut answer = String::new();
    match std::io::stdin().lock().read_line(&mut answer) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes" | "1" | "true")),
    }
}

pub fn uninstall(verbose: bool) -> i32 {
    if verbose {
        println!("Checking if service exists...");
    }
    match nssm(&["status", SERVICE_NAME]) {
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
        Ok(out) if out.code != 0 => {
            println!("Service WinLLDP not found");
            return 0;
        }
        Ok(out) => {
            if verbose {
                println!("Current service status: {}", out.stdout.trim());
                println!("\nService configuration before removal:");
                for item in ["Application", "AppDirectory", "AppParameters"] {
                    if let Ok(r) = nssm(&["get", SERVICE_NAME, item]) {
                        if r.code == 0 {
                            println!("  {item}: {}", r.stdout.trim());
                        }
                    }
                }
            }
        }
    }

    match confirm("Are you sure you want to uninstall the WinLLDP service?") {
        None => {
            eprintln!("Aborted!");
            return 1;
        }
        Some(false) => {
            println!("Uninstallation cancelled");
            return 0;
        }
        Some(true) => {}
    }

    if verbose {
        println!("\nStopping service before removal...");
    }
    if let Ok(out) = nssm(&["stop", SERVICE_NAME]) {
        if verbose {
            println!("  Stop result: {}", out.code);
            if !out.stderr.trim().is_empty() {
                println!("  Output: {}", out.stderr.trim());
            }
        }
    }
    if verbose {
        println!("Waiting for service to stop...");
    }
    std::thread::sleep(Duration::from_secs(2));

    println!("Removing service...");
    if verbose {
        println!("Executing: nssm remove WinLLDP confirm");
    }
    let out = match nssm(&["remove", SERVICE_NAME, "confirm"]) {
        Ok(out) => out,
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
    };
    if verbose {
        println!("Remove command result: {}", out.code);
        if !out.stdout.trim().is_empty() {
            println!("Output: {}", out.stdout.trim());
        }
        if !out.stderr.trim().is_empty() {
            println!("Error output: {}", out.stderr.trim());
        }
    }
    if out.code != 0 {
        eprintln!("Error uninstalling service: {}", out.stderr);
        return 1;
    }
    println!("Service uninstalled successfully");
    remove_bundled_nssm();
    // The driver WinLLDP installed to send LLDP goes with the service
    if crate::xdp::installed_by_winlldp().is_some() {
        println!("Removing the XDP driver installed by WinLLDP...");
        match crate::xdp::uninstall() {
            Ok(()) => println!("XDP driver removed"),
            Err(e) => eprintln!("Warning: {e}"),
        }
    }
    if verbose {
        println!("\nVerifying service removal...");
        if sc_query().code != 0 {
            println!("  Service successfully removed from Windows");
        } else {
            println!("  WARNING: Service may still be registered in Windows");
        }
        let log_file = paths::service_log_file();
        if log_file.exists() {
            println!("\n  Note: Service log file still exists: {}", log_file.display());
            println!("  You may want to delete it manually if no longer needed");
        }
    }
    0
}

fn print_log_tail(log_path: &PathBuf, count: usize, indent: &str) {
    if let Ok(text) = std::fs::read_to_string(log_path) {
        let lines: Vec<&str> = text.lines().collect();
        for line in &lines[lines.len().saturating_sub(count)..] {
            println!("{indent}{}", line.trim_end());
        }
    }
}

pub fn start(verbose: bool) -> i32 {
    if verbose {
        println!("Checking if service exists...");
    }
    let check = sc_query();
    if check.code != 0 {
        eprintln!("Error: Service WinLLDP not found. Please install it first.");
        println!("Run: winlldp service install");
        return 1;
    }
    if verbose {
        println!("Current service status:");
        for line in state_lines(&check.stdout) {
            println!("  {line}");
        }
        println!("Checking NSSM configuration...");
        for name in ["Application", "AppDirectory", "AppParameters", "Start"] {
            match nssm(&["get", SERVICE_NAME, name]) {
                Ok(r) if r.code == 0 => println!("  {name}: {}", r.stdout.trim()),
                _ => println!("  {name}: Unable to retrieve"),
            }
        }
    }

    println!("Starting WinLLDP service...");
    if verbose {
        println!("Executing: nssm start WinLLDP");
    }
    let out = match nssm(&["start", SERVICE_NAME]) {
        Ok(out) => out,
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
    };
    if verbose {
        println!("Start command result: {}", out.code);
        if !out.stdout.trim().is_empty() {
            println!("Output: {}", out.stdout.trim());
        }
        if !out.stderr.trim().is_empty() {
            println!("Error output: {}", out.stderr.trim());
        }
    }
    if out.code == 0 {
        println!("Service started successfully");
        if verbose {
            println!("Verifying service status...");
            std::thread::sleep(Duration::from_secs(2));
            for line in state_lines(&sc_query().stdout) {
                println!("  {line}");
            }
        }
        return 0;
    }
    if out.stderr.contains("already running") {
        println!("Service is already running");
        return 0;
    }
    eprintln!("Error starting service: {}", out.stderr);
    if verbose {
        let log_path = paths::service_log_file();
        if log_path.exists() {
            println!("\nService log file: {}", log_path.display());
            println!("  Last 10 lines:");
            print_log_tail(&log_path, 10, "    ");
        }
    }
    1
}

pub fn stop(verbose: bool) -> i32 {
    if verbose {
        println!("Checking current service status...");
        let q = sc_query();
        if q.code == 0 {
            for line in state_lines(&q.stdout) {
                println!("  Current status: {line}");
            }
        }
    }
    println!("Stopping WinLLDP service...");
    if verbose {
        println!("Executing: nssm stop WinLLDP");
    }
    let out = match nssm(&["stop", SERVICE_NAME]) {
        Ok(out) => out,
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
    };
    if verbose {
        println!("Stop command result: {}", out.code);
        if !out.stdout.trim().is_empty() {
            println!("Output: {}", out.stdout.trim());
        }
        if !out.stderr.trim().is_empty() {
            println!("Error output: {}", out.stderr.trim());
        }
    }
    if out.code == 0 {
        println!("Service stopped successfully");
        if verbose {
            println!("Verifying service is stopped...");
            std::thread::sleep(Duration::from_secs(2));
            let q = sc_query();
            if q.code == 0 {
                for line in state_lines(&q.stdout) {
                    println!("  Final status: {line}");
                }
            }
        }
        return 0;
    }
    if out.stderr.contains("not running") {
        println!("Service is not running");
        return 0;
    }
    eprintln!("Error stopping service: {}", out.stderr);
    1
}

pub fn status(verbose: bool) -> i32 {
    if verbose {
        println!("Checking service status...");
    }
    let out = match nssm(&["status", SERVICE_NAME]) {
        Ok(out) => out,
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
    };
    if out.code != 0 {
        println!("Service WinLLDP not found");
        println!("Run 'winlldp service install' to install the service");
        return 0;
    }
    let status = out.stdout.trim().to_string();
    println!("WinLLDP service status: {status}");
    if verbose {
        println!("\nDetailed service information:");
        let q = sc_query();
        if q.code == 0 {
            println!("  Windows Service Control:");
            for line in q.stdout.lines().map(str::trim) {
                if !line.is_empty()
                    && ["SERVICE_NAME", "STATE", "TYPE", "WIN32_EXIT_CODE"].iter().any(|k| line.contains(k))
                {
                    println!("    {line}");
                }
            }
        }
        println!("\n  NSSM Configuration:");
        for (param, label) in [
            ("Application", "Application"),
            ("AppDirectory", "Working Directory"),
            ("AppParameters", "Parameters"),
            ("DisplayName", "Display Name"),
            ("Start", "Startup Type"),
            ("ObjectName", "Run As User"),
        ] {
            if let Ok(r) = nssm(&["get", SERVICE_NAME, param]) {
                if r.code == 0 {
                    println!("    {label}: {}", r.stdout.trim());
                }
            }
        }
    }
    match status.as_str() {
        "SERVICE_RUNNING" => {
            println!("  The service is running");
            let log_file = paths::service_log_file();
            if let Ok(text) = std::fs::read_to_string(&log_file) {
                let lines: Vec<&str> = text.lines().collect();
                if let Some(line) = lines.iter().rev().find(|l| l.contains("Service status:")) {
                    println!("  Last status: {}", line.trim());
                }
                if verbose && !lines.is_empty() {
                    println!("\n  Service log file: {}", log_file.display());
                    println!("  Log file size: {} bytes", text.len());
                    println!("  Last 5 log entries:");
                    print_log_tail(&log_file, 5, "    ");
                }
            } else if verbose {
                println!("\n  Service log file not found: {}", log_file.display());
            }
        }
        "SERVICE_STOPPED" => println!("  The service is stopped"),
        "SERVICE_PAUSED" => println!("  The service is paused"),
        other => println!("  Unknown status: {other}"),
    }
    0
}

pub fn restart(verbose: bool) -> i32 {
    if verbose {
        println!("Checking current service status...");
        let q = sc_query();
        if q.code == 0 {
            for line in state_lines(&q.stdout) {
                println!("  Current status: {line}");
            }
        }
    }
    println!("Restarting WinLLDP service...");
    if verbose {
        println!("Executing: nssm restart WinLLDP");
    }
    let out = match nssm(&["restart", SERVICE_NAME]) {
        Ok(out) => out,
        Err(e) => {
            print_nssm_missing(&e);
            return 1;
        }
    };
    if verbose {
        println!("Restart command result: {}", out.code);
        if !out.stdout.trim().is_empty() {
            println!("Output: {}", out.stdout.trim());
        }
        if !out.stderr.trim().is_empty() {
            println!("Error output: {}", out.stderr.trim());
        }
    }
    if out.code != 0 {
        eprintln!("Error restarting service: {}", out.stderr);
        return 1;
    }
    println!("Service restarted successfully");
    if verbose {
        println!("\nVerifying service is running...");
        std::thread::sleep(Duration::from_secs(3));
        let q = sc_query();
        if q.code == 0 {
            for line in state_lines(&q.stdout) {
                println!("  Final status: {line}");
            }
        }
    }
    0
}

/// The process NSSM runs (`service-run`): starts the capture process and sends
/// LLDP every interval until NSSM stops it with Ctrl+C
pub fn run_service() -> i32 {
    let log_file = paths::service_log_file();
    let log = |msg: &str| {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file) {
            let _ = writeln!(f, "[{}] {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    };

    log(&"=".repeat(60));
    log("Starting NSSM service wrapper");
    log(&format!("Executable: {}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()));
    let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
    log(&format!("Working directory: {cwd}"));

    // Ensure we're in the runtime directory (the executable's directory)
    let runtime_dir = paths::runtime_directory();
    if std::env::current_dir().ok().as_ref() != Some(&runtime_dir) {
        if let Err(e) = std::env::set_current_dir(&runtime_dir) {
            log(&format!("FATAL ERROR: cannot change directory to {}: {e}", runtime_dir.display()));
            return 1;
        }
        log(&format!("Changed directory to: {}", runtime_dir.display()));
    } else {
        log(&format!("Already in runtime directory: {}", runtime_dir.display()));
    }

    log("Creating configuration...");
    let config = match Config::load(None) {
        Ok(config) => config,
        Err(e) => {
            log(&format!("FATAL ERROR: {e}"));
            return 1;
        }
    };

    log("Creating components...");
    if capture::is_capture_running(&config) {
        log("Capture subprocess already running");
    } else {
        log("Starting capture subprocess...");
        match capture::start_capture(&config) {
            Ok(_) => log("Capture started successfully"),
            Err(e) => log(&format!("WARNING: Failed to start capture subprocess: {e}")),
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed));
    }

    log("Starting LLDP sender...");
    let sender_thread = {
        let (config, stop) = (config.clone(), stop.clone());
        std::thread::spawn(move || sender::run_loop(&config, &stop))
    };
    log("Service started successfully");

    'running: loop {
        for _ in 0..600 {
            if stop.load(Ordering::Relaxed) {
                break 'running;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        log("Service is running...");
    }
    log("Stopping sender...");
    let _ = sender_thread.join();
    log("Service stopped");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16_and_ansi_output() {
        let utf16: Vec<u8> = "SERVICE_RUNNING\r\n".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode(&utf16), "SERVICE_RUNNING\r\n");
        assert_eq!(decode(b"SERVICE_STOPPED\r\n"), "SERVICE_STOPPED\r\n");
    }
}
