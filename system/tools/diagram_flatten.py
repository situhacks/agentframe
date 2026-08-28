#!/usr/bin/env python3
"""Flatten a Diagram Design HTML diagram into a PPT Master-clean SVG.

Diagram Design emits self-contained HTML whose inline SVG leans on browser CSS:
custom properties (`fill="var(--accent)"`), `class` attributes resolved from a
`<style>` block, and percentage geometry (`width="100%"`). PPT Master's
converter reads presentation attributes only, so every one of those is a
blocking finding in `svg_quality_checker.py`.

This tool resolves all three mechanically, so a diagram compiles to native
editable PowerPoint shapes instead of being pasted in as a raster:

    python system/tools/diagram_flatten.py diagram.html -o diagram.svg

Nothing here interprets the diagram. Geometry, structure, and paint order are
carried through untouched; only the way a value is *expressed* changes.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# CSS properties worth carrying to a presentation attribute. Anything outside
# this set is page chrome the converter has no use for (layout, spacing, case).
PAINT_PROPS = {
    "fill",
    "fill-opacity",
    "stroke",
    "stroke-width",
    "stroke-opacity",
    "stroke-dasharray",
    "stroke-linecap",
    "stroke-linejoin",
    "opacity",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "letter-spacing",
    "text-anchor",
    "dominant-baseline",
    "text-transform",
}

# Colour-valued properties get the rgba() -> hex + channel-opacity split.
COLOR_PROPS = {"fill": "fill-opacity", "stroke": "stroke-opacity"}

# CSS named colours the shipped templates actually use. The converter accepts a
# name but prefers hex; an unlisted name passes through unchanged rather than
# being guessed at.
NAMED_COLORS = {
    "white": "#FFFFFF",
    "black": "#000000",
    "red": "#FF0000",
    "gray": "#808080",
    "grey": "#808080",
    "silver": "#C0C0C0",
    "whitesmoke": "#F5F5F5",
}

COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)
STYLE_RE = re.compile(r"<style[^>]*>(.*?)</style>", re.S | re.I)
XML_COMMENT_RE = re.compile(r"<!--.*?-->", re.S)
SVG_OPEN_RE = re.compile(r"<svg\b", re.I)
VAR_RE = re.compile(r"var\(\s*(--[\w-]+)\s*(?:,\s*([^()]*?)\s*)?\)")
RGBA_RE = re.compile(
    r"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)", re.I
)
HEX_RE = re.compile(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$")
# One start tag. HTML permits single-quoted, unquoted, and valueless attributes;
# the emitted SVG is parsed as strict XML, so all three are normalized on the way
# out. `data-polar-chart` in the shipped polar template is a valueless one.
_ATTR_PAT = r"""[\w:.-]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'>=`]+))?"""
TAG_RE = re.compile(r"<([a-zA-Z][\w:-]*)((?:\s+" + _ATTR_PAT + r")*)\s*(/?)>")
ATTR_RE = re.compile(
    r"""([\w:.-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>=`]+)))?"""
)
# `font: <style|weight>* <size>[/<line-height>] <family>` shorthand.
FONT_SHORTHAND_RE = re.compile(
    r"^\s*(?P<pre>(?:(?:normal|italic|oblique|bold|bolder|lighter|\d{3})\s+)*)"
    r"(?P<size>[\d.]+(?:px|pt|rem|em)?|clamp\([^)]*\))"
    r"(?:\s*/\s*[\d.]+\w*)?"
    r"\s+(?P<family>.+?)\s*$"
)


class Rule:
    """One simple compound CSS selector and the declarations it sets."""

    __slots__ = ("tag", "classes", "ident", "specificity", "order", "decls")

    def __init__(self, tag, classes, ident, order, decls):
        self.tag = tag
        self.classes = classes
        self.ident = ident
        self.order = order
        self.decls = decls
        self.specificity = (100 * (1 if ident else 0)) + (10 * len(classes)) + (1 if tag else 0)

    def matches(self, tag: str, classes: set[str], ident: str | None) -> bool:
        if self.tag and self.tag != tag:
            return False
        if self.ident and self.ident != ident:
            return False
        return self.classes <= classes


