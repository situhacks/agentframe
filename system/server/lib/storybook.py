"""Generate a design-language storybook (single self-contained HTML page).

A storybook is an optional, browsable visual catalog of a project's design
language, rendered deterministically from the canonical ``tokens.yaml`` plus a
little prose extracted from the ``design-language-v{N}.md`` body. Shape and
rules live in ``library/deliverables/design-language/storybook.md``.

Usage:
    python -m system.server.lib.storybook path/to/design-language/
    python -m system.server.lib.storybook path/to/design-language/ --out path/to/storybook.html

The input is the design-language *folder*. By default the output is written to
``<folder>/preview/storybook.html``.

Determinism boundary: everything under ``tokens.yaml`` is fully deterministic.
Two prose bits — the mood paragraph and (when absent from yaml) the
anti-patterns — are extracted from the highest ``design-language-v{N}.md`` by
heading. That extraction is the drift-sensitive seam, guarded by up-front
schema validation: on any missing required key or heading the tool exits
non-zero with a named error and writes NO file, rather than emitting a
half-rendered storybook.
"""

from __future__ import annotations

import argparse
import html
import re
import sys
from pathlib import Path
from typing import Any

import yaml


class SchemaError(Exception):
    """Raised when tokens.yaml or the design-language md is missing something
    the storybook renderer requires. The message names the fix."""


VERSION_RE = re.compile(r"design-language-v(\d+)\.md$", re.IGNORECASE)


# ----------------------------- discovery ---------------------------------- #

def find_tokens(folder: Path) -> Path:
    p = folder / "tokens.yaml"
    if not p.exists():
        raise SchemaError(
            f"{p} not found. A storybook needs a canonical tokens.yaml in the "
            f"design-language folder. Create it or hand-author the storybook "
            f"per library/deliverables/design-language/storybook.md."
        )
    return p


def find_design_language(folder: Path) -> Path:
    """Return the highest-versioned design-language-v{N}.md."""
    candidates: list[tuple[int, Path]] = []
    for f in folder.glob("design-language-v*.md"):
        m = VERSION_RE.search(f.name)
        if m:
            candidates.append((int(m.group(1)), f))
    if not candidates:
        raise SchemaError(
            f"No design-language-v{{N}}.md found in {folder}. The storybook "
            f"extracts its mood paragraph from that file. Add one (or migrate a "
            f"legacy -vF.md to -v1.md) or hand-author the storybook."
        )
    candidates.sort()
    return candidates[-1][1]


# ----------------------------- validation --------------------------------- #

def validate_tokens(tokens: dict) -> None:
    for key in ("palette", "type", "canvas"):
        if not tokens.get(key):
            raise SchemaError(
                f"tokens.yaml is missing required top-level key '{key}'. "
                f"See the canonical schema in system/server/lib/tokens_to_css.py. "
                f"Fix the schema or hand-author the storybook."
            )
    type_block = tokens["type"]
    role_specs = [
        (r, s) for r, s in type_block.items() if isinstance(s, dict)
    ]
    if not role_specs:
        raise SchemaError(
            "tokens.yaml 'type' has no role objects with a 'family'. Each type "
            "role (e.g. display/body/mono) needs a 'family'. Fix or hand-author."
        )
    for role, spec in role_specs:
        if not spec.get("family"):
            raise SchemaError(
                f"tokens.yaml type role '{role}' has no 'family'. Add a font "
                f"family or remove the role. Fix or hand-author."
            )


def extract_mood(md_text: str) -> str:
    """Pull the mood paragraph from the design-language body.

    Accepts either a bold '**One-line:**' lead or the first prose paragraph
    after the H1 title. Raises if neither is found.
    """
    one_line = re.search(r"\*\*One-line:\*\*\s*(.+)", md_text)
    if one_line:
        return one_line.group(1).strip()
    # Fallback: first non-empty, non-heading, non-frontmatter paragraph.
    body = re.sub(r"^---\n.*?\n---\n", "", md_text, count=1, flags=re.DOTALL)
    for block in re.split(r"\n\s*\n", body):
        line = block.strip()
        if not line or line.startswith("#") or line.startswith(">"):
            continue
        return line
    raise SchemaError(
        "Could not extract a mood paragraph from the design-language md: no "
        "'**One-line:**' lead and no leading prose paragraph. Add one or "
        "hand-author the storybook."
    )


