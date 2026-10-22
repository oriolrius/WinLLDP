//! Spike for ADR 0001 task 2: does Packet Monitor deliver LLDP frames, and how do
//! they map to adapters? Run as administrator:
//!   pktmon_spike [seconds] [--all-ethertypes] [--swap-ethertype] [--all-sources]

use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use winlldp::packet::{format_mac, LldpPacket};
use winlldp::pktmon::{self, *};

static PACKETS: AtomicU64 = AtomicU64::new(0);
static LLDP: AtomicU64 = AtomicU64::new(0);
static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

unsafe extern "system" fn on_event(_ctx: *mut c_void, _info: *const c_void, kind: i32) {
    let name = ["Started", "Stopped", "FatalError", "ProcessInfo"].get(kind as usize).copied().unwrap_or("?");
    eprintln!("[event] {name}");
}

unsafe extern "system" fn on_data(_ctx: *mut c_void, data: *const StreamDataDescriptor) {
    let d = &*data;
    let (packet, meta) = d.packet();
    PACKETS.fetch_add(1, Ordering::Relaxed);
    if packet.len() < 14 {
        return;
    }
    let ether_type = u16::from_be_bytes([packet[12], packet[13]]);
    let mut line = format!(
        "comp={:>4} edge={} dir={:<7} type={} len={:>4}/{:<4} missed={}/{} {} -> {} 0x{:04x}",
        meta.component_id,
        meta.edge_id,
        pktmon::DIRECTIONS.get(meta.direction_name as usize).copied().unwrap_or("?"),
        meta.packet_type,
        packet.len(),
        d.packet_length,
        d.missed_packet_write_count,
        d.missed_packet_read_count,
        format_mac(&packet[6..12]),
        format_mac(&packet[0..6]),
        ether_type
    );
    if ether_type == 0x88cc {
        LLDP.fetch_add(1, Ordering::Relaxed);
        let info = LldpPacket::decode(&packet[14..]).to_dict();
        let get = |k: &str| info.get(k).map(winlldp::py_str).unwrap_or_default();
        line += &format!(
            " LLDP chassis={} name={} desc={:?} mgmt={}",
            get("chassis_id"),
            get("system_name"),
            get("system_description"),
            get("management_address")
        );
    }
    LINES.lock().unwrap().push(line);
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seconds: u64 = args.iter().find_map(|a| a.parse().ok()).unwrap_or(40);
    let all_ethertypes = args.iter().any(|a| a == "--all-ethertypes");
    let swap = args.iter().any(|a| a == "--swap-ethertype");
    let all_sources = args.iter().any(|a| a == "--all-sources");

    let monitor = PacketMonitor::new()?;
    println!("== Network interface data sources (showHidden=false)");
    let nics = monitor.data_sources(DATA_SOURCE_KIND_NETWORK_INTERFACE, false)?;
    for s in nics.iter() {
        println!(
            "id={:>4} secondary={:>4} parent={:>4} present=0x{:x} mac={:<17} name={:?} desc={:?}",
            s.id,
            s.secondary_id,
            s.parent_id,
            s.is_present,
            s.mac().map(|m| format_mac(&m)).unwrap_or_default(),
            s.name(),
            s.description()
        );
    }
    let all = monitor.data_sources(DATA_SOURCE_KIND_ALL, true)?;
    println!("== All data sources (showHidden=true): {}", all.iter().count());
    if args.iter().any(|a| a == "--list-all") {
        for s in all.iter() {
            println!(
                "  kind={} id={:>4} secondary={:>4} parent={:>4} mac={:<17} name={:?} desc={:?}",
                s.kind,
                s.id,
                s.secondary_id,
                s.parent_id,
                s.mac().map(|m| format_mac(&m)).unwrap_or_default(),
                s.name(),
                s.description()
            );
        }
        return Ok(());
    }

    let mut session = monitor.session("winlldp-spike")?;
    let sources = if all_sources { monitor.data_sources(DATA_SOURCE_KIND_ALL, false)? } else { nics };
    for s in sources.iter() {
        if let Err(e) = session.add_data_source(s) {
            println!("cannot add {}: {e}", s.name());
        }
    }
    if !all_ethertypes {
        let ether_type = if swap { 0x88ccu16.swap_bytes() } else { 0x88cc };
        session.add_constraint(&ProtocolConstraint::ether_type("lldp", ether_type))?;
        println!("== constraint EtherType 0x{ether_type:04x}");
    }
    let config = RealtimeStreamConfiguration {
        user_context: std::ptr::null_mut(),
        event_callback: Some(on_event),
        data_callback: Some(on_data),
        buffer_size_multiplier: 4,
        truncation_size: 9000,
    };
    session.attach_stream(&config)?;
    session.set_active(true)?;
    println!("== capturing for {seconds}s");
    for _ in 0..seconds {
        std::thread::sleep(std::time::Duration::from_secs(1));
        for line in LINES.lock().unwrap().drain(..).take(200) {
            println!("{line}");
        }
    }
    session.set_active(false)?;
    drop(session);
    println!("== packets={} lldp={}", PACKETS.load(Ordering::Relaxed), LLDP.load(Ordering::Relaxed));
    Ok(())
}