def parse_declarations(body: str) -> list[tuple[str, str]]:
    """Split a declaration block, preserving source order for later overrides."""
    out: list[tuple[str, str]] = []
    for chunk in split_top_level(body, ";"):
        if ":" not in chunk:
            continue
        prop, _, value = chunk.partition(":")
        prop = prop.strip().lower()
        value = value.strip()
        if prop and value:
            out.append((prop, value))
    return out


def split_top_level(text: str, sep: str) -> list[str]:
    """Split on `sep` while ignoring separators inside parens or quotes."""
    parts: list[str] = []
    depth = 0
    quote = ""
    buf: list[str] = []
    for ch in text:
        if quote:
            if ch == quote:
                quote = ""
            buf.append(ch)
            continue
        if ch in "\"'":
            quote = ch
            buf.append(ch)
            continue
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth = max(0, depth - 1)
        if ch == sep and depth == 0:
            parts.append("".join(buf))
            buf = []
            continue
        buf.append(ch)
    parts.append("".join(buf))
    return [p.strip() for p in parts if p.strip()]


def parse_stylesheet(css: str) -> tuple[dict[str, str], list[Rule]]:
    """Return the `:root` custom-property map and every usable simple rule.

    Selectors with combinators, pseudo-classes, or at-rules are skipped: they
    target page chrome, and a rule this parser cannot honour exactly is more
    dangerous inlined than dropped.
    """
    css = COMMENT_RE.sub("", css)
    variables: dict[str, str] = {}
    rules: list[Rule] = []
    order = 0
    depth_skip = False

    for block in css.split("}"):
        if "{" not in block:
            continue
        selector, _, body = block.partition("{")
        selector = selector.strip()
        if not selector:
            continue
        if selector.startswith("@"):
            # An at-rule's own braces are consumed by the naive split; the
            # nested rules that follow are skipped by the guard below.
            depth_skip = True
            continue
        if depth_skip and selector.startswith(("from", "to", "0%", "100%")):
            continue
        depth_skip = False

        decls = parse_declarations(body)
        if not decls:
            continue

        for one in split_top_level(selector, ","):
            if one == ":root" or one == "html":
                for prop, value in decls:
                    if prop.startswith("--"):
                        variables[prop] = value
                continue
            parsed = parse_simple_selector(one)
            if parsed is None:
                continue
            tag, classes, ident = parsed
            order += 1
            rules.append(Rule(tag, classes, ident, order, decls))

    return variables, rules


def parse_simple_selector(selector: str) -> tuple[str | None, set[str], str | None] | None:
    """Parse `tag`, `.a`, `.a.b`, `#id`, `tag.a`. Reject everything else."""
    selector = selector.strip()
    if not selector or any(c in selector for c in " >+~:[*("):
        return None
    if not re.fullmatch(r"(?:[a-zA-Z][\w-]*)?(?:[.#][\w-]+)*", selector):
        return None
    tag_match = re.match(r"^[a-zA-Z][\w-]*", selector)
    tag = tag_match.group(0).lower() if tag_match else None
    classes = set(re.findall(r"\.([\w-]+)", selector))
    ids = re.findall(r"#([\w-]+)", selector)
    if len(ids) > 1:
        return None
    if not tag and not classes and not ids:
        return None
    return tag, classes, (ids[0] if ids else None)


def resolve_vars(value: str, variables: dict[str, str], seen: frozenset[str] = frozenset()) -> str:
    """Substitute `var(--x)` recursively, honouring the declared fallback."""

    def repl(match: re.Match[str]) -> str:
        name, fallback = match.group(1), match.group(2)
        if name in seen:
            return fallback or ""
        if name in variables:
            return resolve_vars(variables[name], variables, seen | {name})
        return fallback or ""

    previous = None
    out = value
    while previous != out and "var(" in out:
        previous = out
        out = VAR_RE.sub(repl, out)
    return out.strip()


