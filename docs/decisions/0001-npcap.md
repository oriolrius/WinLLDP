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

### 3. Sending without Npcap: not possible from user mode

- Windows' own LLDP agent (MSLLDP) sends LLDP. On Windows Server it is controlled with
  [`Enable-NetLldpAgent`](https://learn.microsoft.com/en-us/powershell/module/netlldpagent/enable-netlldpagent?view=windowsserver2022-ps)
  ([NetLldpAgent module](https://learn.microsoft.com/en-us/powershell/module/netlldpagent/?view=windowsserver2022-ps)),
  which needs the Data Center Bridging feature. It only advertises basic TLVs (computer name / MAC), and
  its contents cannot be customized
  ([Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/378477/how-to-configure-microsoft-lldp-protocol-driver?page=3)):
  none of WinLLDP's TLVs (Windows version/build, management address, LLDP-MED).
- Our own NDIS protocol driver would work but needs kernel development, an EV certificate, Microsoft
  attestation signing and ongoing security maintenance.

### 4. Other capture libraries

WinPcap (abandoned in 2013) and Win10Pcap (GPL, last release 2015) are unmaintained, and their old
drivers are likely to be blocked on Windows 11. Not an option.

## Decision

1. **Do not bundle or silently install Npcap.** WinLLDP keeps detecting it and failing with a clear
   message that points to <https://npcap.com/> (already in v2). The README documents the requirement and
   the 5-system limit of the free license.
2. **Task 2 — native capture backend (v2):** `capture` uses the Packet Monitor API when `PktMonApi.dll`
   provides it (Windows 11 24H2+, Windows Server 2025), and falls back to Npcap otherwise. Receiving
   neighbors then needs no third-party software on current Windows versions.
3. **Sending stays on Npcap.** There is no user-mode alternative that can send WinLLDP's TLVs. Users who
   cannot install Npcap can still advertise a basic LLDP identity with Windows Server's
   `Enable-NetLldpAgent`, documented as a workaround.
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
