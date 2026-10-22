//! Windows Packet Monitor (pktmon) real-time capture API, loaded at run time from
//! PktMonApi.dll (Windows 11 24H2+ / Windows Server 2025). See docs/decisions/0001-npcap.md.
//!
//! Layouts follow the documented packetmonitor.h definitions
//! (PACKETMONITOR_MAX_NAME_LENGTH 64, PACKETMONITOR_MAX_STRING_LENGTH 128).

use std::ffi::c_void;
use std::sync::OnceLock;

pub const API_VERSION_1_0: u32 = 0x0001_0000;
pub const MAX_NAME_LENGTH: usize = 64;
pub const MAX_STRING_LENGTH: usize = 128;

pub const DATA_SOURCE_KIND_ALL: i32 = 0;
pub const DATA_SOURCE_KIND_NETWORK_INTERFACE: i32 = 1;

/// PACKETMONITOR_PROTOCOL_CONSTRAINT.IsPresent bits
pub const CONSTRAINT_MAC1: u32 = 1 << 0;
pub const CONSTRAINT_MAC2: u32 = 1 << 1;
pub const CONSTRAINT_ETHER_TYPE: u32 = 1 << 3;

/// PACKETMONITOR_DATA_SOURCE_SPECIFICATION.IsPresent bits
pub const SOURCE_GUID: u32 = 1 << 0;
pub const SOURCE_IPV6: u32 = 1 << 1;
pub const SOURCE_IPV4: u32 = 1 << 2;
pub const SOURCE_MAC: u32 = 1 << 3;

/// PKTMON_DIRECTION_TAG
pub const DIRECTIONS: [&str; 7] = ["Unspecified", "In", "Out", "Rx", "Tx", "Ingress", "Egress"];
/// PKTMON_PACKET_TYPE: 1 = Ethernet
pub const PACKET_TYPE_ETHERNET: u16 = 1;

/// PACKETMONITOR_IP_ADDRESS (8-byte aligned union of 16 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpAddress {
    pub bytes: [u64; 2],
}

#[repr(C)]
pub struct DataSourceSpecification {
    pub kind: i32,
    pub name: [u16; MAX_NAME_LENGTH],
    pub description: [u16; MAX_STRING_LENGTH],
    pub id: u32,
    pub secondary_id: u32,
    pub parent_id: u32,
    pub is_present: u32,
    /// union { GUID; PACKETMONITOR_IP_ADDRESS; UCHAR MacAddress[6] }
    pub detail: IpAddress,
}

impl DataSourceSpecification {
    pub fn name(&self) -> String {
        wide_to_string(&self.name)
    }

    pub fn description(&self) -> String {
        wide_to_string(&self.description)
    }

    pub fn mac(&self) -> Option<[u8; 6]> {
        if self.is_present & SOURCE_MAC == 0 {
            return None;
        }
        let bytes: [u8; 16] = unsafe { std::mem::transmute(self.detail) };
        Some(bytes[..6].try_into().unwrap())
    }
}

#[repr(C)]
struct DataSourceList {
    count: u32,
    sources: [*const DataSourceSpecification; 1],
}

#[repr(C)]
pub struct ProtocolConstraint {
    pub name: [u16; MAX_NAME_LENGTH],
    pub is_present: u32,
    pub mac1: [u8; 6],
    pub mac2: [u8; 6],
    pub vlan_id: u16,
    pub ether_type: u16,
    pub dscp: u16,
    pub transport_protocol: u8,
    pub ip1: IpAddress,
    pub ip2: IpAddress,
    pub prefix_length1: u8,
    pub prefix_length2: u8,
    pub port1: u16,
    pub port2: u16,
    pub tcp_flags: u8,
    pub encap_type: u32,
    pub vxlan_port: u16,
    pub packets: u64,
    pub bytes: u64,
}

impl ProtocolConstraint {
    pub fn ether_type(name: &str, ether_type: u16) -> Self {
        let mut c: ProtocolConstraint = unsafe { std::mem::zeroed() };
        for (dst, src) in c.name.iter_mut().zip(name.encode_utf16().take(MAX_NAME_LENGTH - 1)) {
            *dst = src;
        }
        c.is_present = CONSTRAINT_ETHER_TYPE;
        c.ether_type = ether_type;
        c
    }
}

pub type EventCallback = unsafe extern "system" fn(*mut c_void, *const c_void, i32);
pub type DataCallback = unsafe extern "system" fn(*mut c_void, *const StreamDataDescriptor);

#[repr(C)]
pub struct RealtimeStreamConfiguration {
    pub user_context: *mut c_void,
    pub event_callback: Option<EventCallback>,
    pub data_callback: Option<DataCallback>,
    pub buffer_size_multiplier: u16,
    pub truncation_size: u16,
}

