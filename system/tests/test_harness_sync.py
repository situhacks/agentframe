import json
import tempfile
import unittest
from pathlib import Path

from system import af


class HarnessProjectionTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        skills = ["humanizer", "deep-research", "agentframe-structure"]
        for name in skills:
            folder = self.root / "system" / "skills" / name
            folder.mkdir(parents=True)
            (folder / "SKILL.md").write_text(
                f"---\nname: {name}\ndescription: {name} fixture\n---\n\n# {name}\n",
                encoding="utf-8",
            )
            (folder / "reference.txt").write_text(f"{name} reference\n", encoding="utf-8")
        manifest = {
            "schema_version": 1,
            "canonical_root": "system/skills",
            "skills": skills,
            "targets": {
                "claude": ".claude/skills",
                "codex": ".agents/skills",
                "cursor": ".cursor/skills",
            },
            "overlays": {},
        }
        manifest_path = self.root / af.HARNESS_MANIFEST
        manifest_path.parent.mkdir(parents=True)
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    def tearDown(self):
        self._tmp.cleanup()

    def test_write_generates_complete_hashed_bundles_for_each_harness(self):
        canonical = (
            self.root / "system" / "skills" / "humanizer" / "SKILL.md"
        ).read_bytes()

        self.assertEqual(af.sync_harnesses(root=self.root, write=True), [])

        for target in (".claude/skills", ".agents/skills", ".cursor/skills"):
            projected = self.root / target / "humanizer"
            self.assertEqual((projected / "SKILL.md").read_bytes(), canonical)
            self.assertTrue((projected / "reference.txt").is_file())
            marker = json.loads((projected / af.PROJECTION_MARKER).read_text())
            self.assertTrue(marker["do_not_edit"])
            self.assertEqual(marker["source"], "system/skills/humanizer")
            self.assertIn("SKILL.md", marker["files"])
            self.assertTrue((self.root / target / af.PROJECTION_MANIFEST).is_file())
        self.assertEqual(af.sync_harnesses(root=self.root, write=False), [])

    def test_check_reports_projection_drift_without_repairing_it(self):
        af.sync_harnesses(root=self.root, write=True)
        drifted = self.root / ".agents" / "skills" / "humanizer" / "SKILL.md"
        drifted.write_text("locally edited\n", encoding="utf-8")

        issues = af.sync_harnesses(root=self.root, write=False)

        self.assertTrue(any("drifted generated file" in issue for issue in issues))
        self.assertEqual(drifted.read_text(encoding="utf-8"), "locally edited\n")

    def test_check_ignores_utf8_text_line_ending_differences(self):
        af.sync_harnesses(root=self.root, write=True)
        canonical = self.root / "system" / "skills" / "humanizer" / "SKILL.md"
        canonical.write_bytes(af._projection_bytes(canonical).replace(b"\n", b"\r\n"))

        self.assertEqual(af.sync_harnesses(root=self.root, write=False), [])

    def test_check_ignores_crlf_in_the_root_projection_manifest(self):
        # The per-file comparison was EOL-insensitive while the root manifest
        # was still compared raw, so a CRLF checkout reported manifest-only drift.
        af.sync_harnesses(root=self.root, write=True)
        projected = self.root / ".claude" / "skills" / af.PROJECTION_MANIFEST
        projected.write_bytes(af._projection_bytes(projected).replace(b"\n", b"\r\n"))

        self.assertEqual(af.sync_harnesses(root=self.root, write=False), [])

    def test_check_still_reports_a_genuinely_drifted_root_manifest(self):
        af.sync_harnesses(root=self.root, write=True)
        projected = self.root / ".claude" / "skills" / af.PROJECTION_MANIFEST
        projected.write_text('{"skills": {}}\n', encoding="utf-8")

        issues = af.sync_harnesses(root=self.root, write=False)

        self.assertTrue(any("drifted projection manifest" in issue for issue in issues))

    def test_projection_bytes_keep_binary_differences_exact(self):
        left = self.root / "left.bin"
        right = self.root / "right.bin"
        left.write_bytes(b"\xff\r\n")
        right.write_bytes(b"\xff\n")

        self.assertNotEqual(af._projection_bytes(left), af._projection_bytes(right))

    def test_write_rejects_foreign_skill_directory(self):
        foreign = self.root / ".cursor" / "skills" / "humanizer"
        foreign.mkdir(parents=True)
        (foreign / "SKILL.md").write_text("user skill\n", encoding="utf-8")

        with self.assertRaisesRegex(ValueError, "non-generated skill directory"):
            af.sync_harnesses(root=self.root, write=True)

        self.assertEqual((foreign / "SKILL.md").read_text(encoding="utf-8"), "user skill\n")

    def test_write_replaces_only_managed_skills(self):
        unrelated = self.root / ".agents" / "skills" / "personal-skill"
        unrelated.mkdir(parents=True)
        (unrelated / "SKILL.md").write_text("keep me\n", encoding="utf-8")

        af.sync_harnesses(root=self.root, write=True)
        af.sync_harnesses(root=self.root, write=True)

        self.assertEqual((unrelated / "SKILL.md").read_text(encoding="utf-8"), "keep me\n")


if __name__ == "__main__":
    unittest.main()
