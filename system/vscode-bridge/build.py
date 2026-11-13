"""Build the af-bridge .vsix without npm: a zip with extension/..., extension.vsixmanifest and [Content_Types].xml.

Output: system/logs/panel-bridge/af-bridge-<version>.vsix in the MAIN checkout (the vault root, never a worktree),
or in the folder named by --out (the tests build into a temp folder).
"""
import argparse
import json
import re
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent


def vault_root() -> Path:
    p = HERE.parent.parent
    s = str(p)
    m = re.search(r"[\\/]\.claude[\\/]worktrees[\\/][^\\/]+$", s)
    if m:
        p = Path(s[: m.start()])
    return p


def xml_escape(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Build the af-bridge .vsix without npm.")
    ap.add_argument("--out", default=None, help="the folder to write the .vsix into")
    ns = ap.parse_args(argv)
    pkg = json.loads((HERE / "package.json").read_text(encoding="utf-8"))
    version = pkg["version"]
    publisher = pkg["publisher"]
    name = pkg["name"]
    engine = pkg["engines"]["vscode"]
    kind = ",".join(pkg.get("extensionKind", ["workspace"]))

    content_types = (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension=".json" ContentType="application/json"/>'
        '<Default Extension=".js" ContentType="application/javascript"/>'
        '<Default Extension=".md" ContentType="text/markdown"/>'
        '<Default Extension=".vsixmanifest" ContentType="text/xml"/>'
        "</Types>\n"
    )
    manifest = (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011" '
        'xmlns:d="http://schemas.microsoft.com/developer/vsx-schema-design/2011">\n'
        "  <Metadata>\n"
        f'    <Identity Language="en-US" Id="{name}" Version="{version}" Publisher="{publisher}" />\n'
        f'    <DisplayName>{xml_escape(pkg["displayName"])}</DisplayName>\n'
        f'    <Description xml:space="preserve">{xml_escape(pkg["description"])}</Description>\n'
        "    <Tags></Tags>\n"
        "    <Categories>Other</Categories>\n"
        "    <GalleryFlags>Public</GalleryFlags>\n"
        "    <Properties>\n"
        f'      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="{xml_escape(engine)}" />\n'
        '      <Property Id="Microsoft.VisualStudio.Code.ExtensionDependencies" Value="" />\n'
        '      <Property Id="Microsoft.VisualStudio.Code.ExtensionPack" Value="" />\n'
        f'      <Property Id="Microsoft.VisualStudio.Code.ExtensionKind" Value="{kind}" />\n'
        '      <Property Id="Microsoft.VisualStudio.Code.LocalizedLanguages" Value="" />\n'
        '      <Property Id="Microsoft.VisualStudio.Code.EnabledApiProposals" Value="" />\n'
        '      <Property Id="Microsoft.VisualStudio.Code.ExecutesCode" Value="true" />\n'
        "    </Properties>\n"
        "  </Metadata>\n"
        '  <Installation>\n    <InstallationTarget Id="Microsoft.VisualStudio.Code" />\n  </Installation>\n'
        "  <Dependencies />\n"
        "  <Assets>\n"
        '    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true" />\n'
        '    <Asset Type="Microsoft.VisualStudio.Services.Content.Details" Path="extension/README.md" Addressable="true" />\n'
        "  </Assets>\n"
        "</PackageManifest>\n"
    )

    out_dir = Path(ns.out) if ns.out else vault_root() / "system" / "logs" / "panel-bridge"
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / f"{name}-{version}.vsix"
    if out.exists():
        out.unlink()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", content_types)
        z.writestr("extension.vsixmanifest", manifest)
        for fname in ("package.json", "extension.js", "README.md"):
            z.write(HERE / fname, "extension/" + fname)
    print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
