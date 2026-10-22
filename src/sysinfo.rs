//! System information: network interfaces, hostname and Windows version

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Friendly name, e.g. "Ethernet 2"
    pub name: String,
    /// Adapter GUID, e.g. "{0F2B...}": the Npcap device is \Device\NPF_{GUID}
    pub adapter_name: String,
    /// Adapter description, e.g. "Intel(R) Wi-Fi 6E AX210 160MHz"
    pub description: String,
    pub mac: Option<String>,
    pub is_up: bool,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub if_index: u32,
}

impl Interface {
    pub fn npcap_device(&self) -> String {
        format!(r"\Device\NPF_{}", self.adapter_name)
    }
}

/// Npcap (or the older WinPcap) is needed to send and capture raw Ethernet frames
pub fn npcap_installed() -> bool {
    let system32 = system32();
    system32.join("Npcap").join("wpcap.dll").exists() || system32.join("wpcap.dll").exists()
}

pub const NPCAP_URL: &str = "https://npcap.com/";

pub fn npcap_missing_message() -> String {
    format!("Npcap is not installed; it is required for packet capture. Install it from {NPCAP_URL}")
}

pub fn system32() -> std::path::PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    std::path::PathBuf::from(root).join("System32")
}

/// MAC address text (any separator) to bytes
pub fn mac_to_bytes(mac: &str) -> Option<Vec<u8>> {
    let hex: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() % 2 != 0 || hex.len() != mac.chars().filter(|c| *c != ':' && *c != '-').count() {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect()
}

/// IP address text to its address family (IANA) and bytes
pub fn ip_to_bytes(ip: &str) -> Option<(u8, Vec<u8>)> {
    match ip.trim().parse::<IpAddr>().ok()? {
        IpAddr::V4(v4) => Some((crate::packet::ADDRESS_FAMILY_IPV4, v4.octets().to_vec())),
        IpAddr::V6(v6) => Some((crate::packet::ADDRESS_FAMILY_IPV6, v6.octets().to_vec())),
    }
}

/// The IP address used to reach the internet (no packet is sent)
pub fn primary_ip() -> Option<String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(("8.8.8.8", 80)).ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// Bit 7 (Station Only) for both supported and enabled capabilities
pub fn system_capabilities() -> (u16, u16) {
    (0x80, 0x80)
}

pub fn interface_by_name(name: &str) -> Option<Interface> {
    interfaces().into_iter().find(|i| i.name == name)
}

/// Interface index (ifIndex) of an interface, or its position in the list when unknown
pub fn interface_index(name: &str) -> u32 {
    let list = interfaces();
    match list.iter().position(|i| i.name == name) {
        Some(pos) if list[pos].if_index != 0 => list[pos].if_index,
        Some(pos) => pos as u32 + 1,
        None => 0,
    }
}

/// e.g. "Windows 10.0.26100 AMD64"
pub fn system_description() -> String {
    format!("Windows {} {}", os_version(), machine())
}

/// Processor architecture as reported by Windows, e.g. "AMD64"
pub fn machine() -> String {
    std::env::var("PROCESSOR_ARCHITEW6432").or_else(|_| std::env::var("PROCESSOR_ARCHITECTURE")).unwrap_or_default()
}

fn format_ipv6(addr: Ipv6Addr) -> String {
    addr.to_string()
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH};
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};

    pub fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn from_wide_ptr(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }

    unsafe fn from_cstr_ptr(p: *const u8) -> String {
        if p.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(p as *const std::ffi::c_char).to_string_lossy().into_owned()
    }

    /// Adapters with a hardware address, in the order Windows reports them
    pub fn interfaces() -> Vec<Interface> {
        let mut size: u32 = 16 * 1024;
        let mut buffer: Vec<u64> = Vec::new();
        for _ in 0..4 {
            buffer = vec![0u64; (size as usize).div_ceil(8)];
            let rc = unsafe {
                GetAdaptersAddresses(
                    AF_UNSPEC as u32,
                    0,
                    std::ptr::null(),
                    buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                    &mut size,
                )
            };
            if rc == ERROR_SUCCESS {
                break;
            }
            if rc != ERROR_BUFFER_OVERFLOW {
                return Vec::new();
            }
        }

        let mut result = Vec::new();
        let mut adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !adapter.is_null() {
            let a = unsafe { &*adapter };
            adapter = a.Next;
            let mac_len = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());
            if mac_len == 0 {
                continue;
            }
            let mac = crate::packet::format_mac(&a.PhysicalAddress[..mac_len]);
            let mut ipv4 = Vec::new();
            let mut ipv6 = Vec::new();
            let mut unicast = a.FirstUnicastAddress;
            while !unicast.is_null() {
                let u = unsafe { &*unicast };
                unicast = u.Next;
                let sockaddr = u.Address.lpSockaddr;
                if sockaddr.is_null() {
                    continue;
                }
                unsafe {
                    match (*sockaddr).sa_family {
                        AF_INET => {
                            let sin = &*(sockaddr as *const SOCKADDR_IN);
                            ipv4.push(Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr)).to_string());
                        }
                        AF_INET6 => {
                            let sin6 = &*(sockaddr as *const SOCKADDR_IN6);
                            ipv6.push(format_ipv6(Ipv6Addr::from(sin6.sin6_addr.u.Byte)));
                        }
                        _ => {}
                    }
                }
            }
            result.push(Interface {
                name: unsafe { from_wide_ptr(a.FriendlyName) },
                adapter_name: unsafe { from_cstr_ptr(a.AdapterName) },
                description: unsafe { from_wide_ptr(a.Description) },
                mac: Some(mac),
                is_up: a.OperStatus == IfOperStatusUp,
                ipv4,
                ipv6,
                if_index: unsafe { a.Anonymous1.Anonymous.IfIndex },
            });
        }
        result
    }

    pub fn hostname() -> String {
        let mut size: u32 = 256;
        let mut buf = vec![0u16; size as usize];
        let ok = unsafe { GetComputerNameExW(ComputerNameDnsHostname, buf.as_mut_ptr(), &mut size) };
        if ok == 0 {
            return std::env::var("COMPUTERNAME").unwrap_or_default();
        }
        String::from_utf16_lossy(&buf[..size as usize])
    }

    const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    pub fn registry_string(subkey: &str, value: &str) -> Option<String> {
        let (subkey, value) = (wide(subkey), wide(value));
        let mut size: u32 = 0;
        let rc = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                null_mut(),
                &mut size,
            )
        };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        let rc = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                buf.as_mut_ptr() as *mut _,
                &mut size,
            )
        };
        if rc != ERROR_SUCCESS {
            return None;
        }
        Some(unsafe { from_wide_ptr(buf.as_ptr()) })
    }

    fn registry_dword(subkey: &str, value: &str) -> Option<u32> {
        let (subkey, value) = (wide(subkey), wide(value));
        let mut data: u32 = 0;
        let mut size: u32 = 4;
        let rc = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_DWORD,
                null_mut(),
                &mut data as *mut u32 as *mut _,
                &mut size,
            )
        };
        (rc == ERROR_SUCCESS).then_some(data)
    }

    fn build_number() -> Option<u32> {
        registry_string(CURRENT_VERSION, "CurrentBuildNumber")?.trim().parse().ok()
    }

    /// e.g. "10.0.26100" (major.minor.build)
    pub fn os_version() -> String {
        let major = registry_dword(CURRENT_VERSION, "CurrentMajorVersionNumber").unwrap_or(10);
        let minor = registry_dword(CURRENT_VERSION, "CurrentMinorVersionNumber").unwrap_or(0);
        match build_number() {
            Some(build) => format!("{major}.{minor}.{build}"),
            None => format!("{major}.{minor}"),
        }
    }

    /// Windows edition as WMI's Win32_OperatingSystem.Caption reports it, e.g. "Microsoft Windows 11 Pro"
    pub fn os_caption() -> Option<String> {
        let product = registry_string(CURRENT_VERSION, "ProductName")?;
        // Windows 11 still says "Windows 10" in ProductName; builds 22000+ are Windows 11
        let product = match build_number() {
            Some(build) if build >= 22000 => product.replacen("Windows 10", "Windows 11", 1),
            _ => product,
        };
        Some(format!("Microsoft {product}"))
    }
}

