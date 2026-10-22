//! Spike: send an LLDP frame with XDP for Windows (AF_XDP, generic mode), without Npcap.
//! Needs XDP for Windows installed (xdp-setup.ps1 -Install xdp) and administrator rights:
//!   xdp_spike ["Interface name"] [ifIndex to bind instead]

use std::ffi::c_void;
use std::sync::atomic::{fence, AtomicU32, Ordering};
use winlldp::{config::Config, pcap, sender, sysinfo};

type Handle = *mut c_void;

#[repr(C)]
struct XdpApiTable {
    xdp_open_api: *const c_void,
    xdp_close_api: *const c_void,
    xdp_get_routine: *const c_void,
    xdp_create_program: *const c_void,
    xdp_interface_open: *const c_void,
    xsk_create: unsafe extern "system" fn(*mut Handle) -> i32,
    xsk_bind: unsafe extern "system" fn(Handle, u32, u32, u32) -> i32,
    xsk_activate: unsafe extern "system" fn(Handle, u32) -> i32,
    xsk_notify_socket: unsafe extern "system" fn(Handle, u32, u32, *mut u32) -> i32,
    xsk_notify_async: *const c_void,
    xsk_get_notify_async_result: *const c_void,
    xsk_set_sockopt: unsafe extern "system" fn(Handle, u32, *const c_void, u32) -> i32,
    xsk_get_sockopt: unsafe extern "system" fn(Handle, u32, *mut c_void, *mut u32) -> i32,
    xsk_ioctl: *const c_void,
}