#[repr(C)]
pub struct StreamDataDescriptor {
    pub data: *const u8,
    pub data_size: u32,
    pub metadata_offset: u32,
    pub packet_offset: u32,
    pub packet_length: u32,
    pub missed_packet_write_count: u32,
    pub missed_packet_read_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StreamMetadata {
    pub pkt_group_id: u64,
    pub pkt_count: u16,
    pub appearance_count: u16,
    pub direction_name: u16,
    pub packet_type: u16,
    pub component_id: u16,
    pub edge_id: u16,
    pub reserved: u16,
    pub drop_reason: u32,
    pub drop_location: u32,
    pub processor: u16,
    pub timestamp: i64,
}

impl StreamDataDescriptor {
    /// The packet bytes and its metadata (valid only during the callback)
    ///
    /// # Safety
    /// `self` must be a descriptor passed to a data callback by Packet Monitor.
    pub unsafe fn packet(&self) -> (&[u8], StreamMetadata) {
        let packet =
            std::slice::from_raw_parts(self.data.add(self.packet_offset as usize), self.packet_length as usize);
        let metadata = std::ptr::read_unaligned(self.data.add(self.metadata_offset as usize) as *const StreamMetadata);
        (packet, metadata)
    }
}

/// The fields of a data source needed to map packets to interfaces
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    pub id: u32,
    pub secondary_id: u32,
    pub name: String,
    pub description: String,
}

impl From<&DataSourceSpecification> for SourceInfo {
    fn from(s: &DataSourceSpecification) -> Self {
        SourceInfo { id: s.id, secondary_id: s.secondary_id, name: s.name(), description: s.description() }
    }
}

/// Map every component ID (primary and secondary) to a WinLLDP interface name.
///
/// `sources` is the hidden-included list (`showHidden = true`), where a data source may appear
/// several times with the same `id` and different `secondary_id`s; the entry with
/// `id == secondary_id` describes it. Its description is the interface's friendly name for
/// Hyper-V host ports ("vEthernet (...)") and the adapter description for other NICs.
/// `interfaces` are the active interfaces as (friendly name, adapter description).
pub fn component_interfaces(
    sources: &[SourceInfo],
    interfaces: &[(String, String)],
) -> std::collections::HashMap<u32, String> {
    let mut map = std::collections::HashMap::new();
    for primary in sources.iter().filter(|s| s.id == s.secondary_id && !s.description.is_empty()) {
        let interface = interfaces
            .iter()
            .find(|(name, _)| *name == primary.description)
            .or_else(|| interfaces.iter().find(|(_, description)| *description == primary.description));
        if let Some((name, _)) = interface {
            for source in sources.iter().filter(|s| s.id == primary.id) {
                map.insert(source.id, name.clone());
                map.insert(source.secondary_id, name.clone());
            }
        }
    }
    map
}

pub fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|c| *c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}

type Handle = *mut c_void;

type Initialize = unsafe extern "system" fn(u32, *mut c_void, *mut Handle) -> i32;
type Uninitialize = unsafe extern "system" fn(Handle);
type EnumDataSources = unsafe extern "system" fn(Handle, i32, u8, usize, *mut usize, *mut DataSourceList) -> i32;
type CreateLiveSession = unsafe extern "system" fn(Handle, *const u16, *mut Handle) -> i32;
type CloseSessionHandle = unsafe extern "system" fn(Handle);
type AddSingleDataSourceToSession = unsafe extern "system" fn(Handle, *const DataSourceSpecification) -> i32;
type AddCaptureConstraint = unsafe extern "system" fn(Handle, *const ProtocolConstraint) -> i32;
type SetSessionActive = unsafe extern "system" fn(Handle, u8) -> i32;
type CreateRealtimeStream = unsafe extern "system" fn(Handle, *const RealtimeStreamConfiguration, *mut Handle) -> i32;
type CloseRealtimeStream = unsafe extern "system" fn(Handle);
type AttachOutputToSession = unsafe extern "system" fn(Handle, Handle) -> i32;

pub struct Api {
    _lib: libloading::Library,
    initialize: Initialize,
    uninitialize: Uninitialize,
    enum_data_sources: EnumDataSources,
    create_live_session: CreateLiveSession,
    close_session_handle: CloseSessionHandle,
    add_single_data_source_to_session: AddSingleDataSourceToSession,
    add_capture_constraint: AddCaptureConstraint,
    set_session_active: SetSessionActive,
    create_realtime_stream: CreateRealtimeStream,
    close_realtime_stream: CloseRealtimeStream,
    attach_output_to_session: AttachOutputToSession,
}

fn check(what: &str, hr: i32) -> Result<(), String> {
    if hr < 0 {
        return Err(format!("{what} failed: HRESULT 0x{:08X}", hr as u32));
    }
    Ok(())
}