#[cfg(windows)]
pub use win::{hostname, interfaces, os_caption, os_version, registry_string, wide};

#[cfg(not(windows))]
mod fallback {
    use super::Interface;

    pub fn interfaces() -> Vec<Interface> {
        Vec::new()
    }

    pub fn hostname() -> String {
        std::env::var("HOSTNAME").unwrap_or_default()
    }

    pub fn os_version() -> String {
        String::new()
    }

    pub fn os_caption() -> Option<String> {
        None
    }
}

#[cfg(not(windows))]
pub use fallback::{hostname, interfaces, os_caption, os_version};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_conversion() {
        assert_eq!(mac_to_bytes("00:11:22:aa:BB:cc"), Some(vec![0, 0x11, 0x22, 0xaa, 0xbb, 0xcc]));
        assert_eq!(mac_to_bytes("00-11-22-aa-bb-cc"), Some(vec![0, 0x11, 0x22, 0xaa, 0xbb, 0xcc]));
        assert_eq!(mac_to_bytes("zz:11"), None);
    }

    #[test]
    fn ip_conversion() {
        assert_eq!(ip_to_bytes("10.2.0.47"), Some((1, vec![10, 2, 0, 47])));
        assert_eq!(ip_to_bytes("fe80::1").map(|(f, b)| (f, b.len())), Some((2, 16)));
        assert_eq!(ip_to_bytes("not-an-ip"), None);
    }

    #[test]
    fn capabilities_are_station_only() {
        assert_eq!(system_capabilities(), (0x80, 0x80));
    }

    #[cfg(windows)]
    #[test]
    fn windows_information() {
        assert!(os_version().starts_with("10.0."));
        assert!(os_caption().unwrap().starts_with("Microsoft Windows"));
        assert!(!hostname().is_empty());
        assert!(system_description().starts_with("Windows 10.0."));
        for i in interfaces() {
            assert!(i.adapter_name.starts_with('{'), "{i:?}");
            assert!(i.mac.is_some());
        }
    }
}
