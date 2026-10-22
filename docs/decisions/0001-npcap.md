# ADR 0001: Npcap in v2 — no bundling, native capture with Packet Monitor

- **Status**: accepted (2026-10-22); native capture (task 2) not implemented yet
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
That driver subscribes to the LLDP multicast address, which should let LLDP frames reach Packet Monitor
without promiscuous mode — **to be verified**.

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

## Task 2 plan

1. **Spike**: a minimal live session with an EtherType `0x88cc` constraint on this machine. Confirm that
   LLDP frames from a switch and from `winlldp send` arrive without promiscuous mode, that 9000-byte
   truncation works, how metadata maps to adapters (friendly name / ifIndex), and which privileges it
   needs (administrator expected).
2. **Backend**: `src/pktmon.rs` loading `PktMonApi.dll` at run time (like `src/pcap.rs`, so the same
   executable runs on older Windows); a common capture interface used by `capture.rs`; Npcap as fallback.
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
