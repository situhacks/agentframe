import os
import tempfile
import textwrap
import unittest
from pathlib import Path

from system.server.lib.surface import artifacts


def write(path, text="x"):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


class ArtifactFixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.pdir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()


class TestArtifactGroups(ArtifactFixture):
    def test_tracker_row_becomes_group_with_stem_version_count(self):
        write(self.pdir / "phase-2/build-brief/build-brief-v1.md")
        write(self.pdir / "phase-2/build-brief/build-brief-v2.md")
        write(self.pdir / "phase-2/build-brief/exports/handover-2026-07-04.pdf")
        rows = {
            "build-brief": {
                "status": "drafting",
                "file": "phase-2/build-brief/build-brief-v2.md",
                "last_updated": "2026-07-04",
            }
        }
        groups = artifacts.artifact_groups(self.pdir, rows)
        self.assertEqual(len(groups), 1)
        g = groups[0]
        self.assertEqual(g["slug"], "build-brief")
        self.assertEqual(g["label"], "Build Brief")
        self.assertEqual(g["current"], "phase-2/build-brief/build-brief-v2.md")
        self.assertEqual(g["version_count"], 2)
        self.assertTrue(g["has_exports"])
        self.assertFalse(g["archived"])
        # summaries stay lazy: no version list until detail is requested
        self.assertNotIn("versions", g)

    def test_multi_stem_folder_groups_are_stem_scoped(self):
        # a post folder: unversioned assembly record + ingredient version trails
        write(self.pdir / "posts/post-1/post-FINAL.md")
        write(self.pdir / "posts/post-1/slide-copy-v1.md")
        write(self.pdir / "posts/post-1/slide-copy-v2.md")
        rows = {
            "post-1": {"status": "published", "file": "posts/post-1/post-FINAL.md", "last_updated": "2026-07-01"}
        }
        g = artifacts.artifact_groups(self.pdir, rows)[0]
        self.assertEqual(g["version_count"], 1)  # ingredient trails not absorbed

    def test_rows_without_file_become_stub_groups(self):
        rows = {"charter": {"status": "not_started", "file": None}}
        g = artifacts.artifact_groups(self.pdir, rows)[0]
        self.assertIsNone(g["current"])
        self.assertEqual(g["version_count"], 0)

    def test_sorted_last_updated_desc_ties_broken_by_later_row(self):
        write(self.pdir / "a/a-v1.md")
        write(self.pdir / "b/b-v1.md")
        write(self.pdir / "c/c-v1.md")
        rows = {
            "older": {"status": "drafting", "file": "a/a-v1.md", "last_updated": "2026-06-01"},
            "tie-first": {"status": "drafting", "file": "b/b-v1.md", "last_updated": "2026-07-01"},
            "tie-later": {"status": "drafting", "file": "c/c-v1.md", "last_updated": "2026-07-01"},
        }
        slugs = [g["slug"] for g in artifacts.artifact_groups(self.pdir, rows)]
        self.assertEqual(slugs, ["tie-later", "tie-first", "older"])

    def test_archived_rows_merged_and_tagged(self):
        write(self.pdir / "old/old-v1.md")
        write(
            self.pdir / "knowledge/_archive/deliverables-archive.md",
            textwrap.dedent("""\
                ---
                deliverables:
                  old-report:
                    status: published
                    file: old/old-v1.md
                    last_updated: 2026-05-01
                ---
                """),
        )
        rows = {}
        groups = artifacts.artifact_groups(self.pdir, rows)
        self.assertEqual(groups[0]["slug"], "old-report")
        self.assertTrue(groups[0]["archived"])


