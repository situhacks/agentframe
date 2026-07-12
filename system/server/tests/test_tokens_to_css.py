"""Locks the canonical tokens.yaml -> CSS mapping.

Schema is the shape real project tokens.yaml files use: palette,
palette_roles, type.{role}, canvas. Project-specific sections must be
ignored, not emitted.
"""

from __future__ import annotations

from system.server.lib.tokens_to_css import to_css


CANONICAL = {
    "meta": {"campaign": "demo", "version": 2},
    "palette": {
        "base-white": "#ffffff",
        "accent-teal": "#87c7c0",
    },
    "palette_roles": {
        "background": "base-white",
        "system_accent": "accent-teal",
        "risograph_spots": ["accent-teal", "base-white"],  # list -> skipped
    },
    "type": {
        "google_fonts_import": "https://fonts.example/x",  # scalar -> skipped
        "primary": {
            "family": "Inter",
            "weights": [400, 600, 700],
            "line_height": 1.6,
            "sizes": {"headline_hero": 96, "body": 16},
        },
        "mono": {
            "family": "JetBrains Mono",
            "weights": [400],
        },
    },
    "canvas": {
        "width": 1080,
        "height": 1350,
        "aspect": "4:5",
        "safe_margin": 80,
        "grid_columns": 12,
    },
    # Project extensions — must NOT appear in CSS.
    "highlighter": {"pink-rgba": "rgba(1,2,3,0.5)"},
    "nano-banana": {"role": "props"},
    "emphasis": {"brackets": {"rule": "x"}},
}


def test_palette_maps_to_flat_vars():
    css = to_css(CANONICAL)
    assert "--base-white: #ffffff;" in css
    assert "--accent-teal: #87c7c0;" in css


def test_palette_roles_reference_palette_vars():
    css = to_css(CANONICAL)
    assert "--role-background: var(--base-white);" in css
    assert "--role-system_accent: var(--accent-teal);" in css


def test_palette_role_list_value_is_skipped():
    css = to_css(CANONICAL)
    assert "--role-risograph_spots" not in css


def test_type_role_family_weights_lineheight():
    css = to_css(CANONICAL)
    assert '--font-primary: "Inter";' in css
    assert "--fw-primary: 400;" in css  # first weight is the default
    assert "--lh-primary: 1.6;" in css
    assert '--font-mono: "JetBrains Mono";' in css


def test_type_sizes_emit_px():
    css = to_css(CANONICAL)
    assert "--size-primary-headline_hero: 96px;" in css
    assert "--size-primary-body: 16px;" in css


def test_google_fonts_import_scalar_is_skipped():
    css = to_css(CANONICAL)
    assert "--font-google_fonts_import" not in css


def test_canvas_maps():
    css = to_css(CANONICAL)
    assert "--canvas-w: 1080px;" in css
    assert "--canvas-h: 1350px;" in css
    assert "--canvas-aspect: 4:5;" in css
    assert "--canvas-safe: 80px;" in css
    assert "--grid-cols: 12;" in css


def test_project_extensions_absent():
    css = to_css(CANONICAL)
    assert "highlighter" not in css
    assert "nano-banana" not in css
    assert "rgba(1,2,3" not in css


def test_empty_tokens_yields_valid_root():
    css = to_css({})
    assert ":root {" in css
    assert css.rstrip().endswith("}")