# ----------------------------- rendering ---------------------------------- #

def _esc(v: Any) -> str:
    return html.escape(str(v), quote=True)


def _px(v: Any) -> str:
    if v is None or v == "":
        return ""
    if isinstance(v, str) and v.endswith(("px", "%", "em", "rem")):
        return v
    return f"{v}px"


def _swatch(token: str, hex_val: str) -> str:
    return f"""      <figure class="swatch">
        <div class="chip" style="background: var(--{_esc(token)});"></div>
        <figcaption>
          <code class="tok">--{_esc(token)}</code>
          <span class="hex">{_esc(hex_val)}</span>
        </figcaption>
      </figure>"""


def _role_specimen(role: str, spec: dict) -> str:
    family = spec.get("family", "")
    weights = spec.get("weights") or []
    weights_str = ", ".join(str(w) for w in weights) if weights else "—"
    role_desc = _esc(spec.get("role", ""))
    sizes = spec.get("sizes") or {}
    biggest = None
    if isinstance(sizes, dict) and sizes:
        try:
            biggest = max(
                sizes.values(),
                key=lambda s: float(re.sub(r"[^0-9.]", "", str(s)) or 0),
            )
        except ValueError:
            biggest = next(iter(sizes.values()))
    sample_size = _px(biggest) if biggest is not None else "32px"
    size_rows = ""
    if isinstance(sizes, dict):
        for name, val in sizes.items():
            size_rows += (
                f'        <div class="size-row">'
                f'<span class="size-label">{_esc(name)}</span>'
                f'<span style="font-family: var(--font-{_esc(role)}); '
                f'font-size: {_px(val)};">Ag</span>'
                f'<span class="size-val">{_esc(_px(val))}</span></div>\n'
            )
    return f"""    <div class="type-role">
      <div class="type-meta">
        <span class="role-name">{_esc(role)}</span>
        <span class="role-family">{_esc(family)}</span>
        <span class="role-weights">weights: {_esc(weights_str)}</span>
        {f'<span class="role-desc">{role_desc}</span>' if role_desc else ''}
      </div>
      <div class="type-sample" style="font-family: var(--font-{_esc(role)}); font-size: {sample_size};">
        The quick brown fox
      </div>
      <div class="size-list">
{size_rows}      </div>
    </div>"""


def _emphasis_block(emphasis: dict) -> str:
    if not isinstance(emphasis, dict) or not emphasis:
        return '    <p class="none">None for this project.</p>'
    out = ""
    for name, spec in emphasis.items():
        rule = ""
        if isinstance(spec, dict):
            rule = spec.get("rule", "")
        out += f"""    <div class="emphasis-device">
      <span class="device-name">{_esc(name)}</span>
      <span class="device-rule">{_esc(rule)}</span>
    </div>\n"""
    return out


def _sample_gallery(samples: list, folder: Path, out_dir: Path) -> str:
    if not samples:
        return '    <p class="none">No samples embedded for this language.</p>'
    out = ""
    for rel in samples:
        # Resolve relative to the design-language folder, then re-relativize to out dir.
        src = (folder / rel).resolve()
        try:
            href = src.relative_to(out_dir.resolve()).as_posix()
        except ValueError:
            # Different subtree — use a ../-style relative path.
            import os
            href = os.path.relpath(src, out_dir).replace("\\", "/")
        ext = src.suffix.lower()
        if ext in {".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg"}:
            out += f'    <figure class="sample"><img src="{_esc(href)}" alt="{_esc(rel)}"><figcaption>{_esc(rel)}</figcaption></figure>\n'
        else:  # html / other -> iframe
            out += f'    <figure class="sample"><iframe src="{_esc(href)}" loading="lazy"></iframe><figcaption>{_esc(rel)}</figcaption></figure>\n'
    return out


