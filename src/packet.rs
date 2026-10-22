//! LLDP packet (LLDPDU) construction and parsing: IEEE 802.1AB TLVs

use serde_json::{Map, Value};
use std::net::{Ipv4Addr, Ipv6Addr};

pub const END_OF_LLDPDU: u8 = 0;
pub const CHASSIS_ID: u8 = 1;
pub const PORT_ID: u8 = 2;
pub const TTL: u8 = 3;
pub const PORT_DESCRIPTION: u8 = 4;
pub const SYSTEM_NAME: u8 = 5;
pub const SYSTEM_DESCRIPTION: u8 = 6;
pub const SYSTEM_CAPABILITIES: u8 = 7;
pub const MANAGEMENT_ADDRESS: u8 = 8;
pub const ORGANIZATIONALLY_SPECIFIC: u8 = 127;

/// Chassis ID subtype 4 and Port ID subtype 3 carry a MAC address
pub const CHASSIS_ID_MAC_ADDRESS: u8 = 4;
pub const PORT_ID_MAC_ADDRESS: u8 = 3;

/// IANA address family numbers used in the Management Address TLV
pub const ADDRESS_FAMILY_IPV4: u8 = 1;
pub const ADDRESS_FAMILY_IPV6: u8 = 2;

/// TLV values are at most 511 bytes (9-bit length)
pub const MAX_TLV_LENGTH: usize = 511;

const CHASSIS_ID_SUBTYPES: [&str; 7] = [
    "CHASSIS_COMPONENT",
    "INTERFACE_ALIAS",
    "PORT_COMPONENT",
    "MAC_ADDRESS",
    "NETWORK_ADDRESS",
    "INTERFACE_NAME",
    "LOCALLY_ASSIGNED",
];

const PORT_ID_SUBTYPES: [&str; 7] = [
    "INTERFACE_ALIAS",
    "PORT_COMPONENT",
    "MAC_ADDRESS",
    "NETWORK_ADDRESS",
    "INTERFACE_NAME",
    "AGENT_CIRCUIT_ID",
    "LOCALLY_ASSIGNED",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tlv {
    pub tlv_type: u8,
    pub value: Vec<u8>,
}

impl Tlv {
    pub fn new(tlv_type: u8, value: Vec<u8>) -> Self {
        Tlv { tlv_type, value }
    }

    /// 7-bit type and 9-bit length header, then the value
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        if self.value.len() > MAX_TLV_LENGTH {
            return Err(format!("TLV length {} exceeds maximum of {MAX_TLV_LENGTH}", self.value.len()));
        }
        let header = ((self.tlv_type as u16 & 0x7F) << 9) | self.value.len() as u16;
        let mut out = header.to_be_bytes().to_vec();
        out.extend_from_slice(&self.value);
        Ok(out)
    }

    /// Decode the TLV at `offset`; returns it and the offset of the next one,
    /// or None when the data is truncated
    pub fn decode(data: &[u8], offset: usize) -> Option<(Tlv, usize)> {
        let header = data.get(offset..offset + 2)?;
        let header = u16::from_be_bytes([header[0], header[1]]);
        let tlv_type = ((header >> 9) & 0x7F) as u8;
        let length = (header & 0x1FF) as usize;
        let value = data.get(offset + 2..offset + 2 + length)?;
        Some((Tlv::new(tlv_type, value.to_vec()), offset + 2 + length))
    }
}

#[derive(Debug, Default, Clone)]
pub struct LldpPacket {
    pub tlvs: Vec<Tlv>,
}

impl LldpPacket {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_tlv(&mut self, tlv: Tlv) {
        self.tlvs.push(tlv);
    }

    pub fn add_chassis_id(&mut self, subtype: u8, value: &[u8]) {
        self.add_tlv(Tlv::new(CHASSIS_ID, [&[subtype], value].concat()));
    }

    pub fn add_port_id(&mut self, subtype: u8, value: &[u8]) {
        self.add_tlv(Tlv::new(PORT_ID, [&[subtype], value].concat()));
    }

    pub fn add_ttl(&mut self, seconds: u16) {
        self.add_tlv(Tlv::new(TTL, seconds.to_be_bytes().to_vec()));
    }

    pub fn add_port_description(&mut self, description: &str) {
        self.add_tlv(Tlv::new(PORT_DESCRIPTION, description.as_bytes().to_vec()));
    }

    pub fn add_system_name(&mut self, name: &str) {
        self.add_tlv(Tlv::new(SYSTEM_NAME, name.as_bytes().to_vec()));
    }

    pub fn add_system_description(&mut self, description: &str) {
        self.add_tlv(Tlv::new(SYSTEM_DESCRIPTION, description.as_bytes().to_vec()));
    }

