"""Contract tests for `system/tools/diagram_flatten.py`.

The flattener translates how a Diagram Design value is expressed; it never
changes what the diagram shows. These tests pin the translations PPT Master's
converter requires, plus the shipped-asset regression that catches an upstream
change to an SVG emission convention.
"""

from __future__ import annotations

import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

from diagram_flatten import (  # noqa: E402
    extract_svg,
    flatten_document,
    match_marker_colors,
    normalize_color,
    normalize_dasharray,
)

ASSETS = ROOT / "system" / "skills" / "diagram-design" / "assets"


def document(style: str, body: str, view_box: str = "0 0 100 50") -> str:
    return (
        "<!DOCTYPE html><html><head><style>" + style + "</style></head><body>"
        f'<svg viewBox="{view_box}">' + body + "</svg></body></html>"
    )


class TestValueTranslation(unittest.TestCase):
    def test_custom_property_resolves_to_hex(self):
        out = flatten_document(
            document(":root { --accent: #eb6c36; }", '<rect fill="var(--accent)"/>')
        )
        self.assertIn('fill="#EB6C36"', out)
        self.assertNotIn("var(", out)

    def test_custom_property_fallback_is_honoured(self):
        out = flatten_document(document(":root {}", '<rect fill="var(--missing, #123456)"/>'))
        self.assertIn('fill="#123456"', out)

    def test_class_rule_is_inlined_and_class_dropped(self):
        out = flatten_document(
            document(".node { fill: #fff; stroke: #000; }", '<rect class="node"/>')
        )
        self.assertIn('fill="#FFFFFF"', out)
        self.assertIn('stroke="#000000"', out)
        self.assertNotIn("class=", out)

    def test_css_rule_beats_presentation_attribute(self):
        """A browser resolves it this way; the converter reads attributes only."""
        out = flatten_document(
            document(".node { fill: #111111; }", '<rect class="node" fill="#999999"/>')
        )
        self.assertIn('fill="#111111"', out)
        self.assertNotIn("#999999", out)

    def test_inline_style_beats_css_rule(self):
        out = flatten_document(
            document(".node { fill: #111111; }", '<rect class="node" style="fill: #222222"/>')
        )
        self.assertIn('fill="#222222"', out)

    def test_more_specific_selector_wins(self):
        out = flatten_document(
            document(
                ".station { fill: #111111; } .station.focal { fill: #222222; }",
                '<rect class="station focal"/>',
            )
        )
        self.assertIn('fill="#222222"', out)

    def test_rgba_splits_into_colour_and_channel_opacity(self):
        out = flatten_document(document("", '<rect fill="rgba(45,49,66,0.10)"/>'))
        self.assertIn('fill="#2D3142"', out)
        self.assertIn('fill-opacity="0.1"', out)

    def test_font_shorthand_expands(self):
        out = flatten_document(
            document(".t { font: 600 12px 'Geist', sans-serif; }", '<text class="t">x</text>')
        )
        self.assertIn('font-weight="600"', out)
        self.assertIn('font-size="12"', out)
        self.assertIn("font-family=", out)

    def test_em_letter_spacing_resolves_against_font_size(self):
        out = flatten_document(
            document(
                ".e { font: 400 9px monospace; letter-spacing: 0.18em; }",
                '<text class="e">x</text>',
            )
        )
        self.assertIn('letter-spacing="1.62"', out)

    def test_percentage_geometry_becomes_canvas_units(self):
        out = flatten_document(
            document("", '<rect width="100%" height="100%" fill="#fff"/>', "0 0 1280 720")
        )
        self.assertIn('width="1280"', out)
        self.assertIn('height="720"', out)

    def test_transparent_becomes_none(self):
        out = flatten_document(document("", '<rect fill="transparent"/>'))
        self.assertIn('fill="none"', out)

    def test_named_colour_becomes_hex(self):
        out = flatten_document(document("", '<text fill="white">x</text>'))
        self.assertIn('fill="#FFFFFF"', out)

    def test_current_color_resolves_to_document_colour(self):
        out = flatten_document(
            document(
                "body { color: #2d3142; } ",
                '<g stroke="currentColor"><path d="M0 0"/></g>',
            )
        )
        self.assertIn('stroke="#2D3142"', out)
        self.assertNotIn("currentColor", out)

    def test_single_value_dasharray_becomes_a_pair(self):
        self.assertEqual(normalize_dasharray("1"), "1 1")
        self.assertEqual(normalize_dasharray("5 4"), "5 4")
        self.assertEqual(normalize_dasharray("none"), "none")

    def test_hex_shorthand_expands(self):
        self.assertEqual(normalize_color("#abc"), ("#AABBCC", None))


