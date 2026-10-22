use clap::{Args, Parser, Subcommand};
use serde_json::{Map, Value};
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use winlldp::config::Config;
use winlldp::{capture, fsdebug, logger, paths, process, py_str, sender, service, sysinfo, table};

/// Windows LLDP Service CLI
#[derive(Parser)]
#[command(name = "winlldp")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct EnvFile {
    /// Path to .env configuration file
    #[arg(short = 'e', long = "env-file", value_name = "TEXT")]
    env_file: Option<String>,
}

#[derive(Args)]
struct Verbose {
    /// Enable verbose debug output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show WinLLDP version
    Version,
    /// LLDP capture management commands
    #[command(subcommand)]
    Capture(CaptureCommands),
    /// Show discovered LLDP neighbors
    ShowNeighbors {
        #[command(flatten)]
        env: EnvFile,
        /// Watch neighbors continuously
        #[arg(short, long)]
        watch: bool,
        #[command(flatten)]
        verbose: Verbose,
    },
    /// Send LLDP packets immediately
    Send {
        #[command(flatten)]
        env: EnvFile,
        /// Specific interface to send on
        #[arg(short, long, value_name = "TEXT")]
        interface: Option<String>,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Show network interfaces
    ShowInterfaces {
        #[command(flatten)]
        verbose: Verbose,
    },
    /// Show current configuration
    ShowConfig {
        #[command(flatten)]
        env: EnvFile,
        #[command(flatten)]
        verbose: Verbose,
    },
    /// Show debug information about file paths
    DebugPaths {
        #[command(flatten)]
        env: EnvFile,
    },
    /// Clear all discovered neighbors
    ClearNeighbors {
        #[command(flatten)]
        env: EnvFile,
    },
    /// Windows service management commands
    #[command(subcommand)]
    Service(ServiceCommands),
    /// Internal: the capture process started by `capture start`
    #[command(hide = true)]
    CaptureSubprocess { log_file: String, neighbors_file: String, pid_file: String },
    /// Internal: the process run by the Windows service
    #[command(hide = true)]
    ServiceRun,
}

#[derive(Subcommand)]
enum CaptureCommands {
    /// Start LLDP packet capture subprocess
    Start {
        #[command(flatten)]
        env: EnvFile,
        #[command(flatten)]
        verbose: Verbose,
    },
    /// Stop LLDP packet capture subprocess
    Stop {
        #[command(flatten)]
        env: EnvFile,
        #[command(flatten)]
        verbose: Verbose,
    },
    /// Show LLDP capture subprocess status
    Status {
        #[command(flatten)]
        env: EnvFile,
        #[command(flatten)]
        verbose: Verbose,
    },
    /// Show capture process log
    Log {
        /// Number of lines to show
        #[arg(short = 'n', long, default_value_t = 20, allow_negative_numbers = true)]
        lines: i64,
        #[command(flatten)]
        verbose: Verbose,
    },
}

#[derive(Subcommand)]
enum ServiceCommands {
    /// Install as Windows service
    Install(Verbose),
    /// Uninstall Windows service
    Uninstall(Verbose),
    /// Start Windows service
    Start(Verbose),
    /// Stop Windows service
    Stop(Verbose),
    /// Show Windows service status
    Status(Verbose),
    /// Restart Windows service
    Restart(Verbose),
}

fn setup_logging(verbose: bool) {
    logger::init(verbose, true);
}

fn load_config(env_file: &Option<String>) -> Result<Config, i32> {
    Config::load(env_file.as_deref()).map_err(|e| {
        eprintln!("Error loading configuration: {e}");
        1
    })
}

fn capture_start(env_file: &Option<String>, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    if verbose {
        println!("Verbose mode enabled");
    }
    let config = load_config(env_file)?;
    if verbose {
        println!("Configuration loaded:");
        println!("  Neighbors file: {}", config.neighbors_file.display());
        println!("  Capture interval: {}s", config.interval);
        println!("  TTL: {}s", config.ttl);
    }
    match capture::start_capture(&config) {
        Ok(_) => {
            println!("LLDP capture started successfully");
            if !verbose {
                println!("Neighbors file: {}", config.neighbors_file.display());
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to start LLDP capture: {e}");
            Err(1)
        }
    }
}

fn capture_stop(env_file: &Option<String>, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let config = load_config(env_file)?;
    match capture::stop_capture(&config) {
        Ok(_) => {
            println!("LLDP capture stopped successfully");
            Ok(())
        }
        Err(_) => {
            eprintln!("Failed to stop LLDP capture");
            Err(1)
        }
    }
}

fn capture_status(env_file: &Option<String>, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let config = load_config(env_file)?;
    match capture::capture_status(&config) {
        Some((pid, info)) => {
            println!("LLDP Capture Status: RUNNING");
            println!("  PID: {pid}");
            if let Some(backend) = capture::running_backend(&config.log_file) {
                println!("  Backend: {backend}");
            }
            match info {
                Some(info) => {
                    println!("  Uptime: {}", process::format_uptime(info.uptime));
                    println!("  Memory: {:.1} MB", info.memory_bytes as f64 / 1024.0 / 1024.0);
                    println!("  CPU: {:.1}%", info.cpu_percent);
                }
                None => {
                    println!("  Uptime: unknown");
                    println!("  Memory: 0.0 MB");
                    println!("  CPU: 0.0%");
                }
            }
        }
        None => println!("LLDP Capture Status: NOT RUNNING"),
    }
    Ok(())
}

fn capture_log(lines: i64, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let log_file = paths::log_file();
    if !log_file.exists() {
        println!("No log file found. Capture may not have been started yet.");
        return Ok(());
    }
    let text = match std::fs::read(&log_file) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => {
            eprintln!("Error reading log file: {e}");
            return Err(1);
        }
    };
    let all: Vec<&str> = text.lines().collect();
    let shown = if lines > 0 { &all[all.len().saturating_sub(lines as usize)..] } else { &all[..] };
    for line in shown {
        println!("{}", line.trim_end());
    }
    println!("\n[Showing last {} lines of {} total]", shown.len(), all.len());
    Ok(())
}

/// Python's isoformat timestamps, e.g. 2025-07-18T17:25:38.656100
fn parse_timestamp(text: &str) -> Option<chrono::NaiveDateTime> {
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|fmt| chrono::NaiveDateTime::parse_from_str(text, fmt).ok())
}