def normalize_color(value: str) -> tuple[str, str | None]:
    """Return (colour, channel-opacity-or-None) in the converter's dialect."""
    value = value.strip()
    named = NAMED_COLORS.get(value.lower())
    if named:
        return named, None
    rgba = RGBA_RE.fullmatch(value)
    if rgba:
        r, g, b, a = rgba.groups()
        hexed = "#{:02X}{:02X}{:02X}".format(int(r), int(g), int(b))
        if a is None:
            return hexed, None
        alpha = float(a)
        if alpha >= 1:
            return hexed, None
        # Trim to the shortest exact decimal so the attribute stays readable.
        return hexed, ("%g" % round(alpha, 4))
    if HEX_RE.fullmatch(value):
        digits = value[1:]
        if len(digits) in (3, 4):
            digits = "".join(ch * 2 for ch in digits)
        if len(digits) == 8:
            alpha = int(digits[6:8], 16) / 255
            return "#" + digits[:6].upper(), ("%g" % round(alpha, 4))
        return "#" + digits.upper(), None
    return value, None


def expand_font_shorthand(value: str) -> list[tuple[str, str]]:
    """Expand the `font:` shorthand the templates use into SVG properties."""
    match = FONT_SHORTHAND_RE.match(value)
    if not match:
        return []
    out: list[tuple[str, str]] = []
    for token in match.group("pre").split():
        token = token.lower()
        if token in {"italic", "oblique"}:
            out.append(("font-style", token))
        elif token != "normal":
            out.append(("font-weight", token))
    out.append(("font-size", match.group("size")))
    out.append(("font-family", match.group("family").strip()))
    return out


def normalize_dasharray(value: str) -> str:
    """A one-value dasharray means equal dash and gap. SVG treats `1` and `1 1`
    identically; the converter requires the explicit pair."""
    value = value.strip()
    if value in {"none", ""}:
        return value or "none"
    parts = [px_to_number(p) for p in re.split(r"[\s,]+", value) if p]
    if len(parts) == 1:
        parts = parts * 2
    return " ".join(parts)


def px_to_number(value: str) -> str:
    """`12px` -> `12`. The converter accepts unitless values or a px suffix;
    unitless keeps the emitted SVG closest to PPT Master's own authoring."""
    match = re.fullmatch(r"(-?[\d.]+)px", value.strip(), re.I)
    return match.group(1) if match else value


def em_to_px(value: str, font_size: str | None) -> str:
    """`letter-spacing: 0.18em` has no converter equivalent; resolve it against
    the element's own font-size, which is what a browser does."""
    match = re.fullmatch(r"(-?[\d.]+)em", value.strip(), re.I)
    if not match or not font_size:
        return value
    size = re.fullmatch(r"(-?[\d.]+)(?:px)?", font_size.strip(), re.I)
    if not size:
        return value
    return "%g" % round(float(match.group(1)) * float(size.group(1)), 4)


