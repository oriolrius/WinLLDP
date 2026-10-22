//! LLDP transmission: builds the LLDPDU of each interface and sends it

use crate::config::Config;
use crate::packet::{self, LldpPacket};
use crate::pcap::{self, Handle};
use crate::sysinfo::{self, Interface};
use crate::{logger, VERSION};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// TIA OUI used by the LLDP-MED TLVs
const LLDP_MED_OUI: [u8; 3] = [0x00, 0x12, 0xBB];

/// Values that do not change between packets
pub struct LocalSystem {
    pub hostname: String,
    pub system_description: String,
    pub machine: String,
    pub os_version: String,
    pub os_caption: Option<String>,
}

impl LocalSystem {
    pub fn detect() -> Self {
        LocalSystem {
            hostname: sysinfo::hostname(),
            system_description: sysinfo::system_description(),
            machine: sysinfo::machine(),
            os_version: sysinfo::os_version(),
            os_caption: sysinfo::os_caption(),
        }
    }
}

/// The LLDPDU for an interface; `minimal` keeps only the mandatory TLVs
pub fn create_lldp_packet(
    config: &Config,
    local: &LocalSystem,
    iface: &Interface,
    minimal: bool,
) -> Result<Vec<u8>, String> {
    let mut p = LldpPacket::new();
    if let Some(mac) = iface.mac.as_deref().and_then(sysinfo::mac_to_bytes) {
        p.add_chassis_id(packet::CHASSIS_ID_MAC_ADDRESS, &mac);
        p.add_port_id(packet::PORT_ID_MAC_ADDRESS, &mac);
    }
    p.add_ttl(config.ttl);
    if !minimal {
        let port_description = if config.port_description.is_empty() { &iface.name } else { &config.port_description };
        p.add_port_description(port_description);
        let system_name = if config.system_name == "auto" { &local.hostname } else { &config.system_name };
        p.add_system_name(system_name);
        // Always the detailed Windows platform/version, e.g. "Windows 10.0.26100 AMD64"
        p.add_system_description(&local.system_description);

        // LLDP-MED TLVs, which devices such as MikroTik show as platform and version.
        // Capabilities: LLDP-MED capabilities supported, device type Endpoint Class III
        p.add_organizationally_specific(LLDP_MED_OUI, 1, &[0x00, 0x01, 0x03, 0x00]);
        p.add_organizationally_specific(LLDP_MED_OUI, 5, local.machine.as_bytes()); // hardware revision
        p.add_organizationally_specific(LLDP_MED_OUI, 6, local.os_version.as_bytes()); // firmware revision
        p.add_organizationally_specific(LLDP_MED_OUI, 7, format!("WinLLDP {VERSION}").as_bytes()); // software revision
        if let Some(caption) = &local.os_caption {
            p.add_organizationally_specific(LLDP_MED_OUI, 9, caption.as_bytes());
            // model name
        }

        let (capabilities, enabled) = sysinfo::system_capabilities();
        p.add_system_capabilities(capabilities, enabled);

        let management_address = match config.management_address.as_str() {
            "auto" => iface.ipv4.first().cloned().or_else(sysinfo::primary_ip),
            address => Some(address.to_string()),
        };
        if let Some((family, bytes)) = management_address.as_deref().and_then(sysinfo::ip_to_bytes) {
            let index = if iface.if_index != 0 { iface.if_index } else { sysinfo::interface_index(&iface.name) };
            p.add_management_address(family, &bytes, index, b"");
        }
    }
    p.add_end_of_lldpdu();
    p.encode()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn print_packet(iface: &Interface, mac: &str, data: &[u8]) {
    println!("Sending LLDP on {} ({mac})", iface.name);
    println!("  Destination: {}", packet::format_mac(&pcap::LLDP_MULTICAST_MAC));
    println!("  Packet size: {} bytes", data.len());
    println!("  Raw LLDP data: {}", hex(data));
    let info = LldpPacket::decode(data).to_dict();
    let field = |key: &str| info.get(key).map(crate::py_str).unwrap_or_else(|| "N/A".into());
    println!("\n  Packet contents:");
    println!("    Chassis ID: {}", field("chassis_id"));
    println!("    Port ID: {}", field("port_id"));
    println!("    TTL: {}s", field("ttl"));
    println!("    Port Description: {}", field("port_description"));
    println!("    System Name: {}", field("system_name"));
    println!("    System Description: {}", field("system_description"));
    println!("    Management Address: {}", field("management_address"));
    if let Some(caps) = info.get("system_capabilities") {
        println!("    System Capabilities: {caps}");
    }
    println!();
}

/// Send the LLDPDU on one interface (skipped when it is down or has no MAC)
pub fn send_on_interface(
    config: &Config,
    local: &LocalSystem,
    iface: &Interface,
    verbose: bool,
) -> Result<bool, String> {
    let Some(mac) = iface.mac.as_deref().filter(|_| iface.is_up) else {
        return Ok(false);
    };
    let result = (|| {
        let src = sysinfo::mac_to_bytes(mac).filter(|m| m.len() == 6).ok_or(format!("invalid MAC address {mac}"))?;
        let data = create_lldp_packet(config, local, iface, false)?;
        if verbose {
            print_packet(iface, mac, &data);
        }
        let frame = pcap::ethernet_frame(&pcap::LLDP_MULTICAST_MAC, &src, pcap::LLDP_ETHERTYPE, &data);
        let mut handle = Handle::open(&iface.npcap_device(), false, 100)?;
        handle.send(&frame)?;
        if verbose {
            println!(".\nSent 1 packets.");
            println!("  Sent successfully!");
        }
        Ok(true)
    })();
    result.map_err(|e: String| {
        let message = format!("Error sending LLDP on {}: {e}", iface.name);
        logger::error(&message);
        message
    })
}

#[derive(Debug, Default)]
pub struct SendReport {
    pub sent: usize,
    pub errors: Vec<String>,
}

/// Send on every active interface (LLDP_INTERFACE=all) or on the configured one
pub fn send_lldp(config: &Config, verbose: bool) -> SendReport {
    let local = LocalSystem::detect();
    let mut report = SendReport::default();
    let targets: Vec<Interface> = if config.interface == "all" {
        sysinfo::interfaces().into_iter().filter(|i| i.is_up && i.mac.is_some()).collect()
    } else {
        match sysinfo::interface_by_name(&config.interface) {
            Some(iface) => vec![iface],
            None => {
                let message = format!("Interface {} not found", config.interface);
                logger::error(&message);
                report.errors.push(message);
                vec![]
            }
        }
    };
    for iface in &targets {
        match send_on_interface(config, &local, iface, verbose) {
            Ok(true) => report.sent += 1,
            Ok(false) => {}
            Err(e) => report.errors.push(e),
        }
    }
    report
}

/// Send every `interval` seconds until `stop` is set
pub fn run_loop(config: &Config, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        send_lldp(config, false);
        for _ in 0..config.interval * 10 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(vars: &[(&str, &str)]) -> Config {
        let vars: std::collections::HashMap<String, String> =
            vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Config::from_env(|n| vars.get(n).cloned()).unwrap()
    }

    fn local() -> LocalSystem {
        LocalSystem {
            hostname: "d0".into(),
            system_description: "Windows 10.0.26200 AMD64".into(),
            machine: "AMD64".into(),
            os_version: "10.0.26200".into(),
            os_caption: Some("Microsoft Windows 11 Pro".into()),
        }
    }

    fn iface() -> Interface {
        Interface {
            name: "vEthernet (ymbihq.local)".into(),
            adapter_name: "{X}".into(),
            description: "Hyper-V Virtual Ethernet Adapter #2".into(),
            mac: Some("50:eb:f6:2c:f2:ba".into()),
            is_up: true,
            ipv4: vec!["10.2.0.47".into()],
            ipv6: vec![],
            if_index: 11,
        }
    }

    #[test]
    fn full_packet_contents() {
        let data = create_lldp_packet(&config(&[]), &local(), &iface(), false).unwrap();
        let p = LldpPacket::decode(&data);
        let info = p.to_dict();
        assert_eq!(info["chassis_id"], "50:eb:f6:2c:f2:ba");
        assert_eq!(info["port_id"], "50:eb:f6:2c:f2:ba");
        assert_eq!(info["ttl"], 120);
        assert_eq!(info["port_description"], "Ethernet Port");
        assert_eq!(info["system_name"], "d0");
        assert_eq!(info["system_description"], "Windows 10.0.26200 AMD64");
        assert_eq!(info["management_address"], "10.2.0.47");
        assert_eq!(info["system_capabilities"], 128);
        let med: Vec<(u8, Vec<u8>)> = p
            .tlvs
            .iter()
            .filter(|t| t.tlv_type == packet::ORGANIZATIONALLY_SPECIFIC)
            .map(|t| (t.value[3], t.value[4..].to_vec()))
            .collect();
        assert_eq!(
            med,
            vec![
                (1, vec![0, 1, 3, 0]),
                (5, b"AMD64".to_vec()),
                (6, b"10.0.26200".to_vec()),
                (7, format!("WinLLDP {VERSION}").into_bytes()),
                (9, b"Microsoft Windows 11 Pro".to_vec()),
            ]
        );
        // Management address TLV with ifIndex 11, as v1 encodes it
        assert!(hex(&data).ends_with("100c05010a02002f020000000b000000"));
    }

    #[test]
    fn configured_values() {
        let cfg = config(&[
            ("LLDP_SYSTEM_NAME", "my-pc"),
            ("LLDP_PORT_DESCRIPTION", "Desk port"),
            ("LLDP_MANAGEMENT_ADDRESS", "2001:db8::5"),
            ("LLDP_TTL", "300"),
        ]);
        let info = LldpPacket::decode(&create_lldp_packet(&cfg, &local(), &iface(), false).unwrap()).to_dict();
        assert_eq!(info["system_name"], "my-pc");
        assert_eq!(info["port_description"], "Desk port");
        assert_eq!(info["management_address"], "2001:db8::5");
        assert_eq!(info["ttl"], 300);
    }

    #[test]
    fn minimal_packet_has_only_mandatory_tlvs() {
        let data = create_lldp_packet(&config(&[]), &local(), &iface(), true).unwrap();
        let types: Vec<u8> = LldpPacket::decode(&data).tlvs.iter().map(|t| t.tlv_type).collect();
        assert_eq!(types, vec![packet::CHASSIS_ID, packet::PORT_ID, packet::TTL, packet::END_OF_LLDPDU]);
    }

    #[test]
    fn down_interfaces_are_skipped() {
        let down = Interface { is_up: false, ..iface() };
        assert_eq!(send_on_interface(&config(&[]), &local(), &down, false), Ok(false));
    }
}