class TestGroupDetail(ArtifactFixture):
    def test_versions_ascending_and_exports_listed(self):
        write(self.pdir / "phase-2/build-brief/build-brief-v1.md")
        write(self.pdir / "phase-2/build-brief/build-brief-v2.md")
        write(self.pdir / "phase-2/build-brief/exports/handover.pdf")
        write(self.pdir / "phase-2/build-brief/kit/00-happy-path.md")  # different stem, not a version
        detail = artifacts.group_detail(self.pdir, "phase-2/build-brief/build-brief-v2.md")
        self.assertEqual(
            detail["versions"],
            ["phase-2/build-brief/build-brief-v1.md", "phase-2/build-brief/build-brief-v2.md"],
        )
        self.assertEqual(detail["exports"], ["phase-2/build-brief/exports/handover.pdf"])
        self.assertEqual(detail["manifest_media"], [])
        self.assertEqual(detail["folder_media"], [])

    def test_manifest_and_folder_media_attach_to_group_detail(self):
        write(
            self.pdir / "posts/post-1/post-FINAL.md",
            textwrap.dedent("""\
                ---
                status: published
                last_updated: 2026-07-07
                shipped_media:
                  - posts/post-1/media/final.png
                exports:
                  - exports/summary.pdf
                ---
                """),
        )
        write(self.pdir / "posts/post-1/media/final.png")
        write(self.pdir / "posts/post-1/exports/summary.pdf")
        write(self.pdir / "posts/post-1/visuals/draft.png")
        detail = artifacts.group_detail(self.pdir, "posts/post-1/post-FINAL.md")
        self.assertEqual(detail["manifest_media"], ["posts/post-1/media/final.png", "posts/post-1/exports/summary.pdf"])
        self.assertEqual(detail["exports"], [])
        self.assertEqual(detail["folder_media"], ["posts/post-1/visuals/draft.png"])

    def test_ppt_timestamp_versions_sorted(self):
        base = "deck/exports"
        write(self.pdir / f"{base}/deck_20260615_200932.pptx")
        write(self.pdir / f"{base}/deck_20260615_203631.pptx")
        detail = artifacts.group_detail(self.pdir, f"{base}/deck_20260615_200932.pptx")
        self.assertEqual(
            detail["versions"],
            [f"{base}/deck_20260615_200932.pptx", f"{base}/deck_20260615_203631.pptx"],
        )


class TestUntracked(ArtifactFixture):
    def test_untracked_excludes_hidden_dirs_claimed_files_and_nonpreviewable(self):
        write(self.pdir / "phase-1/notes.md")
        write(self.pdir / "phase-1/tool.py")  # not previewable
        write(self.pdir / "sources/dump.md")  # hidden dir
        write(self.pdir / "phase-2/x/x-v1.md")  # claimed below
        write(self.pdir / "phase-3/hideme/.preview-hide")
        write(self.pdir / "phase-3/hideme/secret.png")
        found = artifacts.untracked_files(self.pdir, claimed={"phase-2/x/x-v1.md"})
        self.assertEqual(found, ["phase-1/notes.md"])


class TestProjectFiles(ArtifactFixture):
    def test_flat_file_filters_grouped_exports_untracked_and_hidden(self):
        write(self.pdir / "phase-1/brief/brief-v1.md")
        write(self.pdir / "phase-1/brief/brief-v2.md")
        write(self.pdir / "phase-1/brief/exports/brief.pdf")
        write(self.pdir / "phase-1/brief/media/chart.png")
        write(self.pdir / "phase-2/notes.txt")
        write(self.pdir / "phase-2/page.html")
        write(self.pdir / "phase-2/hidden/.preview-hide")
        write(self.pdir / "phase-2/hidden/secret.png")
        rows = {
            "brief": {
                "status": "ready",
                "file": "phase-1/brief/brief-v2.md",
                "last_updated": "2026-07-07",
            }
        }

        text_files = artifacts.project_files(self.pdir, rows, "text")
        self.assertEqual(
            sorted(r["path"] for r in text_files),
            ["phase-1/brief/brief-v1.md", "phase-1/brief/brief-v2.md", "phase-2/notes.txt"],
        )

        media_files = artifacts.project_files(self.pdir, rows, "media")
        media_paths = sorted(r["path"] for r in media_files)
        self.assertEqual(media_paths, ["phase-1/brief/exports/brief.pdf", "phase-1/brief/media/chart.png", "phase-2/page.html"])
        self.assertNotIn("phase-2/hidden/secret.png", media_paths)

        image_files = artifacts.project_files(self.pdir, rows, "media", "image")
        self.assertEqual([r["path"] for r in image_files], ["phase-1/brief/media/chart.png"])


if __name__ == "__main__":
    unittest.main()
