"""Convert a campaign `tokens.yaml` into a CSS `:root { ... }` block.

The agent runs this every time `tokens.yaml` is written or updated, so
the design-language one-pager and per-slide HTML can consume tokens via
CSS custom properties.

Usage:
    python -m system.server.lib.tokens_to_css path/to/tokens.yaml
    python -m system.server.lib.tokens_to_css path/to/tokens.yaml --out path/to/tokens.css

If `--out` is omitted, the CSS is written next to `tokens.yaml` at
`./preview/assets/tokens.css` (relative to the yaml file).

Schema mapping (from `library/deliverables/carousel-spec/template.md`):

- palette.{key}            -> --{key}                e.g. --bg, --accent_primary
- typography.{role}.family -> --font-{role}          e.g. --font-headline
  typography.{role}.weight -> --fw-{role}
  typography.{role}.line_height -> --lh-{role}
  typography.{role}.fallback can be a list or string (joined into the family stack)
- spacing.base_unit        -> --space-unit
  spacing.scale[i]         -> --space-{i+1}          (multiplied by base_unit, in px)
- layout.slide_width       -> --slide-w
  layout.slide_height      -> --slide-h
  layout.safe_margin       -> --slide-safe
  layout.gutter            -> --slide-gutter
- grid.columns             -> --grid-cols
  grid.gutter              -> --grid-gutter
- motifs[i].name           -> emitted only as a CSS comment for traceability
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any, Iterable

import yaml


def _quote_family(family: str | None, fallback: Any) -> str:
    """Build a CSS font-family stack from family + fallback(s)."""
    parts: list[str] = []
    if family:
        parts.append(f'"{family}"')
    if isinstance(fallback, str):
        parts.append(fallback)
    elif isinstance(fallback, Iterable):
        parts.extend(str(f) for f in fallback)
    return ", ".join(p for p in parts if p)


def _emit_var(lines: list[str], name: str, value: Any) -> None:
    if value is None or value == "":
        return
    lines.append(f"  --{name}: {value};")


def to_css(tokens: dict) -> str:
    lines: list[str] = []
    lines.append("/* Auto-generated from tokens.yaml by system/server/lib/tokens_to_css.py */")
    lines.append("/* Do not hand-edit. Edit tokens.yaml and re-run. */")
    campaign = tokens.get("campaign")
    version = tokens.get("version")
    if campaign or version:
        lines.append(f"/* campaign: {campaign}  version: {version} */")
    lines.append("")
    lines.append(":root {")

    palette = tokens.get("palette") or {}
    if palette:
        lines.append("  /* palette */")
        for key, value in palette.items():
            _emit_var(lines, str(key), value)
        lines.append("")

    typography = tokens.get("typography") or {}
    if typography:
        lines.append("  /* typography */")
        for role, spec in typography.items():
            spec = spec or {}
            family = spec.get("family")
            fallback = spec.get("fallback")
            stack = _quote_family(family, fallback)
            if stack:
                _emit_var(lines, f"font-{role}", stack)
            _emit_var(lines, f"fw-{role}", spec.get("weight"))
            _emit_var(lines, f"lh-{role}", spec.get("line_height"))
        lines.append("")

    spacing = tokens.get("spacing") or {}
    if spacing:
        lines.append("  /* spacing */")
        base = spacing.get("base_unit")
        if base is not None:
            _emit_var(lines, "space-unit", f"{base}px")
        scale = spacing.get("scale") or []
        for i, multiplier in enumerate(scale, start=1):
            try:
                px = float(multiplier) * float(base or 1)
            except (TypeError, ValueError):
                continue
            px_str = f"{int(px)}px" if px.is_integer() else f"{px}px"
            _emit_var(lines, f"space-{i}", px_str)
        lines.append("")

    layout = tokens.get("layout") or {}
    if layout:
        lines.append("  /* layout */")
        _emit_var(lines, "slide-w", _px(layout.get("slide_width")))
        _emit_var(lines, "slide-h", _px(layout.get("slide_height")))
        _emit_var(lines, "slide-safe", _px(layout.get("safe_margin")))
        _emit_var(lines, "slide-gutter", _px(layout.get("gutter")))
        lines.append("")

    grid = tokens.get("grid") or {}
    if grid:
        lines.append("  /* grid */")
        _emit_var(lines, "grid-cols", grid.get("columns"))
        _emit_var(lines, "grid-gutter", _px(grid.get("gutter")))
        lines.append("")

    motifs = tokens.get("motifs") or []
    if motifs:
        lines.append("  /* motifs (names only \u2014 enforcement lives in renderer + checks) */")
        for m in motifs:
            name = (m or {}).get("name") if isinstance(m, dict) else None
            if name:
                lines.append(f"  /* motif: {name} */")
        lines.append("")

    while lines and lines[-1] == "":
        lines.pop()
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def _px(value: Any) -> str | None:
    if value is None or value == "":
        return None
    if isinstance(value, str) and value.endswith(("px", "%", "em", "rem")):
        return value
    return f"{value}px"


def default_output_path(tokens_yaml: Path) -> Path:
    return tokens_yaml.parent / "preview" / "assets" / "tokens.css"


def convert_file(tokens_yaml: Path, out: Path | None = None) -> Path:
    data = yaml.safe_load(tokens_yaml.read_text(encoding="utf-8")) or {}
    css = to_css(data)
    out_path = out or default_output_path(tokens_yaml)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(css, encoding="utf-8")
    return out_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("tokens_yaml", help="Path to tokens.yaml")
    parser.add_argument(
        "--out",
        help="Output path for tokens.css (default: ./preview/assets/tokens.css beside tokens.yaml)",
        default=None,
    )
    args = parser.parse_args(argv)

    tokens_path = Path(args.tokens_yaml).resolve()
    if not tokens_path.exists():
        print(f"error: {tokens_path} does not exist", file=sys.stderr)
        return 2

    out_path = Path(args.out).resolve() if args.out else None
    written = convert_file(tokens_path, out_path)
    print(f"[tokens_to_css] {tokens_path} -> {written}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