class Flattener:
    def __init__(self, variables, rules, canvas, font_map, keep_class, drop_background=False):
        self.variables = variables
        self.rules = sorted(rules, key=lambda r: (r.specificity, r.order))
        self.canvas_w, self.canvas_h = canvas
        self.font_map = font_map
        self.keep_class = keep_class
        self.drop_background = drop_background
        self.dropped = 0
        # `currentColor` inherits from the nearest `color`; the templates set the
        # document one on `body` (or `svg`), which is what the icons resolve to.
        self.default_color = ""
        for rule in self.rules:
            if rule.tag in {"body", "svg", "html"} and not rule.classes and not rule.ident:
                for prop, value in rule.decls:
                    if prop == "color":
                        self.default_color = value

    def is_canvas_fill(self, tag: str, attrs: dict[str, str], closer: str) -> bool:
        """A full-canvas `<rect>` is the diagram's own page ground. On a slide
        the design language already owns the ground, so carrying this through
        paints a light plate over a dark deck."""
        if tag.lower() != "rect" or not closer:
            return False
        if not (self.canvas_w and self.canvas_h):
            return False
        try:
            if float(attrs.get("x", "0") or 0) or float(attrs.get("y", "0") or 0):
                return False
            width = float(px_to_number(attrs.get("width", "")).rstrip("%"))
            height = float(px_to_number(attrs.get("height", "")).rstrip("%"))
        except ValueError:
            return False
        if attrs.get("width", "").endswith("%"):
            width = width / 100 * self.canvas_w
        if attrs.get("height", "").endswith("%"):
            height = height / 100 * self.canvas_h
        return abs(width - self.canvas_w) < 0.5 and abs(height - self.canvas_h) < 0.5

    def computed_style(self, tag, classes, ident, inline_style):
        """CSS rules beat presentation attributes; `style=` beats both."""
        style: dict[str, str] = {}
        for rule in self.rules:
            if rule.matches(tag, classes, ident):
                for prop, value in rule.decls:
                    style[prop] = value
        for prop, value in parse_declarations(inline_style):
            style[prop] = value
        return style

    def map_font_family(self, value: str) -> str:
        for needle, replacement in self.font_map.items():
            if needle.lower() in value.lower():
                return replacement
        return value

    def emit_props(self, props: dict[str, str]) -> dict[str, str]:
        """Normalize a property map into converter-safe attribute values."""
        expanded: dict[str, str] = {}
        for prop, value in props.items():
            if prop == "font":
                for sub_prop, sub_value in expand_font_shorthand(value):
                    expanded[sub_prop] = sub_value
                continue
            expanded[prop] = value

        out: dict[str, str] = {}
        for prop, value in expanded.items():
            if prop not in PAINT_PROPS:
                continue
            value = resolve_vars(value, self.variables)
            if not value or value == "inherit":
                continue
            if prop in COLOR_PROPS:
                if value == "transparent":
                    out[prop] = "none"
                    continue
                if value == "currentColor":
                    # The converter has no inherited-colour concept. Resolve to
                    # the element's own computed `color`, then the document's.
                    inherited = props.get("color") or self.default_color
                    if inherited:
                        resolved_color, alpha = normalize_color(
                            resolve_vars(inherited, self.variables)
                        )
                        out[prop] = resolved_color
                        if alpha is not None:
                            out.setdefault(COLOR_PROPS[prop], alpha)
                        continue
                if value.startswith("url(") or value in {"none", "currentColor"}:
                    out[prop] = value
                    continue
                color, alpha = normalize_color(value)
                out[prop] = color
                if alpha is not None:
                    # An explicit channel opacity already in the map wins.
                    out.setdefault(COLOR_PROPS[prop], alpha)
                continue
            if prop == "font-family":
                out[prop] = self.map_font_family(value)
                continue
            if prop in {"font-size", "stroke-width"}:
                out[prop] = px_to_number(value)
                continue
            if prop == "stroke-dasharray":
                out[prop] = normalize_dasharray(value)
                continue
            if prop == "letter-spacing":
                out[prop] = value  # resolved below, once font-size is known
                continue
            if prop == "text-transform":
                continue  # applied to the text content instead
            out[prop] = value

        if "letter-spacing" in out:
            out["letter-spacing"] = px_to_number(
                em_to_px(out["letter-spacing"], out.get("font-size"))
            )
        return out

    def flatten_tag(self, match: re.Match[str]) -> str:
        tag = match.group(1)
        attrs = parse_attributes(match.group(2) or "")
        closer = match.group(3)

        if self.drop_background and self.is_canvas_fill(tag, attrs, closer):
            self.dropped += 1
            return ""

        classes = set((attrs.get("class") or "").split())
        ident = attrs.get("id")
        inline_style = attrs.pop("style", "")

        # Presentation attributes are the base layer, below any CSS rule.
        base = {k: v for k, v in attrs.items() if k in PAINT_PROPS or k == "font"}
        merged = dict(base)
        merged.update(self.computed_style(tag.lower(), classes, ident, inline_style))

        resolved = self.emit_props(merged)

        out: dict[str, str] = {}
        for key, value in attrs.items():
            if key == "class":
                continue
            if key in PAINT_PROPS or key == "font":
                continue
            out[key] = value

        # Percentage geometry has no converter equivalent; the root viewBox is
        # the canvas authority, so `100%` is exactly the canvas dimension.
        for axis, extent in (("width", self.canvas_w), ("height", self.canvas_h)):
            value = out.get(axis, "")
            pct = re.fullmatch(r"([\d.]+)%", value.strip())
            if pct and extent:
                out[axis] = "%g" % (float(pct.group(1)) / 100 * extent)
        for axis in ("x", "y", "width", "height", "rx", "ry", "cx", "cy", "r", "stroke-width"):
            if axis in out:
                out[axis] = px_to_number(out[axis])

        out.update(resolved)
        if self.keep_class and "class" in attrs:
            out["class"] = attrs["class"]

        rendered = "".join(f' {k}="{escape_attr(v)}"' for k, v in out.items())
        return f"<{tag}{rendered}{'/' if closer else ''}>"

    def run(self, svg: str) -> str:
        return TAG_RE.sub(self.flatten_tag, svg)


