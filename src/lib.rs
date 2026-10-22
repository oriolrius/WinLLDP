//! WinLLDP: LLDP (IEEE 802.1AB) for Windows

pub mod capture;
pub mod config;
pub mod dotenv;
pub mod fsdebug;
pub mod logger;
pub mod packet;
pub mod paths;
pub mod pcap;
pub mod pktmon;
pub mod process;
pub mod sender;
pub mod service;
pub mod sysinfo;
pub mod table;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A JSON value as Python's str() shows it (strings without quotes)
pub fn py_str(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        other => other.to_string(),
    }
}
