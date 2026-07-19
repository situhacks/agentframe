import os
import tempfile
import textwrap
import time
import unittest
from pathlib import Path

from system.server.lib.surface import snapshot


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def make_project(root, slug, frontmatter, activity=None):
    pdir = os.path.join(root, "workspace", "projects", slug)
    write(os.path.join(pdir, "project.md"), "---\n" + textwrap.dedent(frontmatter).strip() + "\n---\n")
    if activity is not None:
        write(os.path.join(pdir, "activity.md"), textwrap.dedent(activity))
    return pdir


FM_A = """
name: "Alpha"
slug: alpha
status: active
domain: marketing
flow: open-flow
current_phase: active
last_activity: 2026-07-05T10:00:00-07:00
deliverables:
  brief:
    status: drafting
    file: phase-1/brief/brief-v1.md
    last_updated: 2026-07-01
  deck:
    status: drafting
    file: phase-2/deck/deck-v1.md
    last_updated: 2026-07-03
"""

ACT_A = """\
# Activity — alpha

## Attention

- [ ] 2026-07-15 | due | Finish preread
- [x] 2026-07-01 | decision | Closed item

2026-07-03 09:00 — export: packaged deck.

2026-07-05 10:00 — published: brief went live.
"""

FM_B = """
name: "Beta"
slug: beta
status: active
deliverables: {}
"""

FM_C = """
name: "Gamma"
slug: gamma
status: active
domain: project-mgmt
flow: open-flow
current_phase: active
last_activity: 2026-07-06T11:30:00-07:00
deliverables: {}
"""

ACT_B = """\
# Activity — beta

2026-07-04 12:00 — plan_revised: replanned everything.
"""


class SnapshotFixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        make_project(self.root, "alpha", FM_A, ACT_A)
        write(str(self.root / "workspace/projects/alpha/phase-1/brief/brief-v1.md"), "x")
        write(str(self.root / "workspace/projects/alpha/phase-2/deck/deck-v1.md"), "x")
        make_project(self.root, "beta", FM_B, ACT_B)

    def tearDown(self):
        self.tmp.cleanup()


class TestSnapshot(SnapshotFixture):
    def test_projects_carry_attention_count_and_latest_deliverable(self):
        snap = snapshot.build_snapshot(self.root)
        alpha = next(p for p in snap["projects"] if p["slug"] == "alpha")
        self.assertEqual(alpha["attention_count"], 1)  # unchecked only
        self.assertEqual(alpha["latest_deliverable"]["slug"], "deck")  # newer last_updated wins
        self.assertEqual(alpha["latest_deliverable"]["file"], "phase-2/deck/deck-v1.md")
        self.assertEqual(alpha["current_deliverable"]["slug"], "deck")
        self.assertEqual(alpha["next_attention"]["text"], "Finish preread")
        self.assertEqual(alpha["last_updated"], "2026-07-05T10:00:00-07:00")
        self.assertEqual(alpha["last_updated_label"], "Jul 5, 10:00 AM")

    def test_projects_sorted_by_last_updated(self):
        make_project(self.root, "gamma", FM_C)
        snap = snapshot.build_snapshot(self.root)
        self.assertEqual([p["slug"] for p in snap["projects"]], ["gamma", "alpha", "beta"])

    def test_governance_status_comes_only_from_governance_files(self):
        snap = snapshot.build_snapshot(self.root)
        alpha = next(p for p in snap["projects"] if p["slug"] == "alpha")
        beta = next(p for p in snap["projects"] if p["slug"] == "beta")
        self.assertEqual(alpha["governance_status"], "ungoverned")
        self.assertEqual(beta["governance_status"], "ungoverned")
        write(str(self.root / "workspace/projects/alpha/knowledge/raid-log.md"), "# RAID")
        snap2 = snapshot.build_snapshot(self.root)
        alpha2 = next(p for p in snap2["projects"] if p["slug"] == "alpha")
        self.assertEqual(alpha2["governance_status"], "governed")

    def test_current_work_prefers_drafting_over_newer_terminal_state(self):
        fm = """
        name: "Delta"
        slug: delta
        status: active
        domain: marketing
        deliverables:
          in-flight:
            status: drafting
            file: drafts/in-flight.md
            last_updated: 2026-07-01
            job: current work
          shipped:
            status: published
            file: shipped/final.md
            last_updated: 2026-07-08
        """
        make_project(self.root, "delta", fm)
        snap = snapshot.build_snapshot(self.root)
        delta = next(p for p in snap["projects"] if p["slug"] == "delta")
        self.assertEqual(delta["latest_deliverable"]["slug"], "shipped")
        self.assertEqual(delta["current_deliverable"]["slug"], "in-flight")
        self.assertEqual(delta["current_deliverable"]["job"], "current work")

    def test_attention_lists_unchecked_only_sorted_by_date(self):
        snap = snapshot.build_snapshot(self.root)
        self.assertEqual(len(snap["attention"]), 1)
        self.assertEqual(snap["attention"][0]["project"], "alpha")
        self.assertEqual(snap["attention"][0]["kind"], "due")

    def test_recent_activity_merged_newest_first_across_projects(self):
        snap = snapshot.build_snapshot(self.root)
        items = snap["recent_activity"]["items"]
        self.assertEqual([i["project"] for i in items[:3]], ["alpha", "beta", "alpha"])
        self.assertEqual(items[0]["timestamp"], "2026-07-05 10:00")
        self.assertEqual(items[0]["project_name"], "Alpha")

    def test_humanize_timestamp(self):
        now = snapshot.datetime.datetime(2026, 7, 7, 12, 0)
        self.assertEqual(snapshot.humanize_timestamp("2026-07-07 09:34", now), "Jul 7, 9:34 AM")
        self.assertEqual(snapshot.humanize_timestamp("2026-07-06 17:42", now), "Jul 6, 5:42 PM")
        self.assertEqual(snapshot.humanize_timestamp("2026-06-25 08:00", now), "Jun 25, 8:00 AM")
        self.assertEqual(snapshot.humanize_project_updated("2026-07-06T12:00:00-07:00"), "Jul 6, 12:00 PM")
        self.assertEqual(snapshot.humanize_project_updated("2026-06-25"), "Jun 25")


