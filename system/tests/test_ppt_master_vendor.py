import os
import re
import unittest

from system.tools import ppt_master_contract

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PPT = os.path.join(ROOT, "system", "skills", "ppt-master")


def read(*parts):
    with open(os.path.join(ROOT, *parts), encoding="utf-8") as handle:
        return handle.read()


class TestPptMasterVendorContract(unittest.TestCase):
    def test_confirmation_adapter_pin_matches_vendor_record(self):
        vendor = read("system", "skills", "ppt-master", "VENDOR.md")
        match = re.search(r"`([0-9a-f]{40})` \(pin the commit hash", vendor)
        self.assertIsNotNone(match)
        self.assertEqual(match.group(1), ppt_master_contract.VENDOR_COMMIT)

    def test_confirmation_stage_canary_tracks_executable_vendor_shapes(self):
        app = read(
            "system", "skills", "ppt-master", "scripts", "confirm_ui",
            "static", "app.js",
        )
        server = read(
            "system", "skills", "ppt-master", "scripts", "confirm_ui", "server.py"
        )
        stage_functions = set(
            re.findall(r"function (stage\dPayload|confirm)\s*\(", app)
        )
        self.assertEqual(stage_functions, {"stage1Payload", "stage2Payload", "confirm"})
        self.assertRegex(app, r"JSON\.parse\(JSON\.stringify\(STATE\)\)")
        self.assertIn("normalizeTypographyForSubmit(payload)", app)
        self.assertIn('payload.stage = "final"', app)

        server_added = set(re.findall(r"result\['([^']+)'\]\s*=", server))
        self.assertTrue({"stage", "status", "confirmed_at"}.issubset(server_added))
        self.assertIn("result['stage'] = 'final'", server)
        self.assertIn("result['status'] = 'confirmed'", server)

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
