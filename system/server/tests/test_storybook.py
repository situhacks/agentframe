"""Storybook generator: happy path + loud-failure on schema drift."""

from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from system.server.lib import storybook


VALID_TOKENS = textwrap.dedent(
    """\
    meta:
      campaign: demo-campaign
      version: 2
      category: "AI & LLM"
      summary: "A test language."
    palette:
      base-white: "#ffffff"
      accent-teal: "#87c7c0"
    type:
      google_fonts_import: "https://fonts.example/x"
      primary:
        family: "Inter"
        weights: [400, 700]
        sizes:
          headline: 96
          body: 16
      mono:
        family: "JetBrains Mono"
        weights: [400]
    canvas:
      width: 1080
      height: 1350
      aspect: "4:5"
      safe_margin: 80
    anti_patterns:
      - "no drop shadows"
    """
)

VALID_MD = textwrap.dedent(
    """\
    ---
    status: ready
    storybook_samples: []
    ---

    # Design Language — Demo

    **One-line:** Cool considered-builder register, HTML-native.

    ## Palette
    ...
    """
)


def _folder(tmp_path: Path, tokens: str = VALID_TOKENS, md: str = VALID_MD,
            md_name: str = "design-language-v2.md") -> Path:
    dl = tmp_path / "design-language"
    dl.mkdir()
    (dl / "tokens.yaml").write_text(tokens, encoding="utf-8")
    (dl / md_name).write_text(md, encoding="utf-8")
    # tokens.css is linked relatively; presence not required to generate.
    return dl


def test_happy_path_writes_storybook(tmp_path):
    dl = _folder(tmp_path)
    out = storybook.generate(dl)
    assert out.exists()
    text = out.read_text(encoding="utf-8")
    assert "Design Storybook" in text
    assert "--base-white" in text  # swatch references the token var
    assert "#87c7c0" in text
    assert "Inter" in text
    assert "Cool considered-builder register" in text  # mood extracted
    assert 'href="assets/tokens.css"' in text  # relative link, not inlined
    assert "no drop shadows" in text  # anti-pattern


def test_picks_highest_version_md(tmp_path):
    dl = _folder(tmp_path, md_name="design-language-v1.md")
    (dl / "design-language-v3.md").write_text(
        VALID_MD.replace("Cool considered-builder", "V3 mood wins"),
        encoding="utf-8",
    )
    out = storybook.generate(dl)
    assert "V3 mood wins" in out.read_text(encoding="utf-8")


def test_missing_required_token_key_fails_loudly(tmp_path):
    bad = VALID_TOKENS.replace("canvas:\n", "canvasX:\n")
    dl = _folder(tmp_path, tokens=bad)
    with pytest.raises(storybook.SchemaError) as e:
        storybook.generate(dl)
    assert "canvas" in str(e.value)
    assert not (dl / "preview" / "storybook.html").exists()  # no partial file


def test_type_role_without_family_fails_loudly(tmp_path):
    import re
    bad = re.sub(r'\n\s*family: "Inter"', "", VALID_TOKENS, count=1)
    dl = _folder(tmp_path, tokens=bad)
    with pytest.raises(storybook.SchemaError) as e:
        storybook.generate(dl)
    assert "family" in str(e.value)


def test_missing_mood_fails_loudly(tmp_path):
    md = textwrap.dedent(
        """\
        ---
        status: ready
        ---

        # Design Language — Demo

        ## Palette
        > only a blockquote and headings, no prose
        """
    )
    dl = _folder(tmp_path, md=md)
    with pytest.raises(storybook.SchemaError) as e:
        storybook.generate(dl)
    assert "mood" in str(e.value).lower()


def test_missing_design_language_md_fails_loudly(tmp_path):
    dl = tmp_path / "design-language"
    dl.mkdir()
    (dl / "tokens.yaml").write_text(VALID_TOKENS, encoding="utf-8")
    with pytest.raises(storybook.SchemaError) as e:
        storybook.generate(dl)
    assert "design-language-v" in str(e.value)


def test_missing_tokens_fails_loudly(tmp_path):
    dl = tmp_path / "design-language"
    dl.mkdir()
    (dl / "design-language-v1.md").write_text(VALID_MD, encoding="utf-8")
    with pytest.raises(storybook.SchemaError) as e:
        storybook.generate(dl)
    assert "tokens.yaml" in str(e.value)


def test_samples_from_frontmatter_embedded(tmp_path):
    md = VALID_MD.replace(
        "storybook_samples: []",
        "storybook_samples:\n  - preview/direction-2.html",
    )
    dl = _folder(tmp_path, md=md)
    (dl / "preview").mkdir(exist_ok=True)
    (dl / "preview" / "direction-2.html").write_text("<i>x</i>", encoding="utf-8")
    out = storybook.generate(dl)
    text = out.read_text(encoding="utf-8")
    assert "direction-2.html" in text
    assert "<iframe" in text
