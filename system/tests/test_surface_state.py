import os
import tempfile
import textwrap
import unittest
from pathlib import Path

from system.server.lib.surface import state


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def make_project(root, slug, frontmatter, body="# Body\n"):
    pdir = os.path.join(root, "workspace", "projects", slug)
    write(os.path.join(pdir, "project.md"), "---\n" + textwrap.dedent(frontmatter).strip() + "\n---\n\n" + body)
    return pdir


ACTIVE_FM = """
name: "AI Chaos Scout"
slug: ai-chaos-scout
status: active
domain: project-mgmt
flow: open-flow
current_phase: 5-recap
last_activity: 2026-07-06T12:00:00-07:00
deliverables:
  build-brief:
    status: drafting
    file: phase-2-architecture/build-brief/build-brief-v2.md
    last_updated: 2026-07-04
"""


class TestScanProjects(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name

    def tearDown(self):
        self.tmp.cleanup()

    def test_scans_active_root_level_projects(self):
        make_project(self.root, "ai-chaos-scout", ACTIVE_FM)
        projects = state.scan_projects(Path(self.root))
        self.assertEqual(len(projects), 1)
        p = projects[0]
        self.assertEqual(p["slug"], "ai-chaos-scout")
        self.assertEqual(p["name"], "AI Chaos Scout")
        self.assertEqual(p["status"], "active")
        self.assertEqual(p["current_phase"], "5-recap")
        self.assertIn("build-brief", p["deliverables"])

    def test_skips_completed_folder_and_projectless_folders(self):
        make_project(self.root, "ai-chaos-scout", ACTIVE_FM)
        # completed/ project must not appear
        make_project(self.root, os.path.join("completed", "old-campaign"), ACTIVE_FM)
        # folder with no project.md must be skipped silently
        os.makedirs(os.path.join(self.root, "workspace", "projects", "example-no-state", "phase-1"))
        projects = state.scan_projects(Path(self.root))
        self.assertEqual([p["slug"] for p in projects], ["ai-chaos-scout"])

    def test_non_active_projects_excluded(self):
        make_project(self.root, "done-thing", ACTIVE_FM.replace("status: active", "status: complete"))
        self.assertEqual(state.scan_projects(Path(self.root)), [])

    def test_parses_inline_yaml_comments(self):
        fm = """
        name: "Agent Architecture POV"
        slug: agent-architecture-pov
        status: active                          # enum: active | complete | cancelled
        domain: marketing
        flow: marketing-standard-flow
        current_phase: 4-production             # enum: 1-research | ...
        deliverables: {}
        """
        make_project(self.root, "agent-architecture-pov", fm)
        p = state.scan_projects(Path(self.root))[0]
        self.assertEqual(p["status"], "active")
        self.assertEqual(p["current_phase"], "4-production")

    def test_missing_fields_render_blank_and_empty_deliverables_ok(self):
        fm = """
        name: "Pm Acme Discovery"
        slug: pm-acme-discovery
        status: active
        deliverables: {}
        """
        make_project(self.root, "pm-acme-discovery", fm)
        p = state.scan_projects(Path(self.root))[0]
        self.assertIsNone(p["flow"])
        self.assertIsNone(p["last_activity"])
        self.assertEqual(p["deliverables"], {})

    def test_broken_frontmatter_never_crashes_scan(self):
        pdir = os.path.join(self.root, "workspace", "projects", "broken")
        write(os.path.join(pdir, "project.md"), "---\n: not [valid yaml\n---\n")
        make_project(self.root, "ai-chaos-scout", ACTIVE_FM)
        projects = state.scan_projects(Path(self.root))
        self.assertEqual([p["slug"] for p in projects], ["ai-chaos-scout"])


ACTIVITY_TEXT = """# Activity — ai-chaos-scout

## Attention

- [ ] 2026-07-15 | due | Finish workshop preread
- [ ] 2026-07-18 | waiting | Client reply on [deck](phase-4-demo/demo-deck-v1.md)
- [x] 2026-07-01 | decision | Vendor shortlist approved

2026-07-04 12:00 — plan_revised: merged old phase-3 into phase-2. Reason: operator scoping session.

2026-07-06 13:08 — published: linkedin-post shipped live as v9.

a raw note line without any timestamp
"""


class TestParseAttention(unittest.TestCase):
    def test_parses_items_with_date_kind_text(self):
        items = state.parse_attention(ACTIVITY_TEXT)
        self.assertEqual(len(items), 3)
        first = items[0]
        self.assertEqual(first["date"], "2026-07-15")
        self.assertEqual(first["kind"], "due")
        self.assertEqual(first["text"], "Finish workshop preread")
        self.assertFalse(first["checked"])

    def test_checked_items_flagged(self):
        items = state.parse_attention(ACTIVITY_TEXT)
        self.assertTrue(items[2]["checked"])

    def test_markdown_link_target_captured_as_file(self):
        items = state.parse_attention(ACTIVITY_TEXT)
        self.assertEqual(items[1]["file"], "phase-4-demo/demo-deck-v1.md")
        self.assertIsNone(items[0]["file"])

    def test_no_attention_block_returns_empty(self):
        self.assertEqual(state.parse_attention("# Activity\n\n2026-01-01 09:00 — note: hi\n"), [])

    def test_malformed_bullet_degrades_to_text(self):
        text = "## Attention\n\n- [ ] just a reminder with no pipes\n"
        items = state.parse_attention(text)
        self.assertEqual(len(items), 1)
        self.assertIsNone(items[0]["date"])
        self.assertIsNone(items[0]["kind"])
        self.assertEqual(items[0]["text"], "just a reminder with no pipes")


class TestParseActivity(unittest.TestCase):
    def test_newest_first_with_timestamp_and_event(self):
        entries = state.parse_activity(ACTIVITY_TEXT)
        self.assertEqual(entries[0]["event"], "published")
        self.assertEqual(entries[0]["timestamp"], "2026-07-06 13:08")
        self.assertEqual(entries[1]["event"], "plan_revised")

    def test_attention_block_and_headings_excluded(self):
        entries = state.parse_activity(ACTIVITY_TEXT)
        texts = " ".join(e["text"] for e in entries)
        self.assertNotIn("Finish workshop preread", texts)
        self.assertNotIn("# Activity", texts)

    def test_untimestamped_line_kept_as_raw(self):
        entries = state.parse_activity(ACTIVITY_TEXT)
        raws = [e for e in entries if e["timestamp"] is None]
        self.assertEqual(len(raws), 1)
        self.assertIn("raw note line", raws[0]["text"])
        # untimestamped lines sort after timestamped ones
        self.assertIs(entries[-1], raws[0])


class TestDetectFileRef(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.pdir = self.tmp.name
        write(os.path.join(self.pdir, "phase-1", "report-v2.md"), "hi")

    def tearDown(self):
        self.tmp.cleanup()

    def test_finds_existing_path_in_line(self):
        line = "2026-07-04 12:00 — export: wrote phase-1/report-v2.md for handover."
        self.assertEqual(state.detect_file_ref(line, Path(self.pdir)), "phase-1/report-v2.md")

    def test_ignores_nonexistent_paths(self):
        line = "moved phase-9/nothing-here.md around"
        self.assertIsNone(state.detect_file_ref(line, Path(self.pdir)))


if __name__ == "__main__":
    unittest.main()
