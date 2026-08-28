"""Contract tests for `system/tools/diagram_profile.py`.

The projection turns a design language's `tokens.yaml` into a Diagram Design
client profile. What matters is that no shipped default survives into a
projected profile: a leftover token or typeface is the exact failure the
projection exists to prevent, and it is invisible until a diagram lands next to
a slide in the real identity.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

from diagram_profile import (  # noqa: E402
    ProjectionError,
    SHIPPED_GUIDE,
    build_profile,
    build_roles,
    font_families,
    invert_roles,
)

LANGUAGE = "editorial-deloitte-digital"

TOKENS = {
    "meta": {"summary": "Test identity"},
    "palette": {
        "canvas": "#F6FAEC",
        "canvas-warm": "#ECF1DE",
        "forest": "#122D12",
        "forest-deep": "#0C1F0C",
        "volt": "#4EFF3A",
        "volt-deep": "#1E8F0C",
        "chalk": "#F3F7E5",
        "ink": "#122D12",
        "moss-muted": "#54634A",
        "sage-muted": "#8CA07E",
        "line-dark": "#2E4A26",
        "line-light": "#C7CFB4",
    },
    "palette_roles": {
        "background": "canvas",
        "foreground": "ink",
        "secondary": "moss-muted",
        "accent": "volt-deep",
        "surface": "canvas-warm",
        "divider": "line-light",
    },
    "type": {
        "display": {"family": "Aptos Display", "fallback": ["Aptos", "Calibri"]},
        "body": {"family": "Aptos", "fallback": ["Calibri"]},
        "mono": {"family": "Cascadia Code", "fallback": ["Consolas"]},
    },
    "grounds": {
        "light": {"bg": "canvas", "text": "ink", "muted": "moss-muted", "divider": "line-light", "accent": "volt-deep"},
        "dark": {"bg": "forest", "text": "chalk", "muted": "sage-muted", "divider": "line-dark", "accent": "volt"},
    },
}

SHIPPED_TOKENS = ("#f5f5f5", "#2d3142", "#eb6c36", "#4f5d75", "#7a8399", "#2e5aa8")
SHIPPED_FONTS = ("Instrument Serif", "Geist")


def skinned_sections(profile: str) -> str:
    """The part of the guide a projection owns.

    The terminal skin and the series palette are deliberately excluded: the
    vendor declares both as fixed, opt-in alternates that onboarding does not
    touch, so their shipped hexes surviving is correct, not a leak.
    """
    start = profile.index("### Semantic roles")
    end = profile.index("### Series palette")
    typography = profile.index("## Typography")
    return profile[start:end] + profile[typography:]


class TestRoleProjection(unittest.TestCase):
    def test_light_roles_come_from_the_light_ground(self):
        roles = build_roles(TOKENS, "light")
        self.assertEqual(roles["paper"], "#F6FAEC")
        self.assertEqual(roles["ink"], "#122D12")
        self.assertEqual(roles["accent"], "#1E8F0C")
        self.assertEqual(roles["paper-2"], "#ECF1DE")

    def test_dark_roles_come_from_the_dark_ground(self):
        roles = build_roles(TOKENS, "dark")
        self.assertEqual(roles["paper"], "#122D12")
        self.assertEqual(roles["ink"], "#F3F7E5")
        self.assertEqual(roles["accent"], "#4EFF3A")

    def test_dark_surface_is_not_the_light_surface(self):
        """`palette_roles.surface` is light-mode; reusing it puts a pale panel
        on a dark canvas."""
        light = build_roles(TOKENS, "light")
        dark = build_roles(TOKENS, "dark")
        self.assertNotEqual(dark["paper-2"], light["paper-2"])

    def test_every_role_is_populated(self):
        expected = {
            "paper", "paper-2", "ink", "muted", "soft",
            "rule", "rule-solid", "accent", "accent-tint", "link",
        }
        self.assertEqual(set(build_roles(TOKENS, "light")), expected)

    def test_light_only_language_falls_back_to_inversion(self):
        light_only = {k: v for k, v in TOKENS.items() if k != "grounds"}
        self.assertEqual(build_roles(light_only, "dark"), {})
        dark = invert_roles(build_roles(light_only, "light"))
        self.assertEqual(dark["paper"], "#122D12")
        self.assertEqual(dark["ink"], "#F6FAEC")

    def test_unresolvable_language_raises_rather_than_inventing(self):
        broken = {"palette": {"a": "#111111"}, "palette_roles": {}, "type": TOKENS["type"]}
        with self.assertRaises(ProjectionError):
            build_roles(broken, "light")

    def test_missing_typography_raises(self):
        with self.assertRaises(ProjectionError):
            font_families({"type": {}})


class TestProfileDocument(unittest.TestCase):
    def setUp(self):
        self.profile = build_profile(LANGUAGE, TOKENS, SHIPPED_GUIDE.read_text(encoding="utf-8"), "Test identity")

    def test_header_is_present_and_single(self):
        self.assertTrue(self.profile.startswith("<!-- diagram-design-profile"))
        self.assertEqual(self.profile.count("<!-- diagram-design-profile"), 1)

    def test_reprojection_does_not_stack_headers(self):
        again = build_profile(LANGUAGE, TOKENS, self.profile, "Test identity")
        self.assertEqual(again.count("<!-- diagram-design-profile"), 1)

    def test_no_shipped_default_token_survives(self):
        skinned = skinned_sections(self.profile)
        for token in SHIPPED_TOKENS:
            self.assertNotIn(token, skinned, f"shipped default {token} leaked into the profile")

    def test_no_shipped_default_typeface_survives(self):
        skinned = skinned_sections(self.profile)
        for font in SHIPPED_FONTS:
            self.assertNotIn(font, skinned, f"shipped default font {font} leaked into the profile")

    def test_terminal_skin_is_deliberately_preserved(self):
        """The vendor declares it a fixed alternate onboarding never touches."""
        self.assertIn("`terminal-accent` | `#ff5a36`", self.profile)

    def test_backend_node_uses_the_language_surface_not_white(self):
        self.assertIn("| `backend` | `paper-2` |", self.profile)

    def test_language_tokens_are_present(self):
        for token in ("#F6FAEC", "#1E8F0C", "#122D12"):
            self.assertIn(token, self.profile)

    def test_provenance_points_at_the_repo(self):
        self.assertIn(f"library/assets/design-languages/{LANGUAGE}/tokens.yaml", self.profile)

    def test_font_stack_names_the_language_families(self):
        self.assertIn("Aptos Display", self.profile)
        self.assertIn("Cascadia Code", self.profile)

    def test_webfont_import_used_when_the_language_declares_one(self):
        with_import = dict(TOKENS)
        with_import["type"] = dict(TOKENS["type"], google_fonts_import="https://example.test/css2?family=X")
        profile = build_profile(LANGUAGE, with_import, SHIPPED_GUIDE.read_text(encoding="utf-8"), "x")
        self.assertIn("https://example.test/css2?family=X", profile)


class TestRealLanguage(unittest.TestCase):
    """The repo's own design language must project without hand-holding."""

    def test_shipped_language_projects(self):
        language_dir = ROOT / "library" / "assets" / "design-languages" / LANGUAGE
        if not (language_dir / "tokens.yaml").exists():
            self.skipTest(f"{LANGUAGE} not present in this instance")
        import yaml

        tokens = yaml.safe_load((language_dir / "tokens.yaml").read_text(encoding="utf-8"))
        profile = build_profile(LANGUAGE, tokens, SHIPPED_GUIDE.read_text(encoding="utf-8"), "real")
        skinned = skinned_sections(profile)
        for token in SHIPPED_TOKENS:
            self.assertNotIn(token, skinned)
        for font in SHIPPED_FONTS:
            self.assertNotIn(font, skinned)
        self.assertIn("#F6FAEC", profile)


if __name__ == "__main__":
    unittest.main()
