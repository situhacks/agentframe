import os
import tempfile
import textwrap
import unittest

from system import af


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


PROJECT_FM = """\
name: Test
slug: test
schema_version: 2026-04-23
created_at: 2026-07-07
domain: marketing
status: active
current_phase: active
flow: open-flow
last_activity: 2026-07-07T12:00:00-07:00
post_manifest:
  ingredients: [body-copy, image-prompts]
deliverables:
  post-1:
    status: delivered
    file: posts/post-1/post-FINAL.md
    last_updated: 2026-07-07
"""


class TestMediaDoctor(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cdir = self.tmp.name
        write(os.path.join(self.cdir, "project.md"), "---\n" + PROJECT_FM + "---\n")

    def tearDown(self):
        self.tmp.cleanup()

    def project_fm(self):
        return af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")[0]

    def test_missing_shipped_media_path_is_issue(self):
        write(
            os.path.join(self.cdir, "posts/post-1/post-FINAL.md"),
            textwrap.dedent("""\
                ---
                status: delivered
                last_updated: 2026-07-07
                shipped_media:
                  - posts/post-1/media/final.png
                ---
                """),
        )
        issues = af.media_manifest_issues(self.cdir, self.project_fm())
        self.assertTrue(any("shipped_media path missing" in issue for issue in issues))

    def test_existing_owner_relative_media_path_passes(self):
        write(os.path.join(self.cdir, "posts/post-1/media/final.png"), "x")
        write(
            os.path.join(self.cdir, "posts/post-1/post-FINAL.md"),
            textwrap.dedent("""\
                ---
                status: delivered
                last_updated: 2026-07-07
                shipped_media:
                  - media/final.png
                ---
                """),
        )
        self.assertEqual(af.media_manifest_issues(self.cdir, self.project_fm()), [])

    def test_delivered_image_post_without_shipped_media_is_note(self):
        write(os.path.join(self.cdir, "posts/post-1/visuals/draft.png"), "x")
        write(
            os.path.join(self.cdir, "posts/post-1/post-FINAL.md"),
            textwrap.dedent("""\
                ---
                status: delivered
                last_updated: 2026-07-07
                ---
                """),
        )
        notes = af.media_manifest_notes(self.cdir)
        self.assertTrue(any("empty shipped_media" in note for note in notes))


CAROUSEL_PROJECT_FM = """\
name: Test
slug: test
domain: marketing
status: active
deliverables:
  post-1-carousel:
    status: locked
    file: post-1-carousel/image-prompts-v1.md
    last_updated: 2026-07-09
"""


class TestLockedExportableDoctor(unittest.TestCase):
    """Doctor flags locked/delivered exportable deliverables with empty exports[]."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cdir = self.tmp.name

    def tearDown(self):
        self.tmp.cleanup()

    def test_locked_image_prompts_without_exports_is_issue(self):
        write(
            os.path.join(self.cdir, "post-1-carousel/image-prompts-v1.md"),
            textwrap.dedent("""\
                ---
                status: locked
                last_updated: 2026-07-09
                ---
                """),
        )
        issues = af.media_manifest_issues(self.cdir, CAROUSEL_PROJECT_FM)
        self.assertTrue(any("empty exports[]" in issue for issue in issues))

    def test_locked_image_prompts_with_filed_exports_passes(self):
        write(os.path.join(self.cdir, "post-1-carousel/media/final.pdf"), "x")
        write(
            os.path.join(self.cdir, "post-1-carousel/image-prompts-v1.md"),
            textwrap.dedent("""\
                ---
                status: locked
                last_updated: 2026-07-09
                exports:
                  - media/final.pdf
                ---
                """),
        )
        self.assertEqual(af.media_manifest_issues(self.cdir, CAROUSEL_PROJECT_FM), [])


if __name__ == "__main__":
    unittest.main()
