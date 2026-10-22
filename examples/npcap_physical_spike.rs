//! Spike: send an LLDP frame through Npcap on a given NPF device (e.g. a physical NIC behind a
//! Hyper-V switch):  npcap_physical_spike "\Device\NPF_{GUID}" ["Interface name for MAC/IP"]

use winlldp::{config::Config, pcap, sender, sysinfo};

fn main() {
    let device = std::env::args().nth(1).expect("NPF device name");
    let name = std::env::args().nth(2).unwrap_or("vEthernet (ymbihq.local)".into());
    let iface = sysinfo::interface_by_name(&name).expect("interface not found");
    let mut config = Config::load(None).unwrap();
    config.system_name = "npcap-test".into();
    let local = sender::LocalSystem::detect();
    let data = sender::create_lldp_packet(&config, &local, &iface, false).unwrap();
    let src = sysinfo::mac_to_bytes(iface.mac.as_deref().unwrap()).unwrap();
    let frame = pcap::ethernet_frame(&pcap::LLDP_MULTICAST_MAC, &src, pcap::LLDP_ETHERTYPE, &data);
    let mut handle = pcap::Handle::open(&device, false, 100).expect("open");
    handle.send(&frame).expect("send");
    println!("sent {} bytes on {device}", frame.len());
}