    pub fn add_system_capabilities(&mut self, capabilities: u16, enabled: u16) {
        let value = [capabilities.to_be_bytes(), enabled.to_be_bytes()].concat();
        self.add_tlv(Tlv::new(SYSTEM_CAPABILITIES, value));
    }

    /// Management address with ifIndex interface numbering and an optional OID
    pub fn add_management_address(&mut self, address_family: u8, address: &[u8], interface_number: u32, oid: &[u8]) {
        let mut value = vec![(address.len() + 1) as u8, address_family];
        value.extend_from_slice(address);
        value.push(2); // interface numbering subtype: ifIndex
        value.extend_from_slice(&interface_number.to_be_bytes());
        value.push(oid.len() as u8);
        value.extend_from_slice(oid);
        self.add_tlv(Tlv::new(MANAGEMENT_ADDRESS, value));
    }

    pub fn add_organizationally_specific(&mut self, oui: [u8; 3], subtype: u8, info: &[u8]) {
        let value = [&oui[..], &[subtype], info].concat();
        self.add_tlv(Tlv::new(ORGANIZATIONALLY_SPECIFIC, value));
    }

    pub fn add_end_of_lldpdu(&mut self) {
        self.add_tlv(Tlv::new(END_OF_LLDPDU, Vec::new()));
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for tlv in &self.tlvs {
            out.extend(tlv.encode()?);
        }
        Ok(out)
    }

    /// Decode TLVs until End of LLDPDU or truncated data
    pub fn decode(data: &[u8]) -> Self {
        let mut packet = Self::new();
        let mut offset = 0;
        while offset < data.len() {
            let Some((tlv, next)) = Tlv::decode(data, offset) else {
                break;
            };
            let end = tlv.tlv_type == END_OF_LLDPDU;
            packet.add_tlv(tlv);
            offset = next;
            if end {
                break;
            }
        }
        packet
    }

    pub fn get_tlv_value(&self, tlv_type: u8) -> Option<&[u8]> {
        self.tlvs.iter().find(|t| t.tlv_type == tlv_type).map(|t| t.value.as_slice())
    }

    /// The neighbor fields stored in neighbors.json (same keys and order as v1)
    pub fn to_dict(&self) -> Map<String, Value> {
        let mut result = Map::new();
        let mut addresses: Vec<String> = Vec::new();

        for tlv in &self.tlvs {
            let v = &tlv.value;
            match tlv.tlv_type {
                CHASSIS_ID if !v.is_empty() => {
                    let id = if v[0] == CHASSIS_ID_MAC_ADDRESS { format_mac(&v[1..]) } else { decode_utf8(&v[1..]) };
                    result.insert("chassis_id".into(), id.into());
                    result.insert("chassis_id_subtype".into(), subtype_name(&CHASSIS_ID_SUBTYPES, v[0]).into());
                }
                PORT_ID if !v.is_empty() => {
                    let id = if v[0] == PORT_ID_MAC_ADDRESS { format_mac(&v[1..]) } else { decode_utf8(&v[1..]) };
                    result.insert("port_id".into(), id.into());
                    result.insert("port_id_subtype".into(), subtype_name(&PORT_ID_SUBTYPES, v[0]).into());
                }
                TTL if v.len() == 2 => {
                    result.insert("ttl".into(), u16::from_be_bytes([v[0], v[1]]).into());
                }
                PORT_DESCRIPTION => {
                    result.insert("port_description".into(), decode_utf8(v).into());
                }
                SYSTEM_NAME => {
                    result.insert("system_name".into(), decode_utf8(v).into());
                }
                SYSTEM_DESCRIPTION => {
                    result.insert("system_description".into(), decode_utf8(v).into());
                }
                SYSTEM_CAPABILITIES if v.len() == 4 => {
                    result.insert("system_capabilities".into(), u16::from_be_bytes([v[0], v[1]]).into());
                    result.insert("enabled_capabilities".into(), u16::from_be_bytes([v[2], v[3]]).into());
                }
                MANAGEMENT_ADDRESS => {
                    if let Some(address) = format_management_address(v) {
                        addresses.push(address);
                        result.insert(
                            "management_addresses".into(),
                            Value::Array(addresses.iter().cloned().map(Value::from).collect()),
                        );
                    }
                }
                _ => {}
            }
        }

        // A neighbor may advertise several management addresses (e.g. IPv4 and IPv6):
        // management_address is the first IPv4 one, or the first one if there is no IPv4
        if let Some(first) = addresses.iter().find(|a| is_ipv4(a)).or(addresses.first()) {
            result.insert("management_address".into(), first.clone().into());
        }
        result
    }
}

