# D2 vendor record

- Upstream: <https://github.com/terrastruct/d2>
- Vendored runtime: Windows AMD64 CLI binary at `bin/windows-amd64/d2.exe`
- Current version and archive hash: [`d2-version.json`](d2-version.json)
- License: [`LICENSE.txt`](LICENSE.txt) (MPL-2.0)

## Refresh procedure

1. Check the upstream release notes for breaking rendering or CLI changes.
2. Run `powershell -ExecutionPolicy Bypass -File system/tools/d2/update.ps1 -Version <version>`.
3. Render the skill's sample to SVG and inspect that the file is non-empty.
4. Update any AgentFrame instructions affected by a breaking change and append a `vendor_update` audit row.

`update.ps1` downloads the official release archive, replaces only the pinned Windows binary and license, and records its SHA-256 in `d2-version.json`. It never silently selects a latest version.

The AgentFrame wrapper deliberately exposes SVG only. This keeps the capability dependency-free and avoids D2's optional browser-backed raster/PDF/GIF exporters. Add a conversion path only when a real deliverable needs one.
