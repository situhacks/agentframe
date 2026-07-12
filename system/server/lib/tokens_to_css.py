"""Convert a project `tokens.yaml` into a CSS `:root { ... }` block.

The agent runs this every time `tokens.yaml` is written or updated, so
the design-language one-pager and per-slide HTML can consume tokens via
CSS custom properties.

Usage:
    python -m system.server.lib.tokens_to_css path/to/tokens.yaml
    python -m system.server.lib.tokens_to_css path/to/tokens.yaml --out path/to/tokens.css

If `--out` is omitted, the CSS is written next to `tokens.yaml` at
`./preview/assets/tokens.css` (relative to the yaml file).

Canonical schema mapping (as used by real project `tokens.yaml` files; see
`library/deliverables/design-language/storybook.md` for the reader contract):

- palette.{key}            -> --{key}                e.g. --base-white, --accent-teal
- palette_roles.{role}     -> --role-{role}          value is a palette token key
                                                     e.g. palette_roles.background: base-white
                                                     -> --role-background: var(--base-white)
- type.{role}.family       -> --font-{role}          role keys are project-defined
                                                     (display/body/mono or primary/annotation/mono)
  type.{role}.weights[0]   -> --fw-{role}            first weight as the default
  type.{role}.line_height  -> --lh-{role}
  type.{role}.sizes.{name} -> --size-{role}-{name}   (px unless already a unit)
  type.google_fonts_import is ignored here (belongs in the HTML <link>)
- canvas.width             -> --canvas-w
  canvas.height            -> --canvas-h
  canvas.aspect            -> --canvas-aspect
  canvas.safe_margin       -> --canvas-safe
  canvas.grid_columns      -> --grid-cols
  canvas.grid_gutter       -> --grid-gutter

Project-specific sections (emphasis, highlighter, nano-banana, etc.) are not
mapped to CSS variables; they are consumed by renderers/storybook prose, not
by tokens.css.
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
    meta = tokens.get("meta") or {}
    project = meta.get("campaign") or tokens.get("project")
    version = meta.get("version") or tokens.get("version")
    if project or version:
        lines.append(f"/* project: {project}  version: {version} */")
    lines.append("")
    lines.append(":root {")

    palette = tokens.get("palette") or {}
    if palette:
        lines.append("  /* palette */")
        for key, value in palette.items():
            _emit_var(lines, str(key), value)
        lines.append("")

    palette_roles = tokens.get("palette_roles") or {}
    if palette_roles:
        lines.append("  /* palette roles (semantic -> palette token) */")
        for role, token_key in palette_roles.items():
            if token_key is None or token_key == "":
                continue
            # Skip role values that are lists (e.g. risograph_spots) \u2014 not single vars.
            if isinstance(token_key, (list, tuple, dict)):
                continue
            _emit_var(lines, f"role-{role}", f"var(--{token_key})")
        lines.append("")

    type_block = tokens.get("type") or {}
    if type_block:
        lines.append("  /* type */")
        for role, spec in type_block.items():
            if not isinstance(spec, dict):
                continue  # skips scalars like google_fonts_import
            family = spec.get("family")
            fallback = spec.get("fallback")
            stack = _quote_family(family, fallback)
            if stack:
                _emit_var(lines, f"font-{role}", stack)
            weights = spec.get("weights")
            if isinstance(weights, (list, tuple)) and weights:
                _emit_var(lines, f"fw-{role}", weights[0])
            else:
                _emit_var(lines, f"fw-{role}", spec.get("weight"))
            _emit_var(lines, f"lh-{role}", spec.get("line_height"))
            sizes = spec.get("sizes") or {}
            if isinstance(sizes, dict):
                for name, value in sizes.items():
                    _emit_var(lines, f"size-{role}-{name}", _px(value))
        lines.append("")

    canvas = tokens.get("canvas") or {}
    if canvas:
        lines.append("  /* canvas */")
        _emit_var(lines, "canvas-w", _px(canvas.get("width")))
        _emit_var(lines, "canvas-h", _px(canvas.get("height")))
        _emit_var(lines, "canvas-aspect", canvas.get("aspect"))
        _emit_var(lines, "canvas-safe", _px(canvas.get("safe_margin")))
        _emit_var(lines, "grid-cols", canvas.get("grid_columns"))
        _emit_var(lines, "grid-gutter", _px(canvas.get("grid_gutter")))
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
