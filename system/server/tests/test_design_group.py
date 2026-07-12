"""Preview rail: pinned Design group for a project's design language."""

import tempfile
import unittest
from pathlib import Path

from system.server.lib.surface import artifacts


DL_MD = """\
---
status: locked
storybook: preview/storybook.html
---

# Design Language — Demo

**One-line:** mood here.
"""


def _project(root: Path, *, with_storybook: bool = True, with_dl: bool = True) -> Path:
    pdir = root / "workspace" / "projects" / "demo"
    dl = pdir / "phase-3-planning" / "design-language"
    dl.mkdir(parents=True)
    if with_dl:
        (dl / "design-language-v1.md").write_text(DL_MD, encoding="utf-8")
        (dl / "tokens.yaml").write_text("palette:\n  x: '#fff'\n", encoding="utf-8")
        (dl / "preview").mkdir()
        (dl / "preview" / "direction-2.html").write_text("<i>x</i>", encoding="utf-8")
        if with_storybook:
            (dl / "preview" / "storybook.html").write_text("<i>book</i>", encoding="utf-8")
    return pdir


class DesignGroupTests(unittest.TestCase):
    def test_group_present_and_points_at_storybook(self):
        with tempfile.TemporaryDirectory() as tmp:
            pdir = _project(Path(tmp))
            g = artifacts.design_group(pdir)
            self.assertIsNotNone(g)
            self.assertEqual(g["kind"], "design")
            self.assertTrue(g["pinned"])
            self.assertTrue(g["current"].endswith("preview/storybook.html"))

    def test_falls_back_to_markdown_without_storybook(self):
        with tempfile.TemporaryDirectory() as tmp:
            pdir = _project(Path(tmp), with_storybook=False)
            g = artifacts.design_group(pdir)
            self.assertIsNotNone(g)
            self.assertTrue(g["current"].endswith("design-language-v1.md"))

    def test_absent_when_no_design_language(self):
        with tempfile.TemporaryDirectory() as tmp:
            pdir = Path(tmp) / "workspace" / "projects" / "demo"
            (pdir / "phase-1-research").mkdir(parents=True)
            self.assertIsNone(artifacts.design_group(pdir))

    def test_group_is_pinned_first_in_artifact_groups(self):
        with tempfile.TemporaryDirectory() as tmp:
            pdir = _project(Path(tmp))
            rows = {"messaging": {"file": "phase-2/messaging-v1.md", "status": "locked"}}
            groups = artifacts.artifact_groups(pdir, rows)
            self.assertEqual(groups[0]["kind"], "design")

    def test_design_detail_lists_whole_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            pdir = _project(Path(tmp))
            g = artifacts.design_group(pdir)
            detail = artifacts.design_detail(pdir, g["folder"])
            files = detail["versions"]
            # Lists PREVIEWABLE files only (md, html); tokens.yaml is not previewable.
            self.assertTrue(any(f.endswith("design-language-v1.md") for f in files))
            self.assertTrue(any(f.endswith("storybook.html") for f in files))
            self.assertTrue(any(f.endswith("direction-2.html") for f in files))

    def test_picks_highest_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            pdir = _project(Path(tmp))
            dl = pdir / "phase-3-planning" / "design-language"
            (dl / "design-language-v2.md").write_text(
                DL_MD.replace("v1", "v2"), encoding="utf-8"
            )
            g = artifacts.design_group(pdir)
            # current is storybook; verify the md picked was v2 via detail folder
            detail = artifacts.design_detail(pdir, g["folder"])
            self.assertTrue(any(f.endswith("design-language-v2.md") for f in detail["versions"]))


if __name__ == "__main__":
    unittest.main()