fn subtype_name(names: &[&str], subtype: u8) -> String {
    match names.get((subtype as usize).wrapping_sub(1)) {
        Some(name) => name.to_string(),
        None => format!("UNKNOWN_{subtype}"),
    }
}

/// Lower-case colon-separated hex, e.g. aa:bb:cc:dd:ee:ff
pub fn format_mac(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
}

/// UTF-8 text, dropping invalid bytes
pub fn decode_utf8(bytes: &[u8]) -> String {
    bytes.utf8_chunks().map(|chunk| chunk.valid()).collect()
}

/// Format the address of a Management Address TLV value (IPv4/IPv6 in standard notation)
pub fn format_management_address(value: &[u8]) -> Option<String> {
    if value.len() < 2 {
        return None;
    }
    let addr_len = value[0] as usize; // address subtype + address
    let family = value[1];
    let end = (1 + addr_len).min(value.len()).max(2);
    let address = &value[2..end];
    if family == ADDRESS_FAMILY_IPV4 && address.len() == 4 {
        return Some(Ipv4Addr::new(address[0], address[1], address[2], address[3]).to_string());
    }
    if family == ADDRESS_FAMILY_IPV6 && address.len() == 16 {
        let octets: [u8; 16] = address.try_into().ok()?;
        return Some(Ipv6Addr::from(octets).to_string());
    }
    let formatted = format_mac(address);
    (!formatted.is_empty()).then_some(formatted)
}