impl Api {
    fn load() -> Result<Api, String> {
        let path = crate::sysinfo::system32().join("PktMonApi.dll");
        let lib = unsafe { libloading::Library::new(&path) }
            .map_err(|e| format!("Packet Monitor API not available ({}): {e}", path.display()))?;
        unsafe {
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes()).map_err(|e| {
                        format!("PktMonApi.dll has no {} (needs Windows 11 24H2+ / Server 2025): {e}", $name)
                    })?
                };
            }
            Ok(Api {
                initialize: sym!("PacketMonitorInitialize"),
                uninitialize: sym!("PacketMonitorUninitialize"),
                enum_data_sources: sym!("PacketMonitorEnumDataSources"),
                create_live_session: sym!("PacketMonitorCreateLiveSession"),
                close_session_handle: sym!("PacketMonitorCloseSessionHandle"),
                add_single_data_source_to_session: sym!("PacketMonitorAddSingleDataSourceToSession"),
                add_capture_constraint: sym!("PacketMonitorAddCaptureConstraint"),
                set_session_active: sym!("PacketMonitorSetSessionActive"),
                create_realtime_stream: sym!("PacketMonitorCreateRealtimeStream"),
                close_realtime_stream: sym!("PacketMonitorCloseRealtimeStream"),
                attach_output_to_session: sym!("PacketMonitorAttachOutputToSession"),
                _lib: lib,
            })
        }
    }
}

pub fn api() -> Result<&'static Api, String> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(Api::load).as_ref().map_err(|e| e.clone())
}

/// An initialized Packet Monitor handle
pub struct PacketMonitor {
    api: &'static Api,
    handle: Handle,
}

impl PacketMonitor {
    pub fn new() -> Result<Self, String> {
        let api = api()?;
        let mut handle: Handle = std::ptr::null_mut();
        check("PacketMonitorInitialize", unsafe {
            (api.initialize)(API_VERSION_1_0, std::ptr::null_mut(), &mut handle)
        })?;
        Ok(PacketMonitor { api, handle })
    }

    /// Data sources (components); the buffer owns the specifications
    pub fn data_sources(&self, kind: i32, show_hidden: bool) -> Result<DataSources, String> {
        let mut needed: usize = 0;
        let _ = unsafe {
            (self.api.enum_data_sources)(self.handle, kind, show_hidden as u8, 0, &mut needed, std::ptr::null_mut())
        };
        let mut buffer = vec![0u64; needed.max(16).div_ceil(8)];
        check("PacketMonitorEnumDataSources", unsafe {
            (self.api.enum_data_sources)(
                self.handle,
                kind,
                show_hidden as u8,
                buffer.len() * 8,
                &mut needed,
                buffer.as_mut_ptr() as *mut DataSourceList,
            )
        })?;
        Ok(DataSources { buffer })
    }

    pub fn session(&self, name: &str) -> Result<Session<'_>, String> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut session: Handle = std::ptr::null_mut();
        check("PacketMonitorCreateLiveSession", unsafe {
            (self.api.create_live_session)(self.handle, wide.as_ptr(), &mut session)
        })?;
        Ok(Session { monitor: self, handle: session, streams: Vec::new() })
    }
}

impl Drop for PacketMonitor {
    fn drop(&mut self) {
        unsafe { (self.api.uninitialize)(self.handle) };
    }
}

pub struct DataSources {
    buffer: Vec<u64>,
}

impl DataSources {
    pub fn iter(&self) -> impl Iterator<Item = &DataSourceSpecification> {
        let list = self.buffer.as_ptr() as *const DataSourceList;
        let count = unsafe { (*list).count } as usize;
        let first = unsafe { std::ptr::addr_of!((*list).sources) as *const *const DataSourceSpecification };
        (0..count).map(move |i| unsafe { &**first.add(i) })
    }
}

pub struct Session<'a> {
    monitor: &'a PacketMonitor,
    handle: Handle,
    streams: Vec<Handle>,
}

