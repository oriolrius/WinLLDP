# XDP for Windows 1.4.0 (redistributed)

Unmodified files from the NuGet package `Microsoft.XDP-for-Windows.Runtime.x64` 1.4.0
(nuget.org, published 2026-07-29; package SHA-256
`f78f091c3eda1d17d2bf4f7e0ddc593fbea1daac6a53ca69de2f1854e8070d23`), folder `runtime/native/`.
Project: <https://github.com/microsoft/xdp-for-windows> — MIT license, see `LICENSE`.

`xdp.sys` and `xdp.cat` are signed by *Microsoft Windows Hardware Compatibility Publisher*;
the catalog covers the files listed in `xdp.inf`, so none of them may be changed.
`SHA256SUMS` lists the files; `src/xdp.rs` embeds them in `winlldp.exe` and installs the
driver the way the package's `xdp-setup.ps1 -Install xdp` does. WinLLDP talks to the driver with
the AF_XDP API version 3 (IOCTLs); `xdpapi.dll` (deprecated API) is installed only because the
signed package includes it.

To update: download the new package, replace these files, update `SHA256SUMS`, this README
and `XDP_VERSION` in `src/xdp.rs`.