/// "Nd Hh", "Hh Mm" or "Mm Ss", like v1
fn format_age(age: chrono::TimeDelta) -> String {
    let micros = age.num_microseconds().unwrap_or(i64::MAX);
    let day = 86_400_000_000i64;
    let (days, seconds) = (micros.div_euclid(day), micros.rem_euclid(day) / 1_000_000);
    if days > 0 {
        format!("{days}d {}h", seconds / 3600)
    } else if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}

fn display_neighbors(neighbors_file: &Path) -> bool {
    if !neighbors_file.exists() {
        println!("No neighbors file found. Make sure capture is running.");
        return false;
    }
    let text = match fsdebug::read_to_string(neighbors_file) {
        Ok(text) => text,
        Err(e) => {
            println!("Error: {e}");
            return false;
        }
    };
    let Ok(Value::Object(neighbors)) = serde_json::from_str::<Value>(&text) else {
        println!("Error reading neighbors file. It may be corrupted.");
        return false;
    };

    let now = chrono::Local::now().naive_local();
    let empty = Map::new();
    let rows: Vec<Vec<String>> = neighbors
        .values()
        .map(|neighbor| {
            let field = |key: &str| neighbor.get(key).map(py_str);
            let data = neighbor.get("data").and_then(Value::as_object).unwrap_or(&empty);
            let get = |key: &str, default: &str| data.get(key).map(py_str).unwrap_or_else(|| default.to_string());
            let age = field("first_seen")
                .and_then(|t| parse_timestamp(&t))
                .map(|first| format_age(now - first))
                .unwrap_or_else(|| "Unknown".into());
            let port = data
                .get("port_description")
                .or_else(|| data.get("port_id"))
                .map(py_str)
                .unwrap_or_else(|| "Unknown".into());
            vec![
                field("interface").unwrap_or_else(|| "Unknown".into()),
                get("chassis_id", "Unknown"),
                get("system_name", "Unknown"),
                port,
                age,
                format!("{}s", get("ttl", "120")),
                get("management_address", "N/A"),
            ]
        })
        .collect();

    if rows.is_empty() {
        println!("No LLDP neighbors in file.");
    } else {
        let headers = ["Interface", "Neighbor", "System Name", "Port", "Age", "TTL", "Management IP"];
        println!("LLDP Neighbors:");
        println!("{}", table::grid(&headers, &rows));
        println!("\nTotal neighbors: {}", rows.len());
    }
    true
}

