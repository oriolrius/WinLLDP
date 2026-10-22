# ADR 0001: Npcap in v2 — no bundling, native capture with Packet Monitor

- **Status**: accepted (2026-10-22); task 2 implemented (see [Implementation](#implementation))
- **Scope**: v2 (Rust, `master`). v1 keeps depending on Npcap through scapy.

## Context

WinLLDP sends and receives raw Ethernet frames (EtherType `0x88cc`, multicast `01:80:c2:00:00:0e`).
Windows has no user-mode API to *send* raw layer-2 frames, so v1 (scapy) and v2 (`src/pcap.rs`, which
loads `wpcap.dll` at run time) both rely on [Npcap](https://npcap.com/), a third-party driver that users
must install themselves. We looked at whether v2 can ship Npcap, or avoid it.

## Research

### 1. Npcap licensing: it cannot be bundled

From the [Npcap license](https://github.com/nmap/npcap/blob/master/LICENSE) and the
[OEM redistribution page](https://npcap.com/oem/redist):

- The free edition may not be redistributed or included in other software without the Nmap Project's
  permission.
- It is limited to **5 systems** (virtual machines count). Use with Nmap, Wireshark or Microsoft Defender
  for Identity does not count. **A WinLLDP deployment on more than 5 machines of an organization needs
  Npcap licenses** (this applied to v1 as well).
- The silent installer (`/S`) only exists in **Npcap OEM**, which allows bundling and unlimited installs:
  perpetual US$39,980–79,980 by company size, or US$6,980–11,980 per quarter.
- There is no general open-source exception. Free software authors may ask for redistribution rights,
  but the Nmap Project "normally recommend[s] that such authors instead ask your users to download and
  install Npcap themselves".

### 2. Receiving without Npcap: Windows Packet Monitor API

Windows 11 24H2 and Windows Server 2025 ship a documented user-mode API for Packet Monitor (`pktmon`),
the built-in packet capture tool ([PacketMonitorInitialize](https://learn.microsoft.com/en-us/windows/win32/pktmon/packetmonitor/nf-packetmonitor-packetmonitorinitialize),
[PacketMonitorCreateRealtimeStream](https://learn.microsoft.com/en-us/windows/win32/pktmon/packetmonitor/nf-packetmonitor-packetmonitorcreaterealtimestream)):

- Components: `PktMon.sys` (driver), `PktMonApi.dll`, `pktmon.exe`, all part of Windows.
- Calls, in order: `PacketMonitorInitialize` (`PACKETMONITOR_API_VERSION_1_0` = `0x00010000`) →
  `PacketMonitorCreateLiveSession` → `PacketMonitorEnumDataSources` (`showHidden = false`) →
  `PacketMonitorAddSingleDataSourceToSession` → `PacketMonitorAddCaptureConstraint` (filter) →
  `PacketMonitorCreateRealtimeStream` + `PacketMonitorAttachOutputToSession` →
  `PacketMonitorSetSessionActive`; then `PacketMonitorCloseRealtimeStream`,
  `PacketMonitorCloseSessionHandle`, `PacketMonitorUninitialize`.
- Real-time stream: 256 KB buffer (multiplier 1–10); packets truncated to 128 bytes by default, up to
  9000 (LLDPDUs can exceed 128 bytes: ours is 175, so raise it). Each `PACKETMONITOR_STREAM_DATA_DESCRIPTOR`
  gives the packet (`PacketOffset`/`PacketLength`) and metadata (`MetadataOffset`), used to map a packet
  to its adapter.
- It cannot see loopback traffic (not relevant for LLDP).
- Working example: [JamesKehr/pspkt](https://github.com/JamesKehr/pspkt) (PowerShell/C#); background:
  [In Detail: PktMon](https://gary-nebbett.blogspot.com/2021/05/pktmon.html).

Checked on the development machine (Windows 11 25H2, build 26200): `PktMonApi.dll` exports the 11
`PacketMonitor*` functions, and the Microsoft LLDP Protocol Driver (`ms_lldp`) is bound to every adapter.
The spike below confirmed that LLDP frames reach Packet Monitor without promiscuous mode.

### 3. Sending without Npcap: what Windows offers, tested

Windows has no socket type for raw layer-2 frames (raw sockets stop at IP), so sending needs an NDIS
protocol driver. Checked on the development machine (Windows 11 25H2):

- **`ndisuio.sys` (NDIS Usermode I/O Protocol)** ships with Windows, runs, and is bound to every
  adapter. Through `\\.\Ndisuio` (IOCTLs from `nuiouser.h`: `BIND_WAIT`, `QUERY_BINDING`,
  `OPEN_DEVICE`, `SET_ETHER_TYPE`) a user-mode program can open an adapter and **send an 802.1X EAPOL
  frame**, but `SET_ETHER_TYPE` only accepts `0x888E` and writing an LLDP frame (`0x88CC`) fails with
  `ERROR_INVALID_PARAMETER`: Windows restricts it to 802.1X on purpose.
- **Windows' own LLDP agent (`mslldp.sys`)** runs on client Windows too and already sends LLDP on every
  adapter (scope "nearest bridge"). WMI exposes it as `root/StandardCimv2:MSFT_NetLldpAgent`
  (`Enable()`/`Disable()`; writable `AdminStatus`, `MsgTxInterval`, `MsgTxHold`, `MsgFastTx`,
  `ReinitDelay`, `TxCreditMax`, `TxFastInit`), but its `Tlvs` are **read-only**. It advertises only
  Chassis ID (the computer name, locally assigned), Port ID (the MAC), TTL, IEEE 802.3 MAC/PHY status and
  LLDP-MED capabilities: no system name/description, Windows version or management address. On Windows
  Server, [`Enable-NetLldpAgent`](https://learn.microsoft.com/en-us/powershell/module/netlldpagent/enable-netlldpagent?view=windowsserver2022-ps)
  ([NetLldpAgent module](https://learn.microsoft.com/en-us/powershell/module/netlldpagent/?view=windowsserver2022-ps),
  Data Center Bridging feature) drives the same agent
  ([Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/378477/how-to-configure-microsoft-lldp-protocol-driver?page=3)).
- **[XDP for Windows](https://github.com/microsoft/xdp-for-windows)** (Microsoft, MIT license): AF_XDP
  sockets send and receive whole L2 frames, and "generic XDP" works on any NDIS interface. Official
  releases are production-signed and install silently (the runtime NuGet package plus `xdp-setup.ps1`;
  an MSI before v1.3), and the license allows redistribution. It is a separate driver that must be
  installed, Microsoft only tests it on Windows Server, and has no plan to ship it with Windows.
  **Validated on client Windows**, see [XDP validation](#xdp-validation).
- **Our own NDIS protocol driver** (like the WDK `ndisprot` sample, without the 802.1X restriction):
  kernel development, an EV certificate and Microsoft signing (attestation signing covers client
  Windows; Windows Server needs HLK certification), and security maintenance.
- Other inbox drivers send fixed protocols only (LLTD `lltdio.sys`/`rspndr.sys`), or inject layer-2
  frames from kernel mode only (WFP MAC-frame layers).

### 4. Other capture libraries

WinPcap (abandoned in 2013) and Win10Pcap (GPL, last release 2015) are unmaintained, and their old
drivers are likely to be blocked on Windows 11. Not an option.

## XDP validation

On the development machine (Windows 11 25H2 build 26200, HVCI on, Secure Boot off, Npcap also
installed), with `examples/xdp_spike.rs`:

- **Package**: `Microsoft.XDP-for-Windows.Runtime.x64` 1.4.0 from nuget.org (July 2026). `xdp.sys` and
  `xdp.cat` are signed by *Microsoft Windows Hardware Compatibility Publisher*, so no test signing.
  `xdp-setup.ps1 -Install xdp` (no prompts, no downloads) registers the `ms_xdp` network component with
  `netcfg`, installs `xdp.sys` and `xdpapi.dll` in System32 and performance counters; the `xdp` service
  runs and `ms_xdp` binds to every adapter. `xdp-setup.ps1 -Uninstall xdp` removes it.
- **Sending**: through `xdpapi.dll` (`XdpOpenApi` version 2 function table): `XskCreate`, a 64 KiB UMEM
  (`XSK_SOCKOPT_UMEM_REG`), TX and TX-completion rings of 8, `XskBind(ifIndex, queue 0, TX | GENERIC)`,
  `XskActivate`, then one descriptor on the TX ring and `XskNotifySocket(POKE_TX | WAIT_TX)`. On each of
  the four active adapters the descriptor completed with no invalid descriptors, and Packet Monitor saw
  the complete 195-byte LLDPDU (`system name = xdp-test`) leave the adapter (`Tx`) and cross the Hyper-V
  switch exactly like a frame sent through Npcap.
- **Coexistence**: XDP, Packet Monitor and Npcap ran at the same time without interference; no program
  (receive hook) is needed to transmit.
- **Received by another machine** (2026-10-22, with the network records in the infra-kb runbooks
  `switch.md` and `lldp-port-verification.md`): d0 is cabled to CRS125 `ether20`; nas1's `lldpd`
  (`eth1`, CRS `ether1`) recorded the frame sent through XDP on d0's **physical** NIC as a neighbor
  (`lldpcli show neighbors hidden`: chassis `50:eb:f6:2c:f2:ba`, SysName `xdp-test`, SysDescr
  `Windows 10.0.26200 AMD64`, TTL 120), within seconds of the send.
- **Hyper-V hosts: LLDP sent on a `vEthernet` adapter never reaches the wire, whatever the sender.**
  Frames sent through Npcap *or* XDP on `vEthernet (ymbihq.local)` (the host port of the external
  switch) never arrived at nas1 or at CRS `ether20`; Packet Monitor shows them crossing the switch to
  VM ports but never leaving the physical NIC (`e2fn.sys`): the Hyper-V switch, like an 802.1D bridge,
  does not forward the reserved `01:80:c2:00:00:0e` address to its uplink. Sent on the physical NIC
  under the switch (`Ethernet`, ifIndex 27, same MAC, no IP stack, so not a WinLLDP interface), both
  XDP and Npcap (`\Device\NPF_{E56200E0-…}`, SysName `npcap-test`) reached nas1. **v1 and v2 have
  this bug**: on hosts with a Hyper-V external switch, WinLLDP must send on the physical adapter that
  backs the `vEthernet` interface (same MAC; it is listed by `GetIfTable2`, not by
  `GetAdaptersAddresses`). Windows' own agent already sends from the physical NIC (nas1 sees its
  `D0` chassis, TTL 3601).
- **Not usable as receivers here**: the CRS125's CPU sniffer (`/tool sniffer quick interface=ether20`)
  sees only what the switch chip sends to the CPU, not frames switched between ports; p0 (`ether12`) and
  the UDM (`ether17`), on trunk ports, received no LLDP from any other host during the tests, only the
  CRS's own; the WSL VM on the `WSL (Hyper-V firewall)` switch received neither XDP nor Npcap frames.
  nas1 on an access port did receive.
- **Implementation notes**: `xdpapi.dll` is the deprecated path; the supported API (version 3) is
  header-only and talks to the driver directly (`NtCreateFile` on `\Device\xdpapi\xsk` with an
  `XdpOpenPacket000` extended attribute, then `NtDeviceIoControlFile`), which a Rust backend can do
  without the DLL. AF_XDP needs administrator rights, as WinLLDP already does.

## Decision

1. **Do not bundle or silently install Npcap.** WinLLDP keeps detecting it and failing with a clear
   message that points to <https://npcap.com/> (already in v2). The README documents the requirement and
   the 5-system limit of the free license.
2. **Task 2 — native capture backend (v2):** `capture` uses the Packet Monitor API when `PktMonApi.dll`
   provides it (Windows 11 24H2+, Windows Server 2025), and falls back to Npcap otherwise. Receiving
   neighbors then needs no third-party software on current Windows versions.
3. **Sending stays on Npcap.** Nothing that ships with Windows can send WinLLDP's TLVs: `ndisuio` only sends
   802.1X frames, and Windows' own LLDP agent only advertises the computer name and MAC, with fixed
   contents. Machines without Npcap still announce that basic identity through Windows' agent (enabled
   by default). XDP for Windows is the only redistributable alternative and works on client Windows
   (see [XDP validation](#xdp-validation)): it is the candidate for a second send backend, so that
   WinLLDP can run with no Npcap at all.
4. Not pursued: Npcap OEM (cost), our own NDIS driver (signing and maintenance burden), WinPcap/Win10Pcap.

## Spike results

`examples/pktmon_spike.rs` (on top of `src/pktmon.rs`, which loads `PktMonApi.dll` at run time) ran for
40 s on the development machine (Windows 11 25H2, build 26200, administrator, no Npcap in use), with
`winlldp send` run twice meanwhile:

- **It works without Npcap and without promiscuous mode**: 75 LLDP frames, from 8 real devices on the
  network (QNAP NAS ×2, MikroTik ×2, UniFi UDM Pro and U6 AP, Proxmox) plus our own frames.
- **Layouts**: the structures in `src/pktmon.rs` follow packetmonitor.h (`PACKETMONITOR_MAX_NAME_LENGTH`
  64, `PACKETMONITOR_MAX_STRING_LENGTH` 128, as in the documentation and three independent copies of the
  header); a unit test pins every offset. `PacketMonitorEnumDataSources` returns a list of *pointers* to
  specifications, which are passed as-is to `PacketMonitorAddSingleDataSourceToSession`.
- **Filter**: `PacketMonitorAddCaptureConstraint` with `IsPresent.EtherType` and `EtherType = 0x88cc` in
  host byte order streams only LLDP.
- **Truncation**: with `TruncationSize = 9000`, frames arrive complete (up to 273 bytes seen;
  `PacketLength` equals the frame length). Packet type is Ethernet (1) for all.
- **Windows sends LLDP by itself**: the Microsoft LLDP agent sends a 54-byte LLDPDU on every adapter
  (chassis ID = the computer name, locally assigned). This is the "LOCALLY_ASSIGNED" neighbor that
  appeared in the integration tests; it is also captured through Npcap.
- **One frame, several reports**: Packet Monitor reports a frame at every component it crosses. With a
  Hyper-V external switch, a frame from the network appears at the physical NIC (`e2fn.sys`, `Rx`), at
  the host vNIC port (`vmswitch.sys`, `Egress`) and at each VM port (`Egress`); our own frames appear at
  the host vNIC (`Tx`) and, on internal switches, again at the switch port (`Ingress`).
- **Component IDs**: the stream metadata's `ComponentId` may be a data source's `Id` *or* its
  `SecondaryId` (e.g. 18 = `Host vNic #2`, secondary of 209 = `vEthernet (ymbihq.local)`), so the
  mapping needs `PacketMonitorEnumDataSources(..., showHidden = true)`.
- **Mapping to Windows interfaces**: a vSwitch host port's `Description` is the interface's friendly name
  (`vEthernet (ymbihq.local)`), and a plain NIC's `Description` is the adapter description
  (`Intel(R) Wi-Fi 6E AX210 160MHz`), both available from `GetAdaptersAddresses`. Physical NICs bound to
  a vSwitch (no IP stack) are not WinLLDP interfaces, as with Npcap.

## Task 2 plan

1. ~~**Spike**~~: done, see above.
2. ~~**Backend**~~ (done): a common capture interface used by `capture.rs`, with two implementations: Packet Monitor
   (`src/pktmon.rs`, preferred when `PktMonApi.dll` provides the API, so the same executable still runs on
   older Windows) and Npcap (fallback). The Packet Monitor one:
   - one live session with every network-interface data source and the EtherType `0x88cc` constraint,
     `TruncationSize` 9000;
   - maps `ComponentId` → data source (by `Id` or `SecondaryId`, hidden ones included) → WinLLDP interface
     (friendly name, else adapter description), and drops frames on components that are not an active
     WinLLDP interface;
   - keeps both directions, like Npcap (our own frames included), and drops the duplicate report of the
     same frame on the same interface (same bytes within one second);
   - keeps the data callback short (copy the frame, hand it to a worker thread), as Microsoft recommends.
3. **`neighbors.json` unchanged**: same keys and interface names as with Npcap, so v1, v2 and both
   backends stay compatible.
4. **Diagnostics**: the capture log and `capture status` say which backend is in use.
5. **Tests**: unit tests for metadata parsing; the integration tests run against whichever backend the
   machine has. GitHub's Windows runners have no Npcap, so the build there checks that the native backend
   starts if the runner image supports Packet Monitor.
6. **Docs**: README "Requirements" (Npcap only needed for sending on Windows 11 24H2+/Server 2025) and
   "What changed in v2".

## Consequences

- On Windows 11 24H2+ and Windows Server 2025, a machine that only needs to *discover* neighbors no longer
  needs Npcap; machines that *advertise* themselves still do, with the free license's 5-system limit.
- Two capture backends to maintain; the Npcap one remains for older Windows and as a fallback.
- If the spike shows that LLDP frames do not reach Packet Monitor (e.g. without promiscuous mode), the
  decision is revisited and this ADR updated.

## Implementation

Done in `src/capture.rs` (backend selection, shared consumer, deduplication) and `src/pktmon.rs`
(bindings and `component_interfaces`), as planned above. `LLDP_CAPTURE_BACKEND` (`auto` | `pktmon` |
`npcap`) selects the backend; `auto` tries Packet Monitor, then Npcap. The capture log records
`Capture backend: ...` and `capture status` shows it. `capture start` only fails when no backend can
run, with a message naming both requirements.

Validation on the development machine, both captures running at the same time for 75 s:

- The 6 neighbors seen by both backends have the same key, interface and data.
- **Packet Monitor saw 10 more**: every real device on the network, received on
  `vEthernet (ymbihq.local)` (the host port of a Hyper-V external switch). Npcap, on that adapter, only
  saw the host's own frames, because the switch does not deliver that multicast to the host vNIC's
  Npcap handle, while Packet Monitor observes it inside the switch.
- Integration tests run the end-to-end check (send our LLDP, find it in `neighbors.json`) once per
  available backend.

The toolchain is pinned in `rust-toolchain.toml` (Rust 1.99.1), so local builds and CI use the same
compiler and clippy.