class TestStructuralTranslation(unittest.TestCase):
    def test_nested_svg_becomes_a_transform_group(self):
        out = flatten_document(
            document(
                "",
                '<svg x="72" y="66" width="24" height="24" viewBox="0 0 24 24" '
                'aria-hidden="true" stroke="#2d3142"><path d="M0 0"/></svg>',
            )
        )
        self.assertIn("translate(72 66)", out)
        self.assertNotIn("aria-hidden", out)
        self.assertEqual(out.count("<svg"), 1)

    def test_nested_svg_scales_when_viewbox_differs(self):
        out = flatten_document(
            document(
                "",
                '<svg x="0" y="0" width="48" height="48" viewBox="0 0 24 24">'
                '<path d="M0 0"/></svg>',
            )
        )
        self.assertIn("scale(2 2)", out)

    def test_extract_svg_keeps_nested_children(self):
        html = document("", '<svg width="24" height="24"><path d="M0 0"/></svg><rect/>')
        svg = extract_svg(html)
        self.assertIn("<rect/>", svg)
        self.assertEqual(svg.count("</svg>"), 2)

    def test_valueless_attribute_becomes_empty_valued(self):
        out = flatten_document(document("", '<g data-polar-chart data-cx="500"/>'))
        self.assertIn('data-polar-chart=""', out)
        ET.fromstring(out)

    def test_comments_containing_double_hyphen_are_removed(self):
        out = flatten_document(document("", '<!-- focal -- and worse --><rect/>'))
        self.assertNotIn("<!--", out)
        ET.fromstring(out)

    def test_ampersand_in_attribute_is_escaped_once(self):
        out = flatten_document(document("", '<text data-note="a &amp; b">x</text>'))
        ET.fromstring(out)
        self.assertNotIn("&amp;amp;", out)

    def test_full_canvas_background_dropped_only_when_requested(self):
        body = '<rect width="100%" height="100%" fill="#f5f5f5"/><rect x="10" y="10" width="20" height="20"/>'
        kept = flatten_document(document("", body, "0 0 100 50"))
        dropped = flatten_document(document("", body, "0 0 100 50"), drop_background=True)
        self.assertIn("#F5F5F5", kept)
        self.assertNotIn("#F5F5F5", dropped)
        self.assertIn('width="20"', dropped)

    def test_font_map_substitutes_family(self):
        out = flatten_document(
            document(".t { font-family: 'Geist Mono', monospace; }", '<text class="t">x</text>'),
            font_map={"Geist Mono": "Consolas"},
        )
        self.assertIn('font-family="Consolas"', out)

    def test_missing_svg_raises(self):
        with self.assertRaises(ValueError):
            flatten_document("<html><body><p>no diagram</p></body></html>")


class TestMarkerMatching(unittest.TestCase):
    def test_shared_marker_clones_per_stroke_colour(self):
        svg = (
            '<svg viewBox="0 0 100 50"><defs>'
            '<marker id="tri"><polygon points="0 0" fill="#F5F5F5"/></marker>'
            "</defs>"
            '<line stroke="#2D3142" marker-end="url(#tri)"/>'
            '<line stroke="#EB6C36" marker-end="url(#tri)"/>'
            "</svg>"
        )
        out, changed = match_marker_colors(svg)
        self.assertEqual(changed, 2)
        self.assertIn('id="tri-2d3142"', out)
        self.assertIn('id="tri-eb6c36"', out)
        ET.fromstring(out)

    def test_marker_already_matching_is_left_alone(self):
        svg = (
            '<svg viewBox="0 0 100 50"><defs>'
            '<marker id="a"><polygon points="0 0" fill="#2D3142"/></marker>'
            "</defs>"
            '<line stroke="#2D3142" marker-end="url(#a)"/></svg>'
        )
        out, changed = match_marker_colors(svg)
        self.assertEqual(changed, 0)
        self.assertEqual(out, svg)

    def test_fill_none_is_not_repainted(self):
        svg = (
            '<svg viewBox="0 0 100 50"><defs>'
            '<marker id="a"><path d="M0 0" fill="none" stroke="#999999"/></marker>'
            "</defs>"
            '<line stroke="#2D3142" marker-end="url(#a)"/></svg>'
        )
        out, _ = match_marker_colors(svg)
        self.assertIn('fill="none"', out)


class TestShippedAssets(unittest.TestCase):
    """Upstream regression: every shipped diagram must still flatten to valid XML.

    A failure here means the vendored skill changed an SVG emission convention
    the flattener does not yet handle. See `system/skills/diagram-design/VENDOR.md`.
    """

    def test_every_shipped_asset_flattens_to_wellformed_xml(self):
        assets = sorted(p for p in ASSETS.glob("*.html") if p.name != "index.html")
        self.assertGreater(len(assets), 100, "vendored assets missing")
        failures = []
        for asset in assets:
            try:
                svg = flatten_document(
                    asset.read_text(encoding="utf-8"), drop_background=True, match_markers=True
                )
                ET.fromstring(svg)
            except Exception as exc:  # noqa: BLE001 - reported in bulk below
                failures.append(f"{asset.name}: {type(exc).__name__}: {exc}")
        self.assertEqual(failures, [], "\n".join(failures))

    def test_no_flattened_asset_retains_a_css_dependency(self):
        for name in ("example-architecture.html", "example-loop.html", "example-swimlane.html"):
            out = flatten_document((ASSETS / name).read_text(encoding="utf-8"))
            self.assertNotIn("var(--", out, name)
            self.assertNotIn(" class=", out, name)
            self.assertNotIn('="100%"', out, name)


if __name__ == "__main__":
    unittest.main()