impl Session<'_> {
    pub fn add_data_source(&self, source: &DataSourceSpecification) -> Result<(), String> {
        check("PacketMonitorAddSingleDataSourceToSession", unsafe {
            (self.monitor.api.add_single_data_source_to_session)(self.handle, source)
        })
    }

    pub fn add_constraint(&self, constraint: &ProtocolConstraint) -> Result<(), String> {
        check("PacketMonitorAddCaptureConstraint", unsafe {
            (self.monitor.api.add_capture_constraint)(self.handle, constraint)
        })
    }

    /// Stream packets to `data` (called on a Packet Monitor thread)
    pub fn attach_stream(&mut self, config: &RealtimeStreamConfiguration) -> Result<(), String> {
        let mut stream: Handle = std::ptr::null_mut();
        check("PacketMonitorCreateRealtimeStream", unsafe {
            (self.monitor.api.create_realtime_stream)(self.monitor.handle, config, &mut stream)
        })?;
        self.streams.push(stream);
        check("PacketMonitorAttachOutputToSession", unsafe {
            (self.monitor.api.attach_output_to_session)(self.handle, stream)
        })
    }

    pub fn set_active(&self, active: bool) -> Result<(), String> {
        check("PacketMonitorSetSessionActive", unsafe {
            (self.monitor.api.set_session_active)(self.handle, active as u8)
        })
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.monitor.api.set_session_active)(self.handle, 0);
            for stream in &self.streams {
                (self.monitor.api.close_realtime_stream)(*stream);
            }
            (self.monitor.api.close_session_handle)(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    fn source(id: u32, secondary_id: u32, name: &str, description: &str) -> SourceInfo {
        SourceInfo { id, secondary_id, name: name.into(), description: description.into() }
    }

    // Data sources seen on the development machine (Hyper-V external switch + Wi-Fi)
    fn sources() -> Vec<SourceInfo> {
        vec![
            source(207, 207, "vmswitch.sys", "VMS ExtMp #2"),
            source(207, 25, "vmswitch.sys", "VMS ExtMp #2"),
            source(220, 220, "e2fn.sys", "Intel(R) Ethernet Controller (3) I225-V"),
            source(209, 209, "vmswitch.sys", "vEthernet (ymbihq.local)"),
            source(209, 18, "VmsProxyHNic.sys", "Host vNic #2"),
            source(218, 218, "vmswitch.sys", "Finances"),
            source(212, 212, "vmswitch.sys", "vEthernet (Default Switch)"),
            source(212, 15, "VmsProxyHNic.sys", "Host vNic"),
            source(20, 20, "Netwtw14.sys", "Intel(R) Wi-Fi 6E AX210 160MHz"),
        ]
    }

    fn interfaces() -> Vec<(String, String)> {
        [
            ("vEthernet (ymbihq.local)", "Hyper-V Virtual Ethernet Adapter #2"),
            ("vEthernet (Default Switch)", "Hyper-V Virtual Ethernet Adapter"),
            ("WiFi", "Intel(R) Wi-Fi 6E AX210 160MHz"),
        ]
        .iter()
        .map(|(n, d)| (n.to_string(), d.to_string()))
        .collect()
    }

    #[test]
    fn components_map_to_interfaces() {
        let map = component_interfaces(&sources(), &interfaces());
        // Host vNIC port and its hidden secondary ID (our own frames are reported there)
        assert_eq!(map[&209], "vEthernet (ymbihq.local)");
        assert_eq!(map[&18], "vEthernet (ymbihq.local)");
        assert_eq!(map[&212], "vEthernet (Default Switch)");
        assert_eq!(map[&15], "vEthernet (Default Switch)");
        // A NIC that is not behind a switch: matched by its adapter description
        assert_eq!(map[&20], "WiFi");
        // The physical NIC behind the switch, the switch itself and VM ports are not WinLLDP interfaces
        for other in [220, 207, 25, 218] {
            assert!(!map.contains_key(&other), "{other}");
        }
    }

    #[test]
    fn only_active_interfaces_are_mapped() {
        let map = component_interfaces(&sources(), &interfaces()[..1]);
        assert_eq!(map.len(), 2);
        assert!(component_interfaces(&sources(), &[]).is_empty());
    }

    // Offsets of the C structures (x64, natural alignment)
    #[test]
    fn layouts_match_packetmonitor_h() {
        assert_eq!(offset_of!(DataSourceSpecification, name), 4);
        assert_eq!(offset_of!(DataSourceSpecification, description), 132);
        assert_eq!(offset_of!(DataSourceSpecification, id), 388);
        assert_eq!(offset_of!(DataSourceSpecification, is_present), 400);
        assert_eq!(offset_of!(DataSourceSpecification, detail), 408);
        assert_eq!(size_of::<DataSourceSpecification>(), 424);
        assert_eq!(offset_of!(DataSourceList, sources), 8);
        assert_eq!(offset_of!(ProtocolConstraint, is_present), 128);
        assert_eq!(offset_of!(ProtocolConstraint, ether_type), 146);
        assert_eq!(offset_of!(ProtocolConstraint, ip1), 152);
        assert_eq!(offset_of!(ProtocolConstraint, encap_type), 192);
        assert_eq!(size_of::<ProtocolConstraint>(), 216);
        assert_eq!(size_of::<RealtimeStreamConfiguration>(), 32);
        assert_eq!(size_of::<StreamDataDescriptor>(), 32);
        assert_eq!(offset_of!(StreamMetadata, drop_reason), 24);
        assert_eq!(offset_of!(StreamMetadata, timestamp), 40);
        assert_eq!(size_of::<StreamMetadata>(), 48);
    }
}
