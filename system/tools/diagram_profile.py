#!/usr/bin/env python3
"""Project an AgentFrame design language into a Diagram Design client profile.

Diagram Design resolves brand from `~/.diagram-design/profiles/<slug>.md` plus
an optional `.diagram-design` project marker. AgentFrame already carries the
same facts in `library/assets/design-languages/<name>/tokens.yaml`, so the
profile library is treated as a **derived cache**: the repo is truth, the home
directory is rebuildable, and nothing about a design language is maintained in
two places.

    python system/tools/diagram_profile.py editorial-deloitte-digital
    python system/tools/diagram_profile.py editorial-deloitte-digital \
        --marker workspace/projects/<project>

Without this projection a diagram authored for a branded deck arrives in the
shipped default skin: white-smoke paper and atomic-tangerine accent, in Geist,
next to slides in the language's own palette and typeface.
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - dependency is present in this repo
    print("error: pyyaml is required (pip install pyyaml)", file=sys.stderr)
    raise SystemExit(1)

ROOT = Path(__file__).resolve().parent.parent.parent
LANGUAGES = ROOT / "library" / "assets" / "design-languages"
SHIPPED_GUIDE = ROOT / "system" / "skills" / "diagram-design" / "references" / "style-guide.md"
PROFILE_LIBRARY = Path.home() / ".diagram-design" / "profiles"
SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")

# Diagram Design semantic role -> how it is derived from a design language.
# `grounds.light` / `grounds.dark` win when the language declares them, because
# a language with several grounds has already made the light/dark decision.
ROLE_SOURCES = {
    "paper": ("bg", "background"),
    "ink": ("text", "foreground"),
    "muted": ("muted", "secondary"),
    "rule-solid": ("divider", "divider"),
    "accent": ("accent", "accent"),
}


class ProjectionError(Exception):
    """A language cannot be projected without inventing brand values."""


def resolve_palette(tokens: dict) -> dict[str, str]:
    palette = tokens.get("palette") or {}
    return {k: str(v) for k, v in palette.items() if isinstance(v, (str, int, float))}


def lookup(palette: dict[str, str], key) -> str | None:
    """A design language names colours by palette key; accept a literal too."""
    if not isinstance(key, str):
        return None
    if key in palette:
        return palette[key]
    if re.fullmatch(r"#[0-9a-fA-F]{3,8}", key):
        return key
    return None


def hex_to_rgb(value: str) -> tuple[int, int, int]:
    digits = value.lstrip("#")
    if len(digits) == 3:
        digits = "".join(c * 2 for c in digits)
    return int(digits[0:2], 16), int(digits[2:4], 16), int(digits[4:6], 16)


def rgba(value: str, alpha: float) -> str:
    r, g, b = hex_to_rgb(value)
    return f"rgba({r},{g},{b},{alpha:g})"


def mix(a: str, b: str, ratio: float) -> str:
    """Blend two hex colours. Used only for roles a language does not name."""
    ra, ga, ba = hex_to_rgb(a)
    rb, gb, bb = hex_to_rgb(b)
    return "#{:02x}{:02x}{:02x}".format(
        round(ra + (rb - ra) * ratio),
        round(ga + (gb - ga) * ratio),
        round(ba + (bb - ba) * ratio),
    )


def build_roles(tokens: dict, mode: str) -> dict[str, str]:
    """Resolve the ten Diagram Design roles for one ground mode."""
    palette = resolve_palette(tokens)
    grounds = tokens.get("grounds") or {}
    ground = grounds.get(mode) or {}
    roles_map = tokens.get("palette_roles") or {}

    out: dict[str, str] = {}
    for role, (ground_key, role_key) in ROLE_SOURCES.items():
        value = lookup(palette, ground.get(ground_key))
        if value is None and mode == "light":
            # `palette_roles` describes the light ground. Reading it in dark mode
            # would report the light palette as a dark one instead of falling
            # through to the inversion rule.
            value = lookup(palette, roles_map.get(role_key))
        if value is None and mode == "dark":
            # A light-only language is inverted by the caller.
            return {}
        if value is None:
            raise ProjectionError(
                f"cannot resolve the '{role}' role for the {mode} ground: "
                f"tokens.yaml names neither grounds.{mode}.{ground_key} nor "
                f"palette_roles.{role_key} as a known palette key"
            )
        out[role] = value

    # `palette_roles.surface` describes the light ground, so a dark mode takes
    # its own ground's surface or a derived one; reusing the light value puts a
    # pale panel on a dark canvas.
    surface = lookup(palette, ground.get("surface"))
    if surface is None and mode == "light":
        surface = lookup(palette, roles_map.get("surface"))
    out["paper-2"] = surface or mix(out["paper"], out["ink"], 0.06)
    body = lookup(palette, ground.get("body"))
    out["soft"] = body or mix(out["muted"], out["paper"], 0.35)
    out["rule"] = rgba(out["ink"], 0.12)
    out["accent-tint"] = rgba(out["accent"], 0.08 if mode == "light" else 0.10)
    # No design language names a distinct hyperlink colour; Diagram Design uses
    # `link` for external/API arrows, where reusing the accent would break the
    # 1-2-focal-nodes rule. A muted-leaning accent keeps it in family.
    out["link"] = mix(out["accent"], out["muted"], 0.45)
    return out


def invert_roles(light: dict[str, str]) -> dict[str, str]:
    """Shipped inversion rule: swap paper and ink, keep the accent readable."""
    dark = dict(light)
    dark["paper"], dark["ink"] = light["ink"], light["paper"]
    dark["paper-2"] = mix(dark["paper"], dark["ink"], 0.08)
    dark["muted"] = mix(light["muted"], dark["ink"], 0.35)
    dark["soft"] = mix(dark["muted"], dark["paper"], 0.30)
    dark["rule"] = rgba(dark["ink"], 0.12)
    dark["rule-solid"] = mix(light["rule-solid"], dark["ink"], 0.30)
    dark["accent"] = mix(light["accent"], "#ffffff", 0.18)
    dark["accent-tint"] = rgba(dark["accent"], 0.10)
    dark["link"] = mix(dark["accent"], dark["muted"], 0.45)
    return dark


def font_families(tokens: dict) -> dict[str, str]:
    """Map the language's type roles onto the guide's display/sans/mono slots."""
    types = tokens.get("type") or {}

    def stack(role: str) -> str | None:
        entry = types.get(role)
        if not isinstance(entry, dict):
            return None
        family = entry.get("family")
        if not family:
            return None
        fallback = entry.get("fallback") or []
        parts = [str(family)] + [str(f) for f in fallback]
        return ", ".join(parts)

    body = stack("body") or stack("primary")
    display = stack("display") or body
    mono = stack("mono") or "Consolas, monospace"
    if not body:
        raise ProjectionError(
            "tokens.yaml declares no `type.body` (or `type.primary`) family; "
            "a profile without typography would silently keep the shipped Geist"
        )
    return {"display": display, "sans": body, "mono": mono}


def first_family(stack: str) -> str:
    return stack.split(",")[0].strip().strip("'\"")


def rewrite_roles_table(guide: str, light: dict[str, str], dark: dict[str, str]) -> str:
    """Replace only the value cells of the semantic-roles table."""

    def row(match: re.Match[str]) -> str:
        role = match.group("role")
        if role not in light:
            return match.group(0)
        return (
            f"| `{role}` | {match.group('purpose').strip()} "
            f"| `{light[role]}` | `{dark[role]}` |"
        )

    pattern = re.compile(
        r"^\|\s*`(?P<role>[\w-]+)`\s*\|(?P<purpose>[^|]*)\|[^|]*\|[^|]*\|\s*$",
        re.M,
    )
    return pattern.sub(row, guide, count=len(light))


def rewrite_typography_table(guide: str, fonts: dict[str, str]) -> str:
    """Replace the family cell for each typography role."""
    role_font = {
        "title": first_family(fonts["display"]),
        "node-name": f"{first_family(fonts['sans'])} (sans)",
        "sublabel": first_family(fonts["mono"]),
        "eyebrow": first_family(fonts["mono"]),
        "arrow-label": first_family(fonts["mono"]),
        "callout": f"{first_family(fonts['display'])} *italic*",
    }

    def row(match: re.Match[str]) -> str:
        role = match.group("role")
        if role not in role_font:
            return match.group(0)
        return (
            f"| `{role}` | {role_font[role]} |{match.group('size')}|"
            f"{match.group('weight')}|{match.group('usage')}|"
        )

    pattern = re.compile(
        r"^\|\s*`(?P<role>[\w-]+)`\s*\|[^|]*\|(?P<size>[^|]*)\|(?P<weight>[^|]*)\|(?P<usage>[^|]*)\|\s*$",
        re.M,
    )
    return pattern.sub(row, guide)


def rewrite_font_stack(guide: str, fonts: dict[str, str], tokens: dict) -> str:
    """Replace the shipped Google Fonts block and the rule that names families.

    Left alone, the profile still tells the agent to load Instrument Serif and
    Geist and to put names in Geist — so a diagram would carry the language's
    colours in the shipped skin's typefaces.
    """
    import_url = ((tokens.get("type") or {}).get("google_fonts_import")) or ""
    display, sans, mono = first_family(fonts["display"]), first_family(fonts["sans"]), first_family(fonts["mono"])

    if import_url:
        loader = f'```html\n<link href="{import_url}" rel="stylesheet">\n```'
    else:
        loader = (
            "This language uses system-installed typefaces; there is no webfont "
            "import. Declare the full stacks directly in the diagram's CSS:\n\n"
            "```css\n"
            f"--serif: {fonts['display']};\n"
            f"--sans:  {fonts['sans']};\n"
            f"--mono:  {fonts['mono']};\n"
            "```"
        )

    rule = (
        f"**Load-bearing rule:** Mono is for *technical* content (ports, commands, "
        f"URLs, field types) and is {mono}. Names go in {sans}. Page title is "
        f"{display}. Italic {display} is reserved for annotation callouts (see "
        f"[primitive-annotation.md](primitive-annotation.md))."
    )

    section = re.compile(
        r"(### Font stack\n\n).*?(?=\n---\n)", re.S
    )
    if not section.search(guide):
        return guide
    return section.sub(lambda m: m.group(1) + loader + "\n\n" + rule + "\n", guide, count=1)


def rewrite_typography_constraint(body: str, fonts: dict[str, str]) -> str:
    """The shipped constraint tells the agent to keep Instrument Serif for the
    title even when the brand is all sans. Left in place it overrides the
    projected display face at the moment the agent decides what to draw."""
    display, sans = first_family(fonts["display"]), first_family(fonts["sans"])
    kept = (
        f"- **Display + text + mono**: three roles, not more. The display face "
        f"for `title` and `callout` is {display}; body and node names are {sans}. "
        f"When the language sets both from one family, hold the contrast with "
        f"weight and size rather than adding a fourth typeface."
    )
    return re.sub(
        r"^- \*\*Serif \+ sans \+ mono\*\*:.*$", kept, body, count=1, flags=re.M
    )


def rewrite_node_treatments(body: str) -> str:
    """The shipped `backend` node is literal white. A design language that names
    its own surface expects panels in that surface, not in white."""
    return re.sub(
        r"^\|\s*`backend`\s*\|[^|]*\|",
        "| `backend` | `paper-2` |",
        body,
        count=1,
        flags=re.M,
    )


def replace_provenance_note(body: str, name: str) -> str:
    """The shipped guide explains its own default palette in a blockquote. Left
    in place it contradicts the projected tokens, so it is repointed at the
    design language that now owns them."""
    replacement = (
        f"> **Brand palette source:** projected from "
        f"`library/assets/design-languages/{name}/tokens.yaml`, which is the "
        f"single source of truth for this identity. Do not edit token values "
        f"here; regenerate with `python system/tools/diagram_profile.py {name}`. "
        f"The `soft`, `rule`, `accent-tint`, and `link` roles are derived, "
        f"because a deck design language does not name them directly."
    )
    body = re.sub(
        r"^> \*\*Brand palette source:\*\*.*?(?=\n\n)", replacement, body, count=1, flags=re.S | re.M
    )
    # The shipped note about pre-baked examples describes the vendor's own
    # regeneration backlog and says nothing about a projected profile.
    return re.sub(r"^> \*\*Note:\*\* The pre-baked example HTML.*?(?=\n\n)", "", body, count=1, flags=re.S | re.M)


def strip_existing_header(body: str) -> str:
    return re.sub(r"\A<!--\s*diagram-design-profile.*?-->\n?\n?", "", body, flags=re.S)


def sanitize(value: str) -> str:
    """A header value must stay on one line and cannot close the comment."""
    return re.sub(r"\s+", " ", str(value)).replace("--", "-").strip() or "none"


def build_profile(name: str, tokens: dict, guide: str, summary: str) -> str:
    light = build_roles(tokens, "light")
    dark = build_roles(tokens, "dark") or invert_roles(light)
    fonts = font_families(tokens)

    body = strip_existing_header(guide)
    body = rewrite_roles_table(body, light, dark)
    body = rewrite_typography_table(body, fonts)
    body = rewrite_font_stack(body, fonts, tokens)
    body = rewrite_typography_constraint(body, fonts)
    body = rewrite_node_treatments(body)
    body = replace_provenance_note(body, name)

    today = dt.date.today().isoformat()
    header = (
        "<!-- diagram-design-profile\n"
        f"name: {sanitize(name)}\n"
        f"slug: {name}\n"
        "source-url: none\n"
        f"created: {today}\n"
        f"updated: {today}\n"
        f"notes: {sanitize(summary)}\n"
        "-->\n\n"
    )
    return header + body


def load_language(name: str) -> tuple[dict, str]:
    tokens_path = LANGUAGES / name / "tokens.yaml"
    if not tokens_path.exists():
        available = sorted(p.name for p in LANGUAGES.iterdir() if p.is_dir()) if LANGUAGES.exists() else []
        raise ProjectionError(
            f"no tokens.yaml for design language '{name}'. Available: "
            + (", ".join(available) or "none")
        )
    tokens = yaml.safe_load(tokens_path.read_text(encoding="utf-8")) or {}
    meta = tokens.get("meta") or {}
    summary = meta.get("summary") or f"Projected from library/assets/design-languages/{name}/tokens.yaml"
    return tokens, summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("language", help="design-language directory name under library/assets/design-languages/")
    parser.add_argument(
        "--marker",
        help="project root to write a `.diagram-design` marker into, so diagrams "
        "authored in that project resolve this profile automatically",
    )
    parser.add_argument("--print", action="store_true", help="write to stdout instead of the profile library")
    args = parser.parse_args()

    name = args.language.strip().strip("/\\")
    if not SLUG_RE.fullmatch(name):
        print(f"error: '{name}' is not a valid profile slug (lowercase, digits, hyphens)", file=sys.stderr)
        return 1
    if name == "default":
        print("error: 'default' is reserved for the shipped profile", file=sys.stderr)
        return 1

    try:
        tokens, summary = load_language(name)
        if not SHIPPED_GUIDE.exists():
            raise ProjectionError(f"vendored style guide missing at {SHIPPED_GUIDE}")
        profile = build_profile(name, tokens, SHIPPED_GUIDE.read_text(encoding="utf-8"), summary)
    except ProjectionError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.print:
        # The style guide carries typographic Unicode; a Windows console
        # defaults to cp1252 and would fail on it.
        if hasattr(sys.stdout, "reconfigure"):
            sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        sys.stdout.write(profile)
        return 0

    PROFILE_LIBRARY.mkdir(parents=True, exist_ok=True)
    dest = PROFILE_LIBRARY / f"{name}.md"
    dest.write_text(profile, encoding="utf-8")
    print(f"wrote profile {dest}")

    if args.marker:
        marker_root = Path(args.marker)
        if not marker_root.is_dir():
            print(f"error: marker path {marker_root} is not a directory", file=sys.stderr)
            return 1
        marker = marker_root / ".diagram-design"
        marker.write_text(f"profile: {name}\n", encoding="utf-8")
        print(f"wrote marker {marker}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