pub fn is_ipv4(address: &str) -> bool {
    address.parse::<Ipv4Addr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_packet() -> LldpPacket {
        let mut p = LldpPacket::new();
        p.add_chassis_id(CHASSIS_ID_MAC_ADDRESS, &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        p.add_port_id(5, b"Ethernet 2");
        p.add_ttl(120);
        p.add_port_description("Uplink");
        p.add_system_name("pc-01");
        p.add_system_description("Windows 11");
        p.add_system_capabilities(0x0080, 0x0080);
        p
    }

    fn decode_with(addresses: &[(u8, Vec<u8>)]) -> Map<String, Value> {
        let mut p = sample_packet();
        for (family, raw) in addresses {
            p.add_management_address(*family, raw, 3, b"");
        }
        p.add_end_of_lldpdu();
        LldpPacket::decode(&p.encode().unwrap()).to_dict()
    }

    #[test]
    fn header_is_7_bit_type_and_9_bit_length() {
        let encoded = Tlv::new(SYSTEM_NAME, vec![b'x'; 300]).encode().unwrap();
        let header = u16::from_be_bytes([encoded[0], encoded[1]]);
        assert_eq!(header >> 9, SYSTEM_NAME as u16);
        assert_eq!(header & 0x1FF, 300);
        assert_eq!(encoded.len(), 302);
    }

    #[test]
    fn tlv_longer_than_511_bytes_is_rejected() {
        assert!(Tlv::new(SYSTEM_NAME, vec![0; 512]).encode().is_err());
    }

    #[test]
    fn decode_round_trip() {
        let (tlv, offset) = Tlv::decode(&Tlv::new(PORT_DESCRIPTION, b"abc".to_vec()).encode().unwrap(), 0).unwrap();
        assert_eq!((tlv.tlv_type, tlv.value.as_slice(), offset), (PORT_DESCRIPTION, &b"abc"[..], 5));
    }

    #[test]
    fn truncated_tlv_is_not_decoded() {
        assert!(Tlv::decode(&[0x0A, 0x05, b'a'], 0).is_none());
        assert!(Tlv::decode(&[0x0A], 0).is_none());
    }

    #[test]
    fn encode_decode_to_dict() {
        let mut p = sample_packet();
        p.add_end_of_lldpdu();
        let info = LldpPacket::decode(&p.encode().unwrap()).to_dict();
        assert_eq!(info["chassis_id"], "00:11:22:33:44:55");
        assert_eq!(info["chassis_id_subtype"], "MAC_ADDRESS");
        assert_eq!(info["port_id"], "Ethernet 2");
        assert_eq!(info["port_id_subtype"], "INTERFACE_NAME");
        assert_eq!(info["ttl"], 120);
        assert_eq!(info["port_description"], "Uplink");
        assert_eq!(info["system_name"], "pc-01");
        assert_eq!(info["system_description"], "Windows 11");
        assert_eq!(info["system_capabilities"], 0x80);
        assert_eq!(info["enabled_capabilities"], 0x80);
        let keys: Vec<&String> = info.keys().collect();
        assert_eq!(
            keys,
            [
                "chassis_id",
                "chassis_id_subtype",
                "port_id",
                "port_id_subtype",
                "ttl",
                "port_description",
                "system_name",
                "system_description",
                "system_capabilities",
                "enabled_capabilities"
            ]
        );
    }

    #[test]
    fn decoding_stops_at_end_of_lldpdu() {
        let mut p = sample_packet();
        p.add_end_of_lldpdu();
        let mut data = p.encode().unwrap();
        data.extend(Tlv::new(SYSTEM_NAME, b"garbage".to_vec()).encode().unwrap());
        assert_eq!(LldpPacket::decode(&data).to_dict()["system_name"], "pc-01");
    }

    #[test]
    fn mac_port_id_and_invalid_utf8() {
        let mut p = LldpPacket::new();
        p.add_port_id(PORT_ID_MAC_ADDRESS, &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        p.add_system_name_bytes(b"ab\xffcd");
        let info = LldpPacket::decode(&p.encode().unwrap()).to_dict();
        assert_eq!(info["port_id"], "aa:bb:cc:dd:ee:ff");
        assert_eq!(info["port_id_subtype"], "MAC_ADDRESS");
        assert_eq!(info["system_name"], "abcd");
    }

    #[test]
    fn ipv4_management_address() {
        let info = decode_with(&[(1, vec![192, 168, 10, 1])]);
        assert_eq!(info["management_address"], "192.168.10.1");
        assert_eq!(info["management_addresses"], serde_json::json!(["192.168.10.1"]));
    }

    #[test]
    fn ipv6_in_standard_notation() {
        let fe80: Ipv6Addr = "fe80::1".parse().unwrap();
        let info = decode_with(&[(2, fe80.octets().to_vec())]);
        assert_eq!(info["management_address"], "fe80::1");
    }

    #[test]
    fn several_addresses_prefer_ipv4() {
        let v6: Ipv6Addr = "2001:db8::10".parse().unwrap();
        let info = decode_with(&[(2, v6.octets().to_vec()), (1, vec![10, 0, 0, 2])]);
        assert_eq!(info["management_addresses"], serde_json::json!(["2001:db8::10", "10.0.0.2"]));
        assert_eq!(info["management_address"], "10.0.0.2");
    }

    #[test]
    fn no_management_address() {
        assert!(!decode_with(&[]).contains_key("management_address"));
    }

    #[test]
    fn other_address_families_and_malformed_values() {
        let mac = [&[7u8, 6][..], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]].concat();
        assert_eq!(format_management_address(&mac).as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(format_management_address(&[5]), None);
    }

    #[test]
    fn management_address_encoding() {
        let mut p = LldpPacket::new();
        p.add_management_address(1, &[10, 2, 0, 47], 11, b"");
        assert_eq!(p.encode().unwrap(), hex("100c05010a02002f020000000b00"));
    }

    #[test]
    fn matches_v1_wire_format() {
        // Captured from v1 (`send -v`, Raw LLDP data) on vEthernet (ymbihq.local)
        let mut p = LldpPacket::new();
        let mac = [0x50, 0xeb, 0xf6, 0x2c, 0xf2, 0xba];
        p.add_chassis_id(CHASSIS_ID_MAC_ADDRESS, &mac);
        p.add_port_id(PORT_ID_MAC_ADDRESS, &mac);
        p.add_ttl(120);
        p.add_port_description("Ethernet Port");
        p.add_system_name("d0");
        p.add_system_description("Windows 10.0.26200 AMD64");
        let med = [0x00, 0x12, 0xBB];
        p.add_organizationally_specific(med, 1, &[0x00, 0x01, 0x03, 0x00]);
        p.add_organizationally_specific(med, 5, b"AMD64");
        p.add_organizationally_specific(med, 6, b"10.0.26200");
        p.add_organizationally_specific(med, 7, b"Python 3.11.13");
        p.add_system_capabilities(0x80, 0x80);
        p.add_management_address(1, &[10, 2, 0, 47], 11, b"");
        p.add_end_of_lldpdu();
        assert_eq!(
            p.encode().unwrap(),
            hex(concat!(
                "02070450ebf62cf2ba04070350ebf62cf2ba06020078080d45746865726e657420506f72740a0264300c18",
                "57696e646f77732031302e302e323632303020414d443634fe080012bb0100010300fe090012bb05414d44",
                "3634fe0e0012bb0631302e302e3236323030fe120012bb07507974686f6e20332e31312e31330e04008000",
                "80100c05010a02002f020000000b000000"
            ))
        );
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    impl LldpPacket {
        fn add_system_name_bytes(&mut self, name: &[u8]) {
            self.add_tlv(Tlv::new(SYSTEM_NAME, name.to_vec()));
        }
    }
}
