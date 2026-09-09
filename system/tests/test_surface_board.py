import datetime as dt
import os
import tempfile
import unittest
from pathlib import Path

from system import board as workboard
from system.server.lib.surface import board as surface_board


PROJECT_MD = """---
name: Joyce Hair Design
slug: joyce-hair-pilot
schema_version: 2026-07-19-v2
created_at: 2026-07-12
domain: project-mgmt
status: active
current_phase: active
flow: open-flow
last_activity: 2026-07-12T10:00:00+00:00
deliverables: {}
---

# Joyce
"""

TZ = dt.datetime.now().astimezone().tzinfo


def when(text):
    return dt.datetime.fromisoformat(text).replace(tzinfo=TZ)


class SurfaceBoardTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        pdir = self.root / "workspace" / "projects" / "joyce-hair-pilot"
        pdir.mkdir(parents=True)
        (pdir / "project.md").write_text(PROJECT_MD, encoding="utf-8")
        self.now = when("2026-09-09T12:00:00")
        self.addCleanup(self._tmp.cleanup)

    def test_no_board_yields_an_empty_model_with_the_init_hint(self):
        model = surface_board.build_model(self.root, now=self.now, rows=[])
        self.assertFalse(model["exists"])
        self.assertEqual([lane["name"] for lane in model["lanes"]], list(workboard.LANES))
        self.assertIn("board init", model["init_hint"])

    def test_model_joins_cards_names_roster_and_archive(self):
        workboard.init(self.root)
        b = workboard.load(self.root)
        card = workboard.add(self.root, b, project="joyce-hair-pilot", deliverable="booking-page-copy", now=self.now)
        other = workboard.add(self.root, b, project="meetcap", deliverable="live-call-trial", by="human", now=self.now)
        workboard.dispatch(b, card.id, session="a5b92fc8")
        workboard.save(self.root, b)
        workboard.drop(self.root, b, other.id, now=self.now, note="not now")
        workboard.save(self.root, b)
        rows = [{"id": "a5b92fc8", "kind": "background", "state": "working", "name": "joyce-hair-pilot/booking-page-copy",
                 "sessionId": "a5b92fc8-136c-4ee3-8736-6d3dc5344056", "cwd": str(self.root)}]
        model = surface_board.build_model(self.root, now=self.now, rows=rows)
        self.assertTrue(model["exists"])
        self.assertEqual(model["projects"]["joyce-hair-pilot"], "Joyce Hair Design")
        lane = next(l for l in model["lanes"] if l["name"] == "In progress")
        self.assertEqual(lane["cards"][0]["state"], "working")
        self.assertEqual(lane["cards"][0]["session_id"], "a5b92fc8-136c-4ee3-8736-6d3dc5344056")
        self.assertEqual(model["errors"], [])
        self.assertEqual(model["waiting_on_you"], 0)
        self.assertEqual(len(model["archive"]), 1)
        self.assertEqual(model["archive"][0]["outcome"], "dropped")
        self.assertEqual(model["archive"][0]["note"], "not now")

    def test_grammar_errors_are_reported_not_repaired(self):
        workboard.init(self.root)
        p = workboard.paths(self.root)["board"]
        p.write_text(p.read_text(encoding="utf-8").replace("## Queued\n", "## Queued\n\n- [ ] T-2026-09-09-01 · p · d\n", 1),
                     encoding="utf-8")
        model = surface_board.build_model(self.root, now=self.now, rows=[])
        self.assertTrue(model["exists"])
        self.assertTrue(any("needs id" in e for e in model["errors"]))
        self.assertEqual(p.read_text(encoding="utf-8").count("T-2026-09-09-01"), 1)

    def test_roster_cache_avoids_repeated_cli_calls(self):
        calls = []
        original = surface_board.workboard.roster
        surface_board.workboard.roster = lambda root, timeout=6.0: calls.append(1) or []
        try:
            surface_board._roster_cache.clear()
            surface_board.cached_roster(self.root)
            surface_board.cached_roster(self.root)
        finally:
            surface_board.workboard.roster = original
            surface_board._roster_cache.clear()
        self.assertEqual(len(calls), 1)

    def test_api_route_is_registered(self):
        from system.server.lib.surface import api
        routes = [pattern for pattern, *_ in api.make_handlers(self.root)]
        self.assertIn(r"/api/board", routes)


if __name__ == "__main__":
    unittest.main()
