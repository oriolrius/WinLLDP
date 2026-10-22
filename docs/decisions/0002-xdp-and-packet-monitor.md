# ADR 0002: Send with bundled XDP for Windows, capture with Packet Monitor, no Npcap

- **Status**: accepted and implemented (2026-10-22); supersedes [ADR 0001](0001-npcap.md)
- **Scope**: v2 (Rust, `master`)

## Context

v2 used Npcap to send LLDP (and as a capture fallback). Npcap cannot be bundled (free license: no
redistribution, 5 systems, no silent install; OEM from US$39,980), so every user had to install it
themselves. ADR 0001 records that research, the tests of what Windows itself offers, and the
validation of two alternatives:

- **Windows Packet Monitor API** captures LLDP without any extra software (Windows 11 24H2+, Windows
  Server 2025+), and even sees frames Npcap missed on Hyper-V hosts.
- **XDP for Windows** (Microsoft, MIT, production-signed) sends raw frames through AF_XDP; its frames
  were received by another machine across the network switch.

Requirement (from the project owner): users must not install or know about any dependency. Whatever
WinLLDP needs is part of its package or is installed transparently.

## Decision

1. **Sending: XDP for Windows, bundled.** The signed driver package (`xdp.inf`, `xdp.cat`, `xdp.sys`,
   `xdpapi.dll`, `xdppcw.man` from `Microsoft.XDP-for-Windows.Runtime.x64` 1.4.0) lives unmodified in
   `vendor/xdp-for-windows/1.4.0/` and is embedded in `winlldp.exe` (+~0.8 MB). The first time
   WinLLDP needs to send (`send`, the service) and XDP does not respond, it extracts the package to
   `%ProgramFiles%\WinLLDP\xdp-1.4.0` and installs it as `xdp-setup.ps1 -Install xdp` does:
   `netcfg -l xdp.inf -c s -i ms_xdp` plus `lodctr` for its counters. It takes about a second, with no
   prompt. An XDP that something else installed is used as is. `winlldp service uninstall` (and
   `winlldp driver uninstall`) remove XDP again (`unlodctr`, `netcfg -u ms_xdp`,
   `pnputil /delete-driver` of its driver-store copy, found by content because pnputil's output is
   localized), only when WinLLDP installed it (marker `%ProgramFiles%\WinLLDP\installed-by-winlldp`).
2. **Capturing: Windows Packet Monitor only** (`src/pktmon.rs`). On older Windows, `capture start`
   explains that capturing needs Windows 11 24H2 / Windows Server 2025 or later; sending still works.
   XDP cannot replace it: its receive rules match IP/UDP/TCP only, not an EtherType, and redirecting
   all frames would take them away from Windows' network stack.
3. **Hyper-V hosts: send on the physical NIC.** A `vEthernet` adapter on an external virtual switch
   never gets LLDP onto the wire (the switch does not forward `01:80:c2:00:00:0e` to its uplink;
   verified with Npcap and XDP alike). WinLLDP sends each interface's LLDPDU on the physical adapter
   that has the interface's MAC (`sysinfo::uplink_adapter`, from `GetIfTable2`: hardware interface,
   connector present, not an NDIS filter, up); interfaces without one (internal switches, plain NICs)
   send for themselves. Capture maps that physical NIC to the same interface (by MAC), so our own
   frames and the network's are recorded under the `vEthernet` name, as before.
4. **NSSM is bundled too** (public domain, `vendor/nssm/`): `service install` extracts it to
   `%ProgramFiles%\WinLLDP\nssm-2.24-101-g897c7ad` (the service's image path) and `service uninstall`
   removes it. Users no longer run `winget install NSSM`.
5. **`%ProgramFiles%\WinLLDP`** holds what the service runs as SYSTEM (NSSM, the driver package):
   only administrators can write there, unlike `%ProgramData%`, where a standard user could plant a
   binary first.
6. **Npcap support is removed**: `src/pcap.rs`, the Npcap capture backend and `LLDP_CAPTURE_BACKEND`.
   `winlldp driver status|install|uninstall` shows and manages the components for diagnostics; users
   never need it.

## Validation (development machine, Windows 11 25H2, HVCI on)

- Fresh state (XDP removed): `winlldp send -v` installed XDP in ~1 s, sent on `Ethernet` (ifIndex 27)
  behind `vEthernet (ymbihq.local)`, and nas1's `lldpd` (CRS125 `ether1`) recorded the full LLDPDU
  (`SysName d0`, `Windows 10.0.26200 AMD64`, `MgmtIP 10.2.0.47`) seconds later: the first time
  WinLLDP's own LLDP reached the network from this host.
- `capture start` + `send`: our frame and every LLDP device on the network appeared under
  `vEthernet (ymbihq.local)`.
- `driver uninstall` left no service, no files in System32, no `oem*.inf` and no
  `%ProgramFiles%\WinLLDP`; the next `send` reinstalled it.
- Integration tests: the usual suite, then (one at a time) the transparent install and the service
  lifecycle with the bundled NSSM, which also checks that uninstalling the service removes XDP.

## Consequences

- No third-party install step, no Npcap licensing (MIT and public-domain notices in
  `THIRD-PARTY-NOTICES.md`).
- WinLLDP installs a kernel component (Microsoft-signed) on first send; it needs administrator rights,
  as WinLLDP already did. XDP is officially tested on Windows Server 2019/2022; it was validated on
  Windows 11 25H2 here.
- Capture needs Windows 11 24H2+ / Windows Server 2025+.
- WinLLDP uses the supported **AF_XDP API version 3** and no deprecated interface: like the SDK's
  header-only implementation (`xdp/details/ioctldef.h`, `ioctlfn.h`, `afxdp.h`), it opens
  `\\Device\\xdpapi\\xsk` with `NtCreateFile` and an `XdpOpenPacket000` extended attribute (version
  1.0, API 3, object type XSK) and drives the socket with `NtDeviceIoControlFile` (`IOCTL_XSK_BIND`,
  `_ACTIVATE`, `_GET_SOCKOPT`, `_SET_SOCKOPT`, `_NOTIFY`). `xdpapi.dll`, the deprecated function-table
  API, is still installed because it is part of the signed package, but WinLLDP never loads it.
  Validated the same way: transparent install, LLDP received by nas1, all tests.
- Updating XDP or NSSM: replace the vendored files, `SHA256SUMS` and the version constants
  (`xdp::XDP_VERSION`, `service::NSSM_VERSION`); WinLLDP upgrades the XDP it installed.