fn clear_screen() {
    if std::io::stdout().is_terminal() {
        let _ = std::process::Command::new("cmd").args(["/c", "cls"]).status();
    }
}

fn show_neighbors(env_file: &Option<String>, watch: bool, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let config = load_config(env_file)?;
    if !watch {
        display_neighbors(&config.neighbors_file);
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed));
    }
    println!("Watching LLDP neighbors... (Press Ctrl+C to exit)");
    'watch: loop {
        clear_screen();
        display_neighbors(&config.neighbors_file);
        println!("\nPress Ctrl+C to exit");
        for _ in 0..50 {
            if stop.load(Ordering::Relaxed) {
                break 'watch;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    println!("\nExiting...");
    Ok(())
}

fn send(env_file: &Option<String>, interface: &Option<String>, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let mut config = load_config(env_file)?;
    if let Some(interface) = interface {
        config.interface = interface.clone();
    }
    if verbose {
        println!("Configuration:");
        println!("  Interface: {}", config.interface);
        println!("  System Name: {}", config.system_name);
        println!("  TTL: {}", config.ttl);
        println!();
    }
    println!("Sending LLDP packets...");
    let report = sender::send_lldp(&config, verbose);
    if report.errors.is_empty() {
        println!("LLDP packets sent successfully!");
        Ok(())
    } else {
        eprintln!("Failed to send LLDP packets ({} sent, {} failed)", report.sent, report.errors.len());
        Err(1)
    }
}

fn show_interfaces(verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let rows: Vec<Vec<String>> = sysinfo::interfaces()
        .into_iter()
        .filter_map(|i| {
            let mac = i.mac?;
            let ipv4 = if i.ipv4.is_empty() { "N/A".to_string() } else { i.ipv4.join(", ") };
            Some(vec![i.name, mac, if i.is_up { "UP" } else { "DOWN" }.to_string(), ipv4])
        })
        .collect();
    println!("Network Interfaces:");
    println!("{}", table::grid(&["Interface", "MAC Address", "Status", "IPv4 Addresses"], &rows));
    Ok(())
}

fn show_config(env_file: &Option<String>, verbose: bool) -> Result<(), i32> {
    setup_logging(verbose);
    let config = load_config(env_file)?;
    println!("Current Configuration:");
    println!("{config}");
    Ok(())
}

fn debug_paths(env_file: &Option<String>) -> Result<(), i32> {
    let config = load_config(env_file)?;
    let exists = |p: &Path| if p.exists() { "True" } else { "False" };
    println!("Configuration:");
    println!("  Config file: {}", env_file.as_deref().unwrap_or(".env (default)"));
    println!(
        "  LLDP_NEIGHBORS_FILE: {}",
        std::env::var("LLDP_NEIGHBORS_FILE").unwrap_or_else(|_| "neighbors.json (default)".into())
    );
    println!("\nFile paths:");
    println!("  Neighbors file: {}", config.neighbors_file.display());
    println!("  Exists: {}", exists(&config.neighbors_file));
    if let Ok(meta) = std::fs::metadata(&config.neighbors_file) {
        println!("  Size: {} bytes", meta.len());
    }
    println!("\n  PID file: {}", config.pid_file.display());
    println!("  Exists: {}", exists(&config.pid_file));
    println!("\n  Log file: {}", config.log_file.display());
    println!("  Exists: {}", exists(&config.log_file));
    Ok(())
}

fn clear_neighbors(env_file: &Option<String>) -> Result<(), i32> {
    let config = load_config(env_file)?;
    let file = &config.neighbors_file;
    if !file.exists() {
        println!("No neighbors file found at: {}", file.display());
        return Ok(());
    }
    match fsdebug::write(file, "{}") {
        Ok(()) => {
            println!("Cleared all discovered neighbors from: {}", file.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("Error clearing neighbors: {e}");
            Err(1)
        }
    }
}

fn exit(code: i32) -> ExitCode {
    ExitCode::from(code.clamp(0, 255) as u8)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Version => {
            println!("WinLLDP version {}", winlldp::VERSION);
            Ok(())
        }
        Commands::Capture(c) => match c {
            CaptureCommands::Start { env, verbose } => capture_start(&env.env_file, verbose.verbose),
            CaptureCommands::Stop { env, verbose } => capture_stop(&env.env_file, verbose.verbose),
            CaptureCommands::Status { env, verbose } => capture_status(&env.env_file, verbose.verbose),
            CaptureCommands::Log { lines, verbose } => capture_log(*lines, verbose.verbose),
        },
        Commands::ShowNeighbors { env, watch, verbose } => show_neighbors(&env.env_file, *watch, verbose.verbose),
        Commands::Send { env, interface, verbose } => send(&env.env_file, interface, *verbose),
        Commands::ShowInterfaces { verbose } => show_interfaces(verbose.verbose),
        Commands::ShowConfig { env, verbose } => show_config(&env.env_file, verbose.verbose),
        Commands::DebugPaths { env } => debug_paths(&env.env_file),
        Commands::ClearNeighbors { env } => clear_neighbors(&env.env_file),
        Commands::Service(s) => {
            let code = match s {
                ServiceCommands::Install(v) => {
                    setup_logging(v.verbose);
                    service::install(v.verbose)
                }
                ServiceCommands::Uninstall(v) => {
                    setup_logging(v.verbose);
                    service::uninstall(v.verbose)
                }
                ServiceCommands::Start(v) => {
                    setup_logging(v.verbose);
                    service::start(v.verbose)
                }
                ServiceCommands::Stop(v) => {
                    setup_logging(v.verbose);
                    service::stop(v.verbose)
                }
                ServiceCommands::Status(v) => {
                    setup_logging(v.verbose);
                    service::status(v.verbose)
                }
                ServiceCommands::Restart(v) => {
                    setup_logging(v.verbose);
                    service::restart(v.verbose)
                }
            };
            if code == 0 {
                Ok(())
            } else {
                Err(code)
            }
        }
        Commands::CaptureSubprocess { log_file, neighbors_file, pid_file } => {
            let code =
                capture::run_capture_process(Path::new(log_file), Path::new(neighbors_file), Path::new(pid_file));
            if code == 0 {
                Ok(())
            } else {
                Err(code)
            }
        }
        Commands::ServiceRun => match service::run_service() {
            0 => Ok(()),
            code => Err(code),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => exit(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_format_matches_v1() {
        let d = |s: i64| chrono::TimeDelta::seconds(s);
        assert_eq!(format_age(d(42)), "0m 42s");
        assert_eq!(format_age(d(5 * 60 + 3)), "5m 3s");
        assert_eq!(format_age(d(2 * 3600 + 5 * 60)), "2h 5m");
        assert_eq!(format_age(d(3 * 86400 + 4 * 3600)), "3d 4h");
        // A first_seen in the future behaves like Python's timedelta (-1 day + seconds)
        assert_eq!(format_age(d(-10)), "23h 59m");
    }

    #[test]
    fn timestamps_from_v1_and_v2() {
        assert!(parse_timestamp("2025-07-18T17:25:38.656100").is_some());
        assert!(parse_timestamp("2024-01-01T00:00:00").is_some());
        assert!(parse_timestamp("garbage").is_none());
    }

    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
