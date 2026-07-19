import tempfile
import unittest
from pathlib import Path

from system.server.lib.surface import api, snapshot, state


PROJECT = """---
name: {name}
slug: {slug}
created_at: {created_at}
status: {status}
domain: marketing
flow: open-flow
current_phase: active
last_activity: {last_activity}
completed_at: {completed_at}
cancelled_at: null
deliverables:
  brief:
    status: {deliverable_status}
    file: brief/brief-v1.md
    last_updated: {deliverable_date}
    job: leadership-ready brief
---
"""


class SurfaceCalendarTests(unittest.TestCase):
    def _project(
        self,
        root: Path,
        slug: str,
        *,
        status: str,
        created_at: str,
        last_activity: str,
        completed_at: str = "null",
        under_completed: bool = False,
    ) -> Path:
        base = root / "workspace" / "projects"
        if under_completed:
            base /= "completed"
        project = base / slug
        (project / "brief").mkdir(parents=True)
        (project / "brief" / "brief-v1.md").write_text("# Brief\n", encoding="utf-8")
        (project / "project.md").write_text(
            PROJECT.format(
                name=slug.replace("-", " ").title(),
                slug=slug,
                status=status,
                created_at=created_at,
                last_activity=last_activity,
                completed_at=completed_at,
                deliverable_status="ready" if status == "active" else "published",
                deliverable_date=last_activity[:10],
            ),
            encoding="utf-8",
        )
        return project

    def test_scan_projects_can_include_completed_history(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            self._project(
                root,
                "active-project",
                status="active",
                created_at="2026-06-01",
                last_activity="2026-06-10T10:00:00-07:00",
            )
            self._project(
                root,
                "completed-project",
                status="complete",
                created_at="2026-05-01",
                last_activity="2026-05-20T10:00:00-07:00",
                completed_at="2026-05-20",
                under_completed=True,
            )

            self.assertEqual([p["slug"] for p in state.scan_projects(root)], ["active-project"])
            self.assertEqual(
                {p["slug"] for p in state.scan_projects(root, include_completed=True)},
                {"active-project", "completed-project"},
            )

    def test_preview_project_filters_include_completed_history(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            self._project(
                root, "active-project", status="active",
                created_at="2026-06-01", last_activity="2026-06-10T10:00:00-07:00",
            )
            self._project(
                root, "completed-project", status="complete",
                created_at="2026-05-01", last_activity="2026-05-20T10:00:00-07:00",
                completed_at="2026-05-20", under_completed=True,
            )
            snap = snapshot.build_snapshot(root)

            self.assertEqual(
                [p["slug"] for p in api.project_summaries(root, snap, "active")],
                ["active-project"],
            )
            completed = api.project_summaries(root, snap, "completed")
            self.assertEqual([p["slug"] for p in completed], ["completed-project"])
            self.assertEqual(completed[0]["status"], "complete")
            self.assertEqual(
                {p["slug"] for p in api.project_summaries(root, snap, "all")},
                {"active-project", "completed-project"},
            )

    def test_timeline_projects_sorted_active_first_then_created(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            # completed project created earliest
            self._project(
                root, "old-complete", status="complete",
                created_at="2026-01-01", last_activity="2026-02-01T10:00:00-07:00",
                completed_at="2026-02-01", under_completed=True,
            )
            # active project created later
            self._project(
                root, "new-active", status="active",
                created_at="2026-06-01", last_activity="2026-06-10T10:00:00-07:00",
            )
            # active project created earliest of the actives
            self._project(
                root, "early-active", status="active",
                created_at="2026-03-01", last_activity="2026-06-11T10:00:00-07:00",
            )
            timeline = snapshot.build_snapshot(root)["timeline_projects"]
            self.assertEqual(
                [p["slug"] for p in timeline],
                ["early-active", "new-active", "old-complete"],
            )

    def test_timeline_project_reports_worked_days(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            project = self._project(
                root, "worklog", status="active",
                created_at="2026-06-01", last_activity="2026-06-10T10:00:00-07:00",
            )
            (project / "activity.md").write_text(
                "# Activity\n\n"
                "2026-06-10 10:00 — deliverable_ready: brief ready; brief/brief-v1.md\n"
                "2026-06-12 14:30 — export: deck exported\n",
                encoding="utf-8",
            )
            timeline = snapshot.build_snapshot(root)["timeline_projects"]
            worked = timeline[0]["worked_days"]
            # deliverable stamped 2026-06-10 + activity on 06-10 and 06-12
            self.assertEqual(worked, ["2026-06-10", "2026-06-12"])

    def test_work_blocks_cluster_and_pad(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            project = self._project(
                root, "sprint", status="active",
                created_at="2026-06-01", last_activity="2026-06-10T22:00:00-07:00",
            )
            (project / "activity.md").write_text(
                "# Activity\n\n"
                "2026-06-10 12:00 — plan_revised: a\n"
                "2026-06-10 13:10 — export: b\n"       # 70 min gap -> same block
                "2026-06-10 21:00 — ready: c\n"         # 470 min gap -> new block
                "2026-06-11 09:00 — export: d\n",       # different day -> new block
                encoding="utf-8",
            )
            timeline = snapshot.build_snapshot(root)["timeline_projects"]
            blocks = timeline[0]["work_blocks"]
            self.assertEqual(len(blocks), 3)
            b0 = blocks[0]
            self.assertEqual(b0["date"], "2026-06-10")
            self.assertEqual(b0["start"], 12 * 60 - 30)   # 11:30
            self.assertEqual(b0["end"], 13 * 60 + 10 + 30) # 13:40
            self.assertEqual([e["label"] for e in b0["events"]], ["plan_revised", "export"])
            self.assertEqual(blocks[1]["date"], "2026-06-10")
            self.assertEqual(blocks[2]["date"], "2026-06-11")

    def test_work_blocks_ignore_untimed_and_clamp(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            project = self._project(
                root, "edge", status="active",
                created_at="2026-06-01", last_activity="2026-06-10T00:00:00-07:00",
            )
            (project / "activity.md").write_text(
                "# Activity\n\n"
                "2026-06-10 00:00 — frontmatter_manual_edit: midnight artifact\n"
                "a raw line with no timestamp\n",
                encoding="utf-8",
            )
            blocks = snapshot.build_snapshot(root)["timeline_projects"][0]["work_blocks"]
            self.assertEqual(len(blocks), 1)
            self.assertEqual(blocks[0]["start"], 0)          # clamped, not -30
            self.assertEqual(blocks[0]["end"], 60)           # 1-hour minimum

    def _pulse_project(self, root: Path) -> None:
        project = self._project(
            root, "pulse", status="active",
            created_at="2026-06-01", last_activity="2026-06-10T10:00:00-07:00",
        )
        (project / "activity.md").write_text(
            "# Activity\n\n"
            "2026-06-10 10:00 — ready: brief ready; brief/brief-v1.md\n"
            "2026-06-12 09:15 — artifact_drafted: redesign created; redesign/redesign-v1.md\n"
            "2026-06-13 14:40 — artifact_versioned: redesign v1 -> v2; redesign/redesign-v2.md\n",
            encoding="utf-8",
        )

    def test_pulse_events_count_as_work_but_not_material_markers(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            self._pulse_project(root)
            timeline = snapshot.build_snapshot(root)["timeline_projects"][0]
            self.assertIn("2026-06-12", timeline["worked_days"])
            self.assertIn("2026-06-13", timeline["worked_days"])
            block_dates = {b["date"] for b in timeline["work_blocks"]}
            self.assertIn("2026-06-12", block_dates)
            self.assertIn("2026-06-13", block_dates)
            self.assertEqual({e["event"] for e in timeline["activity"]}, {"ready"})

    def test_pulse_events_excluded_from_recent_activity(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            self._pulse_project(root)
            events = [i["event"] for i in snapshot.build_snapshot(root)["recent_activity"]["items"]]
            self.assertIn("ready", events)
            self.assertNotIn("artifact_drafted", events)
            self.assertNotIn("artifact_versioned", events)

    def test_snapshot_calendar_includes_milestones_events_and_future_attention(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            project = self._project(
                root,
                "leadership-proof",
                status="active",
                created_at="2026-06-01",
                last_activity="2026-06-10T10:00:00-07:00",
            )
            (project / "activity.md").write_text(
                """# Activity

## Attention

- [ ] 2026-07-15 | review | Coach review of [brief](brief/brief-v1.md)

2026-06-10 10:00 — deliverable_ready: brief ready; brief/brief-v1.md
""",
                encoding="utf-8",
            )

            data = snapshot.build_snapshot(root)
            timeline = data["timeline_projects"]

            self.assertEqual(len(timeline), 1)
            self.assertEqual(timeline[0]["created_at"], "2026-06-01")
            self.assertEqual(timeline[0]["deliverables"][0]["slug"], "brief")
            self.assertEqual(timeline[0]["activity"][0]["event"], "deliverable_ready")
            self.assertEqual(timeline[0]["activity"][0]["file"], "brief/brief-v1.md")
            self.assertEqual(timeline[0]["attention"][0]["date"], "2026-07-15")
            self.assertEqual(timeline[0]["attention"][0]["file"], "brief/brief-v1.md")

    def test_snapshot_calendar_includes_live_career_deadlines_and_nudges(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            pipeline = root / "workspace" / "pipeline"
            pipeline.mkdir(parents=True)
            (pipeline / "pipeline.md").write_text(
                """---
created_at: 2026-07-01
last_activity: 2026-07-10T09:00:00-07:00
applications:
  promotion-case:
    stage: preparing
    company: Acme
    role: Senior promotion
    saved: 2026-07-02
    deadline: 2026-08-15
  market-case:
    stage: applied
    company: Example Co
    role: Product lead
    saved: 2026-07-03
    applied: 2026-07-09
    next_nudge: 2026-07-16
  closed-case:
    stage: dropped
    company: Old Co
    role: Closed role
    deadline: 2026-09-01
---
""",
                encoding="utf-8",
            )

            timeline = snapshot.build_snapshot(root)["timeline_projects"]

            self.assertEqual(len(timeline), 1)
            career = timeline[0]
            self.assertEqual(career["slug"], "@career-pipeline")
            self.assertEqual(career["name"], "Career cases")
            self.assertFalse(career["previewable"])
            self.assertEqual(career["worked_days"], ["2026-07-02", "2026-07-03", "2026-07-09", "2026-07-10"])
            self.assertEqual(
                [(item["date"], item["kind"], item["text"]) for item in career["attention"]],
                [
                    ("2026-07-16", "career follow-up", "Example Co · Product lead"),
                    ("2026-08-15", "career case deadline", "Acme · Senior promotion"),
                ],
            )

    def test_pipeline_calendar_change_invalidates_snapshot_cache(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            pipeline = root / "workspace" / "pipeline"
            pipeline.mkdir(parents=True)
            board = pipeline / "pipeline.md"
            board.write_text("---\ncreated_at: 2026-07-01\napplications: {}\n---\n", encoding="utf-8")
            cache = snapshot.SnapshotCache(root)
            first = cache.get()["etag"]

            board.write_text(
                "---\ncreated_at: 2026-07-01\napplications:\n  case:\n    stage: preparing\n    deadline: 2026-08-15\n---\n",
                encoding="utf-8",
            )

            self.assertNotEqual(first, cache.get()["etag"])


if __name__ == "__main__":
    unittest.main()
