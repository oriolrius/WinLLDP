//! Configuration: environment variables, then the .env file, then defaults

use crate::{dotenv, paths};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub interval: u64,
    pub interface: String,
    pub system_name: String,
    pub system_description: String,
    pub port_description: String,
    pub management_address: String,
    pub ttl: u16,
    pub neighbors_file: PathBuf,
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
    pub runtime_dir: PathBuf,
}

impl Config {
    /// Load `env_file` (or the first .env found from the current directory upwards)
    /// without overriding variables already set in the environment
    pub fn load(env_file: Option<&str>) -> Result<Config, String> {
        match env_file {
            Some(file) => {
                dotenv::load(Path::new(file));
            }
            None => {
                if let Some(file) = std::env::current_dir().ok().and_then(|dir| dotenv::find(&dir)) {
                    dotenv::load(&file);
                }
            }
        }
        Self::from_env(|name| std::env::var(name).ok())
    }

    pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
        let get = |name: &str, default: &str| var(name).unwrap_or_else(|| default.to_string());
        let int = |name: &str, default: &str| -> Result<i64, String> {
            let value = get(name, default);
            value.trim().parse::<i64>().map_err(|_| format!("invalid literal for int() with base 10: '{value}'"))
        };

        let interval = int("LLDP_INTERVAL", "30")?;
        let ttl = int("LLDP_TTL", "120")?;
        if interval < 5 {
            return Err("LLDP_INTERVAL must be at least 5 seconds".into());
        }
        if ttl < interval {
            return Err("LLDP_TTL must be greater than LLDP_INTERVAL".into());
        }
        if ttl > 65535 {
            return Err("LLDP_TTL must be less than 65536".into());
        }

        // A custom neighbors file is only used when it is an absolute path
        let custom = get("LLDP_NEIGHBORS_FILE", "");
        let neighbors_file = if !custom.is_empty() && Path::new(&custom).is_absolute() {
            PathBuf::from(custom)
        } else {
            paths::neighbors_file()
        };

        Ok(Config {
            interval: interval as u64,
            interface: get("LLDP_INTERFACE", "all"),
            system_name: get("LLDP_SYSTEM_NAME", "auto"),
            system_description: get("LLDP_SYSTEM_DESCRIPTION", "Windows LLDP Service"),
            port_description: get("LLDP_PORT_DESCRIPTION", "Ethernet Port"),
            management_address: get("LLDP_MANAGEMENT_ADDRESS", "auto"),
            ttl: ttl as u16,
            neighbors_file,
            pid_file: paths::pid_file(),
            log_file: paths::log_file(),
            runtime_dir: paths::runtime_directory(),
        })
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Config(\n  interval={}s,\n  interface={},\n  system_name={},\n  system_description={},\n  \
             port_description={},\n  management_address={},\n  ttl={}s,\n  neighbors_file={}\n)",
            self.interval,
            self.interface,
            self.system_name,
            self.system_description,
            self.port_description,
            self.management_address,
            self.ttl,
            self.neighbors_file.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Result<Config, String> {
        let vars: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_env(|name| vars.get(name).cloned())
    }

    #[test]
    fn defaults() {
        let c = config(&[]).unwrap();
        assert_eq!((c.interval, c.ttl), (30, 120));
        assert_eq!(c.interface, "all");
        assert_eq!(c.system_name, "auto");
        assert_eq!(c.system_description, "Windows LLDP Service");
        assert_eq!(c.port_description, "Ethernet Port");
        assert_eq!(c.management_address, "auto");
        assert_eq!(c.neighbors_file, paths::neighbors_file());
        assert_eq!(c.pid_file, paths::pid_file());
        assert_eq!(c.log_file, paths::log_file());
    }

    #[test]
    fn validation() {
        assert_eq!(config(&[("LLDP_INTERVAL", "4")]).unwrap_err(), "LLDP_INTERVAL must be at least 5 seconds");
        assert_eq!(
            config(&[("LLDP_INTERVAL", "60"), ("LLDP_TTL", "30")]).unwrap_err(),
            "LLDP_TTL must be greater than LLDP_INTERVAL"
        );
        assert_eq!(config(&[("LLDP_TTL", "65536")]).unwrap_err(), "LLDP_TTL must be less than 65536");
        assert!(config(&[("LLDP_TTL", "abc")]).unwrap_err().contains("'abc'"));
        assert_eq!(config(&[("LLDP_TTL", "65535")]).unwrap().ttl, 65535);
    }

    #[test]
    fn custom_neighbors_file_must_be_absolute() {
        let c = config(&[("LLDP_NEIGHBORS_FILE", r"C:\data\n.json")]).unwrap();
        assert_eq!(c.neighbors_file, PathBuf::from(r"C:\data\n.json"));
        let c = config(&[("LLDP_NEIGHBORS_FILE", "neighbors.json")]).unwrap();
        assert_eq!(c.neighbors_file, paths::neighbors_file());
    }

    #[test]
    fn display_matches_v1() {
        let c = config(&[]).unwrap();
        let text = c.to_string();
        assert!(text.starts_with("Config(\n  interval=30s,\n  interface=all,\n  system_name=auto,\n"));
        assert!(text.ends_with(&format!("  ttl=120s,\n  neighbors_file={}\n)", c.neighbors_file.display())));
    }
}