# A nested <svg> becomes <g transform>; these attributes describe the viewport
# it no longer has, or accessibility the HTML deliverable still carries.
NESTED_SVG_DROP = {
    "x", "y", "width", "height", "viewBox", "viewbox", "xmlns", "xmlns:xlink",
    "preserveAspectRatio", "preserveaspectratio", "aria-hidden", "role",
    "aria-labelledby", "version",
}


def inline_nested_svgs(svg: str) -> str:
    """Rewrite nested `<svg>` icon viewports as `<g transform=...>`.

    PPT Master reads a nested `<svg>` as an imported crop wrapper and accepts
    only a narrow attribute set on one, so the icon viewports the data-platform
    diagrams use are rejected wholesale. A translate/scale group is the exact
    geometric equivalent and converts to a native group instead.
    """
    while True:
        root_end = svg.find(">")
        if root_end == -1:
            return svg
        nested = SVG_OPEN_RE.search(svg, root_end + 1)
        if not nested:
            return svg

        match = TAG_RE.match(svg, nested.start())
        if not match:
            return svg
        attrs = parse_attributes(match.group(2) or "")

        # Find this element's matching close tag.
        depth = 1
        cursor = match.end()
        close_start = close_end = -1
        for token in re.finditer(r"<svg\b|</svg\s*>", svg[cursor:], re.I):
            if token.group(0).lower().startswith("</"):
                depth -= 1
                if depth == 0:
                    close_start = cursor + token.start()
                    close_end = cursor + token.end()
                    break
            else:
                depth += 1
        if close_start == -1:
            return svg

        transform = nested_transform(attrs)
        kept = {k: v for k, v in attrs.items() if k not in NESTED_SVG_DROP}
        if transform:
            existing = kept.pop("transform", "")
            kept["transform"] = f"{existing} {transform}".strip()
        rendered = "".join(f' {k}="{escape_attr(v)}"' for k, v in kept.items())

        svg = (
            svg[: nested.start()]
            + f"<g{rendered}>"
            + svg[match.end(): close_start]
            + "</g>"
            + svg[close_end:]
        )


def nested_transform(attrs: dict[str, str]) -> str:
    """translate() for the viewport origin, scale() for a viewBox that differs."""
    def number(key: str, default: float = 0.0) -> float:
        try:
            return float(px_to_number(attrs.get(key, "")) or default)
        except ValueError:
            return default

    x, y = number("x"), number("y")
    width, height = number("width"), number("height")
    box = attrs.get("viewBox") or attrs.get("viewbox") or ""
    parts = re.split(r"[\s,]+", box.strip()) if box.strip() else []

    scale_x = scale_y = 1.0
    min_x = min_y = 0.0
    if len(parts) == 4:
        try:
            min_x, min_y, box_w, box_h = (float(p) for p in parts)
            if box_w and width:
                scale_x = width / box_w
            if box_h and height:
                scale_y = height / box_h
        except ValueError:
            pass

    pieces = []
    tx, ty = x - min_x * scale_x, y - min_y * scale_y
    if tx or ty:
        pieces.append("translate(%g %g)" % (tx, ty))
    if scale_x != 1.0 or scale_y != 1.0:
        pieces.append("scale(%g %g)" % (scale_x, scale_y))
    return " ".join(pieces)


