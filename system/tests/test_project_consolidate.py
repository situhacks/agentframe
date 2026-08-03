import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
HELPER_PATH = (
    REPO_ROOT
    / "system"
    / "skills"
    / "project-consolidate"
    / "scripts"
    / "consolidation_review.py"
)
SPEC = importlib.util.spec_from_file_location("consolidation_review", HELPER_PATH)
review = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(review)


class ConsolidationReviewTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.base = Path(self._tmp.name)
        self.root = self.base / "agentframe"
        (self.root / "workspace" / "projects").mkdir(parents=True)
        (self.root / "library" / "context" / "people").mkdir(parents=True)
        (self.root / ".gitignore").write_text(
            "workspace/projects/\nlibrary/context/people/\n", encoding="utf-8"
        )
        self.addCleanup(self._tmp.cleanup)

    def make_project(self, slug, *, completed=False):
        base = self.root / "workspace" / "projects"
        if completed:
            base = base / "completed"
        project = base / slug
        (project / "knowledge" / "people").mkdir(parents=True)
        (project / "project.md").write_bytes(
            f"---\nslug: {slug}\n---\n# {slug}\r\n".encode("utf-8")
        )
        return project

    def test_sealed_restore_is_visible_exact_and_leaves_unsealed_addition(self):
        project = self.make_project("alpha")
        notes = project / "knowledge" / "notes.md"
        original_project = (project / "project.md").read_bytes()
        original_notes = b"first\r\nsecond\r\n"
        notes.write_bytes(original_notes)
        global_profile = self.root / "library" / "context" / "people" / "sam" / "profile.md"
        global_profile.parent.mkdir()
        original_profile = "Résumé\r\n".encode("utf-8")
        global_profile.write_bytes(original_profile)

        run_dir = review.snapshot_state(
            self.root, "alpha", self.base / "review-run"
        )
        manifest = json.loads(
            (run_dir / review.SNAPSHOT_MANIFEST).read_text(encoding="utf-8")
        )
        self.assertIn(
            "workspace/projects/alpha/project.md",
            manifest["files"],
        )
        self.assertIn(
            "library/context/people/sam/profile.md",
            manifest["files"],
        )

        (project / "project.md").write_bytes(b"changed\n")
        notes.unlink()
        (project / "knowledge" / "added.md").write_bytes(b"new\n")
        concurrent = project / "knowledge" / "concurrent.md"
        concurrent.write_bytes(b"unrelated concurrent work\n")
        global_profile.write_bytes(b"changed globally\n")

        rendered = review.render_review(run_dir)
        self.assertIn("Summary: 2 added, 2 changed, 1 deleted", rendered)
        self.assertIn("workspace/projects/alpha/knowledge/added.md", rendered)
        self.assertIn("workspace/projects/alpha/knowledge/notes.md", rendered)
        self.assertIn("library/context/people/sam/profile.md", rendered)
        self.assertIn("-Résumé", rendered)
        self.assertIn("+changed globally", rendered)

        with patch.object(
            review, "current_state", wraps=review.current_state
        ) as current_state:
            sealed = review.seal_review(
                run_dir,
                [
                    "workspace/projects/alpha/project.md",
                    "workspace/projects/alpha/knowledge/notes.md",
                    "workspace/projects/alpha/knowledge/added.md",
                    "library/context/people/sam/profile.md",
                ],
            )
        self.assertEqual(current_state.call_count, 1)
        self.assertIn("Review state SHA-256:", sealed)
        self.assertIn("Summary: 2 added, 2 changed, 1 deleted", sealed)
        self.assertIn("+unrelated concurrent work", sealed)
        self.assertIn(
            "UNSEALED added workspace/projects/alpha/knowledge/concurrent.md",
            sealed,
        )
        review_manifest = json.loads(
            (run_dir / review.REVIEW_MANIFEST).read_text(encoding="utf-8")
        )
        self.assertEqual(
            review_manifest["snapshot_manifest_sha256"],
            review.sha256_bytes(
                (run_dir / review.SNAPSHOT_MANIFEST).read_bytes()
            ),
        )
        self.assertEqual(
            review_manifest["review_state_sha256"],
            review._review_state_digest(
                review_manifest["snapshot_manifest_sha256"],
                review_manifest["scope_changes"],
            ),
        )
        self.assertEqual(len(review_manifest["scope_changes"]), 5)
        self.assertEqual(
            review_manifest["scope_changes"][
                "workspace/projects/alpha/knowledge/concurrent.md"
            ]["change_kind"],
            "added",
        )
        sealed_added = review_manifest["paths"][
            "workspace/projects/alpha/knowledge/added.md"
        ]
        self.assertEqual(sealed_added["change_kind"], "added")
        self.assertTrue(sealed_added["exists"])
        self.assertEqual(sealed_added["size"], 4)
        sealed_deleted = review_manifest["paths"][
            "workspace/projects/alpha/knowledge/notes.md"
        ]
        self.assertEqual(sealed_deleted["change_kind"], "deleted")
        self.assertFalse(sealed_deleted["exists"])
        self.assertIsNone(sealed_deleted["sha256"])

        concurrent.write_bytes(b"unsealed work changed after review\n")
        with self.assertRaises(review.ReviewError):
            review.verify_review(run_dir)
        with self.assertRaises(review.ReviewError):
            review.restore_snapshot(run_dir)
        self.assertEqual((project / "project.md").read_bytes(), b"changed\n")
        self.assertFalse(notes.exists())
        self.assertTrue((project / "knowledge" / "added.md").exists())
        self.assertEqual(global_profile.read_bytes(), b"changed globally\n")

        concurrent.write_bytes(b"unrelated concurrent work\n")
        verified = review.verify_review(run_dir)
        self.assertIn(
            f"Review state verified: {review_manifest['review_state_sha256']}",
            verified,
        )
        restored, removed = review.restore_snapshot(run_dir)
        self.assertEqual(restored, 3)
        self.assertEqual(removed, 1)
        self.assertEqual((project / "project.md").read_bytes(), original_project)
        self.assertEqual(notes.read_bytes(), original_notes)
        self.assertEqual(global_profile.read_bytes(), original_profile)
        self.assertFalse((project / "knowledge" / "added.md").exists())
        self.assertEqual(concurrent.read_bytes(), b"unrelated concurrent work\n")
        remaining = review.render_review(run_dir)
        self.assertIn("Summary: 1 added, 0 changed, 0 deleted", remaining)
        self.assertIn("workspace/projects/alpha/knowledge/concurrent.md", remaining)

    def test_scope_validation_rejects_escape_and_run_dir_inside_project(self):
        project = self.make_project("alpha")
        with self.assertRaises(review.ReviewError):
            review.snapshot_state(self.root, "../alpha", self.base / "bad-run")
        with self.assertRaises(review.ReviewError):
            review.snapshot_state(self.root, "alpha", project / "review-run")
        self.assertFalse((project / "review-run").exists())
        run_dir = review.snapshot_state(
            self.root, "alpha", self.base / "valid-run"
        )
        with self.assertRaises(review.ReviewError):
            review.seal_review(
                run_dir, ["workspace/projects/alpha/project.md"]
            )

    def test_missing_global_people_directory_is_an_empty_review_scope(self):
        self.make_project("alpha")
        people = self.root / "library" / "context" / "people"
        people.rmdir()

        run_dir = review.snapshot_state(
            self.root, "alpha", self.base / "fresh-copy-run"
        )
        manifest = json.loads(
            (run_dir / review.SNAPSHOT_MANIFEST).read_text(encoding="utf-8")
        )
        self.assertIn("library/context/people", manifest["scopes"])
        self.assertFalse(
            any(
                rel.startswith("library/context/people/")
                for rel in manifest["files"]
            )
        )

        profile = people / "sam" / "profile.md"
        profile.parent.mkdir(parents=True)
        profile.write_bytes(b"# Sam\n")
        rendered = review.render_review(run_dir)
        self.assertIn("Summary: 1 added", rendered)
        self.assertIn("library/context/people/sam/profile.md", rendered)

        review.seal_review(
            run_dir, ["library/context/people/sam/profile.md"]
        )
        restored, removed = review.restore_snapshot(run_dir)
        self.assertEqual((restored, removed), (0, 1))
        self.assertFalse(profile.exists())

    def test_restore_preflights_every_sealed_path_before_mutating_any(self):
        project = self.make_project("alpha")
        first = project / "knowledge" / "first.md"
        second = project / "knowledge" / "second.md"
        first.write_bytes(b"first before\n")
        second.write_bytes(b"second before\n")
        run_dir = review.snapshot_state(
            self.root, "alpha", self.base / "atomic-run"
        )

        first.write_bytes(b"first pass-owned\n")
        second.write_bytes(b"second pass-owned\n")
        review.seal_review(
            run_dir,
            [
                "workspace/projects/alpha/knowledge/first.md",
                "workspace/projects/alpha/knowledge/second.md",
            ],
        )
        second.write_bytes(b"second changed after seal\n")

        with self.assertRaises(review.ReviewError):
            review.verify_review(run_dir)
        with self.assertRaises(review.ReviewError):
            review.restore_snapshot(run_dir)
        self.assertEqual(first.read_bytes(), b"first pass-owned\n")
        self.assertEqual(second.read_bytes(), b"second changed after seal\n")

    def test_people_discovery_includes_active_and_completed_projects(self):
        active = self.make_project("alpha")
        completed = self.make_project("beta", completed=True)
        (active / "knowledge" / "people" / "sam.md").write_text(
            "active", encoding="utf-8"
        )
        (completed / "knowledge" / "people" / "sam.md").write_text(
            "completed", encoding="utf-8"
        )
        (completed / "knowledge" / "people" / "lee.md").write_text(
            "completed only", encoding="utf-8"
        )

        records = review.discover_project_people(self.root)
        locations = {(item["state"], item["project"], item["person"]) for item in records}
        self.assertIn(("active", "alpha", "sam"), locations)
        self.assertIn(("completed", "beta", "sam"), locations)
        self.assertIn(("completed", "beta", "lee"), locations)

        inventory = review.render_people_inventory(self.root, "alpha")
        self.assertIn("active\talpha\tsam", inventory)
        self.assertIn("completed\tbeta\tsam", inventory)
        self.assertNotIn("\tlee\t", inventory)

    def test_promotion_batch_binds_receipt_and_rejects_changed_candidates(self):
        self.make_project("alpha")
        run_dir = review.snapshot_state(
            self.root, "alpha", self.base / "promotion-run"
        )
        candidate = (
            run_dir
            / "promotion-candidates"
            / "library"
            / "context"
            / "people"
            / "sam"
            / "profile.md"
        )
        candidate.parent.mkdir(parents=True)
        candidate.write_bytes(b"# Sam\n")

        staged = review.stage_promotions(run_dir)
        self.assertIn("Promotion batch: 1 candidate(s)", staged)
        self.assertIn("Frozen batch SHA-256:", staged)
        self.assertIn("library/context/people/sam/profile.md", staged)
        with self.assertRaises(review.ReviewError):
            review.apply_promotions(run_dir, "   ")

        candidate.write_bytes(b"# Sam changed after review\n")
        with self.assertRaises(review.ReviewError):
            review.apply_promotions(run_dir, "Yes, approve this promotion.")

        review.stage_promotions(run_dir)
        receipt = "Yes, approve this promotion."
        self.assertEqual(review.apply_promotions(run_dir, receipt), 1)
        promotion_manifest = json.loads(
            (run_dir / review.PROMOTION_MANIFEST).read_text(encoding="utf-8")
        )
        self.assertEqual(promotion_manifest["approval"]["receipt"], receipt)
        self.assertEqual(
            promotion_manifest["approval"]["batch_sha256"],
            promotion_manifest["batch_sha256"],
        )
        self.assertIn(
            "not authenticated",
            promotion_manifest["approval"]["authentication"],
        )
        live = self.root / "library" / "context" / "people" / "sam" / "profile.md"
        self.assertEqual(live.read_bytes(), b"# Sam changed after review\n")
        self.assertIn("Summary: 1 added", review.render_review(run_dir))

        review.seal_review(
            run_dir, ["library/context/people/sam/profile.md"]
        )
        review.restore_snapshot(run_dir)
        self.assertFalse(live.exists())

    def test_approval_receipt_file_is_read_as_utf8_data(self):
        run_dir = self.base / "receipt-run"
        run_dir.mkdir()
        receipt_path = run_dir / "operator-approval.txt"
        receipt_path.write_text(
            "Approve `$()` and other shell-looking text.\n", encoding="utf-8"
        )
        self.assertEqual(
            review.read_approval_receipt(receipt_path, run_dir),
            "Approve `$()` and other shell-looking text.",
        )
        with self.assertRaises(review.ReviewError):
            review.read_approval_receipt(self.base / "other.txt", run_dir)

    def test_skill_requires_a_later_operator_authority_boundary(self):
        skill = (
            REPO_ROOT / "system" / "skills" / "project-consolidate" / "SKILL.md"
        ).read_text(encoding="utf-8-sig")
        self.assertIn("The later operator response is the authority boundary", skill)
        self.assertIn("never infer, synthesize, or pre-fill it", skill)


if __name__ == "__main__":
    unittest.main()