class TestActivityPaging(SnapshotFixture):
    def test_cursor_pages_through_merged_stream(self):
        cache = snapshot.SnapshotCache(self.root)
        page1 = cache.activity_page(cursor=0, limit=2)
        self.assertEqual(len(page1["items"]), 2)
        self.assertEqual(page1["next_cursor"], 2)
        page2 = cache.activity_page(cursor=2, limit=2)
        self.assertEqual(len(page2["items"]), 1)
        self.assertIsNone(page2["next_cursor"])


class TestEtagInvalidation(SnapshotFixture):
    def test_etag_stable_until_state_file_changes(self):
        cache = snapshot.SnapshotCache(self.root)
        first = cache.get()
        second = cache.get()
        self.assertEqual(first["etag"], second["etag"])
        # touching a watched file invalidates
        activity = self.root / "workspace/projects/beta/activity.md"
        time.sleep(0.01)
        activity.write_text(activity.read_text(encoding="utf-8") + "\n2026-07-06 09:00 — note: touched.\n", encoding="utf-8")
        os.utime(activity)
        third = cache.get()
        self.assertNotEqual(first["etag"], third["etag"])

    def test_new_project_folder_invalidates(self):
        cache = snapshot.SnapshotCache(self.root)
        first = cache.get()
        time.sleep(0.01)
        make_project(self.root, "gamma", FM_B.replace('"Beta"', '"Gamma"').replace("slug: beta", "slug: gamma"))
        third = cache.get()
        self.assertNotEqual(first["etag"], third["etag"])
        self.assertIn("gamma", [p["slug"] for p in third["projects"]])


class TestPathContainment(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.pdir = Path(self.tmp.name)
        write(str(self.pdir / "phase-1/ok.md"), "x")
        write(str(Path(self.tmp.name).parent / "outside-marker-file.txt"), "x")

    def tearDown(self):
        self.tmp.cleanup()

    def test_inside_path_resolves(self):
        p = snapshot.resolve_in_project(self.pdir, "phase-1/ok.md")
        self.assertIsNotNone(p)
        self.assertTrue(str(p).startswith(str(self.pdir.resolve())))

    def test_escape_attempts_rejected(self):
        self.assertIsNone(snapshot.resolve_in_project(self.pdir, "../outside-marker-file.txt"))
        self.assertIsNone(snapshot.resolve_in_project(self.pdir, "phase-1/../../outside-marker-file.txt"))
        self.assertIsNone(snapshot.resolve_in_project(self.pdir, "C:/Windows/system32/drivers/etc/hosts"))


if __name__ == "__main__":
    unittest.main()