MARKER_DEF_RE = re.compile(r'<marker\b[^>]*\bid="([^"]+)"[^>]*>.*?</marker>', re.S | re.I)
MARKER_REF_RE = re.compile(r'marker-(start|mid|end)="url\(#([^)"]+)\)"', re.I)


def match_marker_colors(svg: str) -> tuple[str, int]:
    """Repaint line-end markers to their line's stroke, cloning where a marker
    is shared by lines of different colours.

    PowerPoint line-ends take the line's own colour; an independently filled
    arrowhead (diagram-design's hollow UML triangle, its muted arrow on an ink
    line) has no native equivalent. This changes how the diagram looks, so it is
    opt-in rather than part of the mechanical translation.
    """
    markers = {m.group(1): m.group(0) for m in MARKER_DEF_RE.finditer(svg)}
    if not markers:
        return svg, 0

    variants: dict[tuple[str, str], str] = {}
    changed = 0

    def repaint(block: str, color: str, new_id: str) -> str:
        block = re.sub(r'\bid="[^"]*"', f'id="{new_id}"', block, count=1)

        def paint(match: re.Match[str]) -> str:
            prop, value = match.group(1), match.group(2)
            if value == "none" or value.startswith("url("):
                return match.group(0)
            return f'{prop}="{color}"'

        return re.sub(r'\b(fill|stroke)="([^"]*)"', paint, block)

    def rewrite_element(match: re.Match[str]) -> str:
        nonlocal changed
        tag = match.group(0)
        stroke_match = re.search(r'\bstroke="(#[0-9A-Fa-f]{6})"', tag)
        if not stroke_match:
            return tag
        stroke = stroke_match.group(1).upper()

        def swap(ref: re.Match[str]) -> str:
            nonlocal changed
            position, marker_id = ref.group(1), ref.group(2)
            block = markers.get(marker_id)
            if block is None:
                return ref.group(0)
            current = re.search(r'\bfill="(#[0-9A-Fa-f]{6})"', block)
            if current and current.group(1).upper() == stroke:
                return ref.group(0)
            key = (marker_id, stroke)
            if key not in variants:
                variants[key] = f"{marker_id}-{stroke.lstrip('#').lower()}"
            changed += 1
            return f'marker-{position}="url(#{variants[key]})"'

        return MARKER_REF_RE.sub(swap, tag)

    out = TAG_RE.sub(rewrite_element, svg)
    if not variants:
        return svg, 0

    clones = "".join(repaint(markers[mid], color, new_id) for (mid, color), new_id in variants.items())
    if "</defs>" in out:
        out = out.replace("</defs>", clones + "</defs>", 1)
    else:
        out = re.sub(r"(>)", r"\1<defs>" + clones + "</defs>", out, count=1)
    return out, changed


def parse_attributes(raw: str) -> dict[str, str]:
    """Read a start tag's attributes. A valueless HTML attribute becomes an
    empty-valued XML one, which keeps presence detectable and the file parseable."""
    attrs: dict[str, str] = {}
    for name, dq, sq, bare in ATTR_RE.findall(raw):
        if dq:
            value = dq
        elif sq:
            value = sq
        elif bare:
            value = bare
        else:
            value = ""
        attrs[name] = value
    return attrs


def escape_attr(value: str) -> str:
    """Escape for an XML attribute without double-escaping an existing entity."""
    value = re.sub(r"&(?!(?:#\d+|#x[0-9a-fA-F]+|amp|lt|gt|quot|apos);)", "&amp;", value)
    return value.replace("<", "&lt;").replace('"', "&quot;")