def _safe_zone_diagram(canvas: dict) -> str:
    w = canvas.get("width") or 1080
    h = canvas.get("height") or 1350
    safe = canvas.get("safe_margin") or 80
    aspect = canvas.get("aspect") or f"{w}:{h}"
    # Scaled preview box keeping aspect; safe inset shown as inner dashed box.
    try:
        ratio = float(h) / float(w)
    except (TypeError, ValueError, ZeroDivisionError):
        ratio = 1.25
    box_w = 220
    box_h = int(box_w * ratio)
    try:
        inset_pct = (float(safe) / float(w)) * 100
    except (TypeError, ValueError, ZeroDivisionError):
        inset_pct = 7
    return f"""    <div class="canvas-diagram">
      <div class="canvas-box" style="width:{box_w}px; height:{box_h}px;">
        <div class="safe-box" style="inset:{inset_pct:.1f}%;"></div>
        <span class="canvas-dims">{_esc(w)}×{_esc(h)} · {_esc(aspect)}</span>
      </div>
      <p class="canvas-note">Dashed inner box = safe area ({_esc(_px(safe))} margin).</p>
    </div>"""


PAGE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Design Storybook</title>
{fonts_link}
<link rel="stylesheet" href="assets/tokens.css">
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{
    background: var(--role-background, var(--base-white, #fff));
    color: var(--role-foreground, var(--ink, #111));
    font-family: var(--font-{body_role}, system-ui, sans-serif);
    line-height: 1.5;
    padding: 56px clamp(24px, 6vw, 96px);
    max-width: 1100px;
    margin: 0 auto;
  }}
  header.book {{ margin-bottom: 56px; }}
  .kicker {{
    font-family: var(--font-{mono_role}, monospace);
    font-size: 11px; letter-spacing: 0.16em; text-transform: uppercase;
    color: var(--role-secondary, var(--meta, #666));
  }}
  h1 {{ font-size: clamp(40px, 7vw, 72px); letter-spacing: -0.03em; margin: 10px 0 6px; }}
  .summary {{ font-size: 18px; color: var(--role-secondary, var(--meta, #666)); }}
  .mood {{ font-size: 20px; max-width: 60ch; margin-top: 20px; }}
  section {{ margin: 48px 0; }}
  section > h2 {{
    font-family: var(--font-{mono_role}, monospace);
    font-size: 12px; letter-spacing: 0.16em; text-transform: uppercase;
    color: var(--role-secondary, var(--meta, #666));
    border-bottom: 1px solid var(--accent-modifier, #ccc);
    padding-bottom: 8px; margin-bottom: 24px;
  }}
  .swatch-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 16px; }}
  .swatch .chip {{ height: 72px; border-radius: 6px; border: 1px solid rgba(0,0,0,0.08); }}
  .swatch figcaption {{ margin-top: 8px; display: flex; flex-direction: column; gap: 2px; }}
  .swatch .tok {{ font-family: var(--font-{mono_role}, monospace); font-size: 12px; }}
  .swatch .hex {{ font-size: 12px; color: var(--role-secondary, #666); text-transform: uppercase; }}
  .rules {{ margin-top: 20px; }}
  .rules li {{ margin-left: 20px; font-size: 15px; }}
  .type-role {{ padding: 20px 0; border-bottom: 1px solid var(--surface-legend, #f0f0f0); }}
  .type-meta {{ display: flex; flex-wrap: wrap; gap: 14px; align-items: baseline; margin-bottom: 12px; }}
  .role-name {{ font-family: var(--font-{mono_role}, monospace); font-size: 12px; letter-spacing: 0.12em; text-transform: uppercase; }}
  .role-family {{ font-weight: 600; }}
  .role-weights, .role-desc {{ font-size: 13px; color: var(--role-secondary, #666); }}
  .type-sample {{ line-height: 1.05; margin-bottom: 14px; }}
  .size-list {{ display: flex; flex-direction: column; gap: 6px; }}
  .size-row {{ display: grid; grid-template-columns: 120px 1fr 60px; align-items: baseline; gap: 12px; }}
  .size-label, .size-val {{ font-family: var(--font-{mono_role}, monospace); font-size: 11px; color: var(--role-secondary, #666); }}
  .emphasis-device {{ display: flex; gap: 16px; padding: 10px 0; align-items: baseline; }}
  .device-name {{ font-family: var(--font-{mono_role}, monospace); font-size: 12px; letter-spacing: 0.1em; text-transform: uppercase; min-width: 120px; }}
  .device-rule {{ font-size: 15px; color: var(--role-body, var(--ink-soft, #222)); }}
  .canvas-box {{ position: relative; background: var(--surface-legend, #f0f0f0); border: 1px solid var(--accent-modifier, #ccc); }}
  .safe-box {{ position: absolute; border: 1px dashed var(--accent-teal, #87c7c0); }}
  .canvas-dims {{ position: absolute; bottom: 6px; left: 8px; font-family: var(--font-{mono_role}, monospace); font-size: 10px; color: var(--role-secondary, #666); }}
  .canvas-note {{ font-size: 13px; color: var(--role-secondary, #666); margin-top: 10px; }}
  .comp-rules li {{ margin-left: 20px; font-size: 15px; margin-top: 4px; }}
  .sample-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 20px; }}
  .sample img, .sample iframe {{ width: 100%; border: 1px solid var(--accent-modifier, #ccc); border-radius: 4px; }}
  .sample iframe {{ height: 320px; }}
  .sample figcaption {{ font-family: var(--font-{mono_role}, monospace); font-size: 11px; color: var(--role-secondary, #666); margin-top: 6px; }}
  .anti li {{ margin-left: 20px; font-size: 15px; margin-top: 4px; }}
  .none {{ color: var(--role-secondary, #666); font-style: italic; }}
  footer.book {{ margin-top: 64px; padding-top: 16px; border-top: 1px solid var(--accent-modifier, #ccc);
    font-family: var(--font-{mono_role}, monospace); font-size: 11px; color: var(--role-secondary, #666); }}
</style>
</head>
<body>
<header class="book">
  {category_line}
  <h1>{title}</h1>
  <p class="summary">{summary}</p>
  <p class="mood">{mood}</p>
</header>

<section id="palette">
  <h2>Palette</h2>
  <div class="swatch-grid">
{swatches}
  </div>
  {palette_rules}
</section>

<section id="type">
  <h2>Type</h2>
{type_specimens}
</section>

<section id="emphasis">
  <h2>Emphasis Devices</h2>
{emphasis}
</section>

<section id="composition">
  <h2>Composition</h2>
{canvas_diagram}
  {comp_rules}
</section>

<section id="samples">
  <h2>Sample Gallery</h2>
  <div class="sample-grid">
{samples}
  </div>
</section>

<section id="anti-patterns">
  <h2>Anti-patterns</h2>
{anti_patterns}
</section>

<footer class="book">Generated from tokens.yaml v{version} by system/server/lib/storybook.py · do not hand-edit; edit the design language and regenerate.</footer>
</body>
</html>
"""


def _pick_role(type_block: dict, *candidates: str) -> str:
    roles = [r for r, s in type_block.items() if isinstance(s, dict)]
    for c in candidates:
        if c in roles:
            return c
    return roles[0] if roles else "body"


def render(folder: Path, out_dir: Path, tokens: dict, md_text: str, samples: list) -> str:
    meta = tokens.get("meta") or {}
    palette = tokens.get("palette") or {}
    type_block = tokens.get("type") or {}
    canvas = tokens.get("canvas") or {}
    emphasis = tokens.get("emphasis") or {}
    composition = tokens.get("composition") or {}

    title = str(meta.get("campaign", folder.parent.parent.name)).replace("-", " ").title()
    summary = meta.get("summary", "")
    category = meta.get("category", "")
    version = meta.get("version", "1")
    mood = extract_mood(md_text)

    body_role = _pick_role(type_block, "body", "primary")
    mono_role = _pick_role(type_block, "mono")

    swatches = "\n".join(
        _swatch(tok, hexval) for tok, hexval in palette.items()
    )

    # Palette rules: from md accent rules block if present; else empty.
    palette_rules = ""

    type_specimens = "\n".join(
        _role_specimen(role, spec)
        for role, spec in type_block.items()
        if isinstance(spec, dict)
    )

    canvas_diagram = _safe_zone_diagram(canvas)

    comp_rules = ""
    rules = composition.get("rules") if isinstance(composition, dict) else None
    if rules:
        items = "".join(f"    <li>{_esc(r)}</li>\n" for r in rules)
        comp_rules = f'<ul class="comp-rules">\n{items}  </ul>'

    anti = tokens.get("anti_patterns") or []
    if anti:
        items = "".join(f"    <li>{_esc(a)}</li>\n" for a in anti)
        anti_html = f'<ul class="anti">\n{items}  </ul>'
    else:
        anti_html = '    <p class="none">None recorded for this project.</p>'

    fonts_link = ""
    gf = type_block.get("google_fonts_import")
    if gf:
        fonts_link = (
            '<link rel="preconnect" href="https://fonts.googleapis.com">\n'
            '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n'
            f'<link href="{_esc(gf)}" rel="stylesheet">'
        )

    category_line = f'<p class="kicker">{_esc(category)}</p>' if category else '<p class="kicker">Design Language</p>'

    return PAGE.format(
        title=_esc(title),
        summary=_esc(summary),
        mood=_esc(mood),
        category_line=category_line,
        version=_esc(version),
        fonts_link=fonts_link,
        body_role=_esc(body_role),
        mono_role=_esc(mono_role),
        swatches=swatches,
        palette_rules=palette_rules,
        type_specimens=type_specimens,
        emphasis=_emphasis_block(emphasis),
        canvas_diagram=canvas_diagram,
        comp_rules=comp_rules,
        samples=_sample_gallery(samples, folder, out_dir),
        anti_patterns=anti_html,
    )


def _load_samples(md_text: str) -> list:
    """Read storybook_samples from the md frontmatter (list of relative paths)."""
    fm = re.match(r"^---\n(.*?)\n---\n", md_text, re.DOTALL)
    if not fm:
        return []
    try:
        data = yaml.safe_load(fm.group(1)) or {}
    except yaml.YAMLError:
        return []
    samples = data.get("storybook_samples") or []
    return [s for s in samples if isinstance(s, str)]


def generate(folder: Path, out: Path | None = None) -> Path:
    folder = folder.resolve()
    tokens_path = find_tokens(folder)
    md_path = find_design_language(folder)

    tokens = yaml.safe_load(tokens_path.read_text(encoding="utf-8")) or {}
    validate_tokens(tokens)

    md_text = md_path.read_text(encoding="utf-8")
    samples = _load_samples(md_text)

    out_path = out or (folder / "preview" / "storybook.html")
    out_path = out_path.resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    html_out = render(folder, out_path.parent, tokens, md_text, samples)
    out_path.write_text(html_out, encoding="utf-8")
    return out_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("folder", help="Path to the design-language folder")
    parser.add_argument("--out", default=None, help="Output HTML path (default: <folder>/preview/storybook.html)")
    args = parser.parse_args(argv)

    folder = Path(args.folder)
    if not folder.is_dir():
        print(f"error: {folder} is not a directory", file=sys.stderr)
        return 2

    out = Path(args.out) if args.out else None
    try:
        written = generate(folder, out)
    except SchemaError as e:
        print(f"[storybook] SCHEMA ERROR: {e}", file=sys.stderr)
        return 3
    print(f"[storybook] {folder} -> {written}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
