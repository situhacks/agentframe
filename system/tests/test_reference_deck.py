import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from system.tools import reference_deck


PRESENTATION_XML = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
 xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
 xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
 <p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst>
 <p:sldSz cx="12192000" cy="6858000"/>
</p:presentation>
"""

PRESENTATION_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>
"""

EMPTY_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>
"""


def shape(shape_id, text, *, x=100, fill="112233", name="Body"):
    return f"""
    <p:sp>
      <p:nvSpPr><p:cNvPr id="{shape_id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
      <p:spPr>
        <a:xfrm><a:off x="{x}" y="200"/><a:ext cx="300" cy="400"/></a:xfrm>
        <a:solidFill><a:srgbClr val="{fill}"/></a:solidFill>
      </p:spPr>
      <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody>
    </p:sp>
    """


def group_shape(text):
    return f"""
    <p:grpSp>
      <p:nvGrpSpPr><p:cNvPr id="4" name="Diagram Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="1000"/>
        <a:chOff x="0" y="0"/><a:chExt cx="1000" cy="1000"/></a:xfrm></p:grpSpPr>
      {shape(5, text, name="Grouped Label")}
    </p:grpSp>
    """


def write_deck(path, *, text="Keep this text", x=100, fill="112233", extra=False,
               replacement_shape_id=2, grouped=False):
    shapes = (
        group_shape(text)
        if grouped
        else shape(replacement_shape_id, text, x=x, fill=fill)
    )
    if extra:
        shapes += shape(3, "Added shape", x=600, name="Extra")
    slide_xml = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
     xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
     xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
      <p:cSld><p:spTree>{shapes}</p:spTree></p:cSld>
    </p:sld>
    """
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("ppt/presentation.xml", PRESENTATION_XML)
        archive.writestr("ppt/_rels/presentation.xml.rels", PRESENTATION_RELS)
        archive.writestr("ppt/slides/slide1.xml", slide_xml)
        archive.writestr("ppt/slides/_rels/slide1.xml.rels", EMPTY_RELS)


class TestReferenceDeck(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        self.source = self.root / "source.pptx"
        write_deck(self.source)
        self.project = self.root / "project"
        self.manifest_path = reference_deck.scaffold(self.source, self.project)

    def tearDown(self):
        self.temp_dir.cleanup()

    def manifest(self):
        return json.loads(self.manifest_path.read_text(encoding="utf-8"))

    def write_manifest(self, manifest):
        self.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    def test_scaffold_is_preserve_by_default_and_copies_source(self):
        manifest = self.manifest()
        self.assertEqual(manifest["schema"], "agentframe.reference-redesign.v1")
        self.assertEqual(manifest["slides"][0]["mode"], "preserve")
        self.assertEqual(manifest["slides"][0]["shape_count"], 1)
        self.assertTrue((self.project / "reference/source.pptx").is_file())
        self.assertTrue((self.project / "working/working.pptx").is_file())

    def test_preserve_rejects_style_change(self):
        candidate = self.root / "candidate.pptx"
        write_deck(candidate, fill="FFFFFF")
        report = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertFalse(report["ok"])
        self.assertIn("preserve slide XML changed", report["errors"][0])

    def test_verify_rejects_a_different_source_deck(self):
        different_source = self.root / "different-source.pptx"
        write_deck(different_source, text="Different source")
        with self.assertRaisesRegex(reference_deck.ReferenceDeckError, "source_sha256"):
            reference_deck.verify(
                different_source,
                self.project / "working/working.pptx",
                self.manifest_path,
            )

    def test_native_allows_style_only_change(self):
        candidate = self.root / "candidate.pptx"
        write_deck(candidate, fill="FFFFFF")
        manifest = self.manifest()
        manifest["slides"][0]["mode"] = "native"
        self.write_manifest(manifest)
        report = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertTrue(report["ok"], report["errors"])

    def test_native_text_change_requires_shape_allowlist(self):
        candidate = self.root / "candidate.pptx"
        write_deck(candidate, text="Approved replacement")
        manifest = self.manifest()
        manifest["slides"][0]["mode"] = "native"
        self.write_manifest(manifest)
        denied = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertFalse(denied["ok"])
        self.assertIn("text changed without approval", denied["errors"][0])

        manifest["slides"][0]["allow_text_shape_ids"] = [2]
        self.write_manifest(manifest)
        allowed = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertTrue(allowed["ok"], allowed["errors"])

    def test_native_geometry_change_requires_shape_allowlist(self):
        candidate = self.root / "candidate.pptx"
        write_deck(candidate, x=900)
        manifest = self.manifest()
        manifest["slides"][0]["mode"] = "native"
        self.write_manifest(manifest)
        denied = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertFalse(denied["ok"])

        manifest["slides"][0]["allow_geometry_shape_ids"] = [2]
        self.write_manifest(manifest)
        allowed = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertTrue(allowed["ok"], allowed["errors"])

    def test_native_rejects_added_shape(self):
        candidate = self.root / "candidate.pptx"
        write_deck(candidate, extra=True)
        manifest = self.manifest()
        manifest["slides"][0]["mode"] = "native"
        self.write_manifest(manifest)
        report = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertFalse(report["ok"])
        self.assertTrue(any("unapproved shape 3" in item for item in report["errors"]))

    def test_group_child_text_allowlist_does_not_require_group_id(self):
        grouped_source = self.root / "grouped-source.pptx"
        grouped_candidate = self.root / "grouped-candidate.pptx"
        grouped_project = self.root / "grouped-project"
        write_deck(grouped_source, text="Original label", grouped=True)
        write_deck(grouped_candidate, text="Updated label", grouped=True)
        grouped_manifest_path = reference_deck.scaffold(grouped_source, grouped_project)
        manifest = json.loads(grouped_manifest_path.read_text(encoding="utf-8"))
        manifest["slides"][0]["mode"] = "native"
        manifest["slides"][0]["allow_text_shape_ids"] = [5]
        grouped_manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        report = reference_deck.verify(
            grouped_source, grouped_candidate, grouped_manifest_path
        )
        self.assertTrue(report["ok"], report["errors"])

    def test_rebuild_requires_non_exempt_source_text_as_native_text(self):
        candidate = self.root / "candidate.pptx"
        write_deck(candidate, text="Different content", replacement_shape_id=8)
        manifest = self.manifest()
        manifest["slides"][0]["mode"] = "rebuild"
        self.write_manifest(manifest)
        denied = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertFalse(denied["ok"])
        self.assertTrue(any("dropped native source text" in item for item in denied["errors"]))

        write_deck(candidate, text="Keep this text", replacement_shape_id=8)
        allowed = reference_deck.verify(self.source, candidate, self.manifest_path)
        self.assertTrue(allowed["ok"], allowed["errors"])


if __name__ == "__main__":
    unittest.main()
