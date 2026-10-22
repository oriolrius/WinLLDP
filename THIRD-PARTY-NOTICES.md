# Third-party components in winlldp.exe

`winlldp.exe` embeds these components unmodified and installs them when they are needed.

## XDP for Windows 1.4.0

- Source: <https://github.com/microsoft/xdp-for-windows>, NuGet package
  `Microsoft.XDP-for-Windows.Runtime.x64` 1.4.0 (`vendor/xdp-for-windows/1.4.0/`)
- Files: `xdp.inf`, `xdp.cat`, `xdp.sys`, `xdpapi.dll`, `xdppcw.man`
- Use: sends LLDP frames (AF_XDP); installed to Windows' driver store on first use
- License: MIT

```
MIT License

Copyright (c) Microsoft Corporation.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE
```

## NSSM 2.24-101-g897c7ad

- Source: <https://nssm.cc/ci/nssm-2.24-101-g897c7ad.zip> (`win64/nssm.exe`, `vendor/nssm/`)
- Use: runs WinLLDP as a Windows service
- License: public domain (<https://nssm.cc/download>)
