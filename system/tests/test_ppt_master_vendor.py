import os
import unittest


ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PPT = os.path.join(ROOT, "system", "skills", "ppt-master")


def read(*parts):
    with open(os.path.join(ROOT, *parts), encoding="utf-8") as handle:
        return handle.read()


class TestPptMasterVendorContract(unittest.TestCase):
    def test_upstream_documentation_dependencies_are_vendored(self):
        required = (
            ("system", "docs", "technical-design.md"),
            ("system", "docs", "templates-architecture.md"),
            ("system", "docs", "templates-guide.md"),
            ("system", "docs", "zh", "technical-design.md"),
            ("system", "docs", "zh", "templates-architecture.md"),
            ("system", "docs", "zh", "templates-guide.md"),
        )
        for parts in required:
            with self.subTest(path="/".join(parts)):
                self.assertTrue(os.path.isfile(os.path.join(ROOT, *parts)))

    def test_native_slide_background_promotion_remains_upstream_owned(self):
        source = read(
            "system", "skills", "ppt-master", "scripts", "svg_to_pptx",
            "drawingml", "converter.py",
        )
        self.assertIn("def _extract_background_candidate", source)
        self.assertIn("<p:bg><p:bgPr>", source)
        self.assertIn("background_skip_id", source)

    def test_typography_drift_remains_upstream_checked(self):
        checker = read(
            "system", "skills", "ppt-master", "scripts", "svg_quality_checker.py"
        )
        executor = read(
            "system", "skills", "ppt-master", "references", "executor-base.md"
        )
        self.assertIn("def _check_spec_lock_drift", checker)
        self.assertIn("font-family value(s)", checker)
        self.assertIn("Font family from `typography`", executor)
        self.assertIn("consistent size deck-wide", executor)

    def test_agentframe_does_not_mirror_vendor_routing_or_svg_guidance(self):
        overlay = read("system", "skills", "ppt-master", "AGENTS.md")
        process = read("library", "process", "deck-production.md")
        self.assertNotIn("Dedicated session only", overlay)
        self.assertNotIn("Paragraph authoring", overlay)
        self.assertNotIn("## PPT Master workflow selection", process)
        self.assertIn("workflows/routing.md", overlay)
        self.assertIn("workflows/routing.md", process)


if __name__ == "__main__":
    unittest.main()
