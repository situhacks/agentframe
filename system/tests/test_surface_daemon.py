import json
import socket
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from system.server.lib.surface import daemon


class TestLockFile(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.lock_path = Path(self.tmp.name) / ".surface.lock"

    def tearDown(self):
        self.tmp.cleanup()

    def test_write_and_read_roundtrip(self):
        daemon.write_lock(self.lock_path, port=8080, root="C:/somewhere")
        lock = daemon.read_lock(self.lock_path)
        self.assertEqual(lock["port"], 8080)
        self.assertEqual(lock["root"], "C:/somewhere")
        self.assertIn("pid", lock)
        self.assertIn("started_at", lock)

    def test_read_missing_or_corrupt_returns_none(self):
        self.assertIsNone(daemon.read_lock(self.lock_path))
        self.lock_path.write_text("{not json", encoding="utf-8")
        self.assertIsNone(daemon.read_lock(self.lock_path))


class TestPortPicking(unittest.TestCase):
    def test_prefers_free_port(self):
        port = daemon.pick_port(preferred=48620)
        self.assertEqual(port, 48620)

    def test_skips_occupied_port(self):
        blocker = socket.socket()
        blocker.bind(("localhost", 48621))
        blocker.listen(1)
        try:
            port = daemon.pick_port(preferred=48621)
            self.assertEqual(port, 48622)
        finally:
            blocker.close()


class TestHealthProbe(unittest.TestCase):
    def test_unreachable_port_is_unhealthy(self):
        self.assertFalse(daemon.is_healthy(48699, expected_root="C:/x", timeout=0.3))


class TestSurfaceUrl(unittest.TestCase):
    def test_builds_named_views(self):
        self.assertEqual(daemon.surface_url(8080, view="dashboard"), "http://localhost:8080/#/dashboard")
        self.assertEqual(daemon.surface_url(8081, view="calendar"), "http://localhost:8081/#/calendar")

    def test_builds_encoded_artifact_deep_link(self):
        url = daemon.surface_url(
            8080,
            view="preview",
            project="demo project",
            file="phase-1/file name.md",
        )
        self.assertEqual(
            url,
            "http://localhost:8080/#/preview?project=demo+project&file=phase-1%2Ffile+name.md",
        )

    def test_rejects_unknown_view(self):
        with self.assertRaises(ValueError):
            daemon.surface_url(8080, view="unknown")


class TestStop(unittest.TestCase):
    def test_stops_only_health_verified_workspace_process(self):
        health = {
            "ok": True,
            "pid": 4321,
            "workspace_root": str(daemon.PROJECT_ROOT),
        }
        with tempfile.TemporaryDirectory() as tmpdir, mock.patch.object(
            daemon, "read_health", side_effect=[health, None]
        ), mock.patch.object(daemon.os, "kill") as kill:
            result = daemon.stop(preferred_port=8080, lock_path=Path(tmpdir) / ".surface.lock")

        self.assertEqual(result, {"stopped": True, "port": 8080, "pid": 4321})
        kill.assert_called_once_with(4321, daemon.signal.SIGTERM)

    def test_does_not_kill_a_different_workspace(self):
        health = {"ok": True, "pid": 4321, "workspace_root": "C:/somewhere-else"}
        with tempfile.TemporaryDirectory() as tmpdir, mock.patch.object(
            daemon, "read_health", return_value=health
        ), mock.patch.object(daemon.os, "kill") as kill:
            result = daemon.stop(preferred_port=8080, lock_path=Path(tmpdir) / ".surface.lock")

        self.assertEqual(result, {"stopped": False})
        kill.assert_not_called()


if __name__ == "__main__":
    unittest.main()
