# NSSM 2.24-101-g897c7ad (redistributed)

`nssm.exe` from `win64/` of <https://nssm.cc/ci/nssm-2.24-101-g897c7ad.zip>, the build the
Windows Package Manager installs as `NSSM.NSSM` (archive SHA-256
`99f5045fffbffb745d67fe3a065a953c4a3d9c253b868892d9b685b0ee7d07b8`, as in its winget manifest).
NSSM is public domain (<https://nssm.cc/download>).

`src/service.rs` embeds it in `winlldp.exe` and extracts it to `%ProgramFiles%\WinLLDP\` to run
the WinLLDP service, so users do not install NSSM themselves.
