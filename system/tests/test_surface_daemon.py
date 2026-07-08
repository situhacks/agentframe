import json
import socket
import tempfile
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
