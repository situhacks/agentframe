import tempfile
import unittest
from pathlib import Path

from system.server.lib.surface import snapshot, state


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
                deliverable_status="locked" if status == "active" else "delivered",
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

2026-06-10 10:00 — deliverable_locked: brief locked; brief/brief-v1.md
""",
                encoding="utf-8",
            )

            data = snapshot.build_snapshot(root)
            timeline = data["timeline_projects"]

            self.assertEqual(len(timeline), 1)
            self.assertEqual(timeline[0]["created_at"], "2026-06-01")
            self.assertEqual(timeline[0]["deliverables"][0]["slug"], "brief")
            self.assertEqual(timeline[0]["activity"][0]["event"], "deliverable_locked")
            self.assertEqual(timeline[0]["activity"][0]["file"], "brief/brief-v1.md")
            self.assertEqual(timeline[0]["attention"][0]["date"], "2026-07-15")
            self.assertEqual(timeline[0]["attention"][0]["file"], "brief/brief-v1.md")


if __name__ == "__main__":
    unittest.main()