#[repr(C)]
struct UmemReg {
    total_size: u64,
    chunk_size: u32,
    headroom: u32,
    address: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct RingInfo {
    ring: usize,
    descriptors_offset: u32,
    producer_index_offset: u32,
    consumer_index_offset: u32,
    flags_offset: u32,
    size: u32,
    element_stride: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct RingInfoSet {
    fill: RingInfo,
    completion: RingInfo,
    rx: RingInfo,
    tx: RingInfo,
}

const XSK_SOCKOPT_UMEM_REG: u32 = 1;
const XSK_SOCKOPT_TX_RING_SIZE: u32 = 4;
const XSK_SOCKOPT_TX_COMPLETION_RING_SIZE: u32 = 5;
const XSK_SOCKOPT_RING_INFO: u32 = 6;
const XSK_SOCKOPT_STATISTICS: u32 = 7;
const XSK_BIND_FLAG_TX: u32 = 0x2;
const XSK_BIND_FLAG_GENERIC: u32 = 0x4;
const XSK_NOTIFY_FLAG_POKE_TX: u32 = 0x2;
const XSK_NOTIFY_FLAG_WAIT_TX: u32 = 0x8;
const XSK_RING_FLAG_NEED_POKE: u32 = 0x2;

fn check(what: &str, hr: i32) {
    println!("{what}: {}", if hr >= 0 { "OK".to_string() } else { format!("HRESULT 0x{:08X}", hr as u32) });
    if hr < 0 {
        std::process::exit(1);
    }
}

unsafe fn ring_u32(info: &RingInfo, offset: u32) -> &AtomicU32 {
    &*((info.ring + offset as usize) as *const AtomicU32)
}

fn main() {
    let wanted = std::env::args().nth(1).unwrap_or("vEthernet (ymbihq.local)".into());
    // Optional ifIndex to bind to instead, e.g. a physical NIC behind a Hyper-V switch
    // (no IP stack, so not a WinLLDP interface) that shares the named interface's MAC
    let bind_index: Option<u32> = std::env::args().nth(2).and_then(|a| a.parse().ok());
    let mut iface = sysinfo::interface_by_name(&wanted).expect("interface not found");
    if let Some(index) = bind_index {
        iface.if_index = index;
    }
    println!("== {} (ifIndex {}, {})", iface.name, iface.if_index, iface.mac.as_deref().unwrap_or("?"));

    let lib = unsafe { libloading::Library::new(sysinfo::system32().join("xdpapi.dll")) }
        .or_else(|_| unsafe { libloading::Library::new("xdpapi.dll") })
        .expect("xdpapi.dll not found: is XDP for Windows installed?");
    let open: libloading::Symbol<unsafe extern "system" fn(u32, *mut *const XdpApiTable) -> i32> =
        unsafe { lib.get(b"XdpOpenApi\0") }.unwrap();
    let mut table: *const XdpApiTable = std::ptr::null();
    check("XdpOpenApi(2)", unsafe { open(2, &mut table) });
    let api = unsafe { &*table };

    let mut sock: Handle = std::ptr::null_mut();
    check("XskCreate", unsafe { (api.xsk_create)(&mut sock) });

    // UMEM: 16 chunks of 4 KiB
    let chunk = 4096u32;
    let mut umem = vec![0u8; 16 * chunk as usize];
    let reg = UmemReg {
        total_size: umem.len() as u64,
        chunk_size: chunk,
        headroom: 0,
        address: umem.as_mut_ptr() as *mut c_void,
    };
    let set = |name: &str, opt: u32, value: *const c_void, len: usize| {
        check(name, unsafe { (api.xsk_set_sockopt)(sock, opt, value, len as u32) });
    };
    set("UMEM_REG", XSK_SOCKOPT_UMEM_REG, &reg as *const _ as *const c_void, std::mem::size_of::<UmemReg>());
    let ring_size = 8u32;
    set("TX_RING_SIZE", XSK_SOCKOPT_TX_RING_SIZE, &ring_size as *const _ as *const c_void, 4);
    set("TX_COMPLETION_RING_SIZE", XSK_SOCKOPT_TX_COMPLETION_RING_SIZE, &ring_size as *const _ as *const c_void, 4);
    check("XskBind(TX|GENERIC, queue 0)", unsafe {
        (api.xsk_bind)(sock, iface.if_index, 0, XSK_BIND_FLAG_TX | XSK_BIND_FLAG_GENERIC)
    });
    check("XskActivate", unsafe { (api.xsk_activate)(sock, 0) });

    let mut rings = RingInfoSet::default();
    let mut len = std::mem::size_of::<RingInfoSet>() as u32;
    check("RING_INFO", unsafe {
        (api.xsk_get_sockopt)(sock, XSK_SOCKOPT_RING_INFO, &mut rings as *mut _ as *mut c_void, &mut len)
    });
    println!("tx ring: {:?}", rings.tx);
    println!("completion ring: {:?}", rings.completion);

    // The LLDP frame, with a recognizable system name
    let mut config = Config::load(None).unwrap();
    config.system_name = "xdp-test".into();
    let local = sender::LocalSystem::detect();
    let data = sender::create_lldp_packet(&config, &local, &iface, false).unwrap();
    let src = sysinfo::mac_to_bytes(iface.mac.as_deref().unwrap()).unwrap();
    let frame = pcap::ethernet_frame(&pcap::LLDP_MULTICAST_MAC, &src, pcap::LLDP_ETHERTYPE, &data);
    umem[..frame.len()].copy_from_slice(&frame);

    unsafe {
        let tx = &rings.tx;
        let producer = ring_u32(tx, tx.producer_index_offset);
        let index = producer.load(Ordering::Relaxed);
        let slot = (index & (tx.size - 1)) as usize;
        let descriptor = (tx.ring + tx.descriptors_offset as usize + slot * tx.element_stride as usize) as *mut u64;
        // XSK_BUFFER_DESCRIPTOR { Address (base 0, offset 0), Length, Reserved }
        descriptor.write(0);
        (descriptor.add(1) as *mut u32).write(frame.len() as u32);
        (descriptor.add(1) as *mut u32).add(1).write(0);
        fence(Ordering::Release);
        producer.store(index + 1, Ordering::Release);
        let flags = ring_u32(tx, tx.flags_offset).load(Ordering::Acquire);
        println!("produced 1 TX descriptor ({} bytes), ring flags 0x{flags:x}", frame.len());

        let mut result = 0u32;
        let mut notify = XSK_NOTIFY_FLAG_WAIT_TX;
        if flags & XSK_RING_FLAG_NEED_POKE != 0 {
            notify |= XSK_NOTIFY_FLAG_POKE_TX;
        }
        let hr = (api.xsk_notify_socket)(sock, notify | XSK_NOTIFY_FLAG_POKE_TX, 2000, &mut result);
        println!("XskNotifySocket: hr=0x{:08X} result=0x{result:x}", hr as u32);

        let completion = &rings.completion;
        let done = ring_u32(completion, completion.producer_index_offset).load(Ordering::Acquire)
            - ring_u32(completion, completion.consumer_index_offset).load(Ordering::Acquire);
        println!("completed TX descriptors: {done}");
    }

    let mut stats = [0u64; 4];
    let mut len = 32u32;
    unsafe { (api.xsk_get_sockopt)(sock, XSK_SOCKOPT_STATISTICS, stats.as_mut_ptr() as *mut c_void, &mut len) };
    println!("statistics (rx dropped, rx truncated, rx invalid, tx invalid): {stats:?}");
}