def extract_svg(html: str) -> str:
    """Return the outermost `<svg>` element, nesting included.

    Several shipped diagrams inline icon `<svg>` elements inside the diagram,
    so stopping at the first `</svg>` truncates the file mid-document.
    """
    start = SVG_OPEN_RE.search(html)
    if not start:
        raise ValueError("no <svg> element found in the input document")

    depth = 0
    pos = start.start()
    for match in re.finditer(r"<svg\b|</svg\s*>", html[start.start():], re.I):
        token = match.group(0).lower()
        if token.startswith("</"):
            depth -= 1
            if depth == 0:
                return html[pos: start.start() + match.end()]
        else:
            # A self-closing `<svg .../>` opens and closes in one tag.
            tag_end = html.find(">", start.start() + match.start())
            if tag_end != -1 and html[tag_end - 1] == "/":
                continue
            depth += 1
    raise ValueError("unbalanced <svg> element in the input document")


def read_canvas(svg: str) -> tuple[float | None, float | None]:
    match = re.search(r'viewBox\s*=\s*"([^"]+)"', svg)
    if not match:
        return None, None
    parts = re.split(r"[\s,]+", match.group(1).strip())
    if len(parts) != 4:
        return None, None
    try:
        return float(parts[2]), float(parts[3])
    except ValueError:
        return None, None


def parse_font_map(raw: str | None) -> dict[str, str]:
    """`--font-map "Geist=Aptos,Geist Mono=Consolas"` -> substitution map."""
    out: dict[str, str] = {}
    if not raw:
        return out
    for pair in raw.split(","):
        if "=" not in pair:
            continue
        needle, _, replacement = pair.partition("=")
        needle, replacement = needle.strip(), replacement.strip()
        if needle and replacement:
            out[needle] = replacement
    return out


def flatten_document(
    html: str, font_map=None, keep_class=False, drop_background=False, match_markers=False
) -> str:
    """Extract the first SVG from a Diagram Design document and flatten it."""
    svg = extract_svg(html)
    # Authoring comments carry no rendering meaning, and the templates contain
    # `--` runs inside them, which strict XML forbids.
    svg = XML_COMMENT_RE.sub("", svg)

    svg = inline_nested_svgs(svg)

    css = "\n".join(STYLE_RE.findall(html))
    variables, rules = parse_stylesheet(css)
    canvas = read_canvas(svg)

    flattener = Flattener(variables, rules, canvas, font_map or {}, keep_class, drop_background)
    out = flattener.run(svg)

    if match_markers:
        out, _ = match_marker_colors(out)

    if 'xmlns="http://www.w3.org/2000/svg"' not in out:
        out = out.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"', 1)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("input", help="Diagram Design .html file")
    parser.add_argument("-o", "--output", help="output .svg (default: alongside the input)")
    parser.add_argument(
        "--font-map",
        help='substitute font families, e.g. "Geist=Aptos,Geist Mono=Consolas". '
        "Prefer setting the design language's fonts in the diagram profile so "
        "the diagram is authored in them; this is the escape hatch.",
    )
    parser.add_argument(
        "--no-background",
        action="store_true",
        help="drop full-canvas background rects. Use for a diagram going onto a "
        "slide, where the design language already owns the ground.",
    )
    parser.add_argument(
        "--match-markers",
        action="store_true",
        help="repaint line-end markers to their line's stroke colour. Changes "
        "how the diagram looks (a hollow arrowhead becomes solid); PowerPoint "
        "line-ends cannot carry a fill independent of the line.",
    )
    parser.add_argument(
        "--keep-class",
        action="store_true",
        help="retain class attributes (debugging; PPT Master rejects them)",
    )
    parser.add_argument("--stdout", action="store_true", help="write to stdout instead of a file")
    args = parser.parse_args()

    src = Path(args.input)
    if not src.exists():
        print(f"error: {src} not found", file=sys.stderr)
        return 1

    try:
        svg = flatten_document(
            src.read_text(encoding="utf-8"),
            parse_font_map(args.font_map),
            args.keep_class,
            args.no_background,
            args.match_markers,
        )
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    document = '<?xml version="1.0" encoding="UTF-8"?>\n' + svg + "\n"
    if args.stdout:
        sys.stdout.write(document)
        return 0

    dest = Path(args.output) if args.output else src.with_suffix(".svg")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(document, encoding="utf-8")
    print(f"wrote {dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
