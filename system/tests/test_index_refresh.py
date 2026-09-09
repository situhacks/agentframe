"""The session-start index refresh: decides from the index's own timestamp, spawns at most
one detached update, and never touches a machine that has no index yet."""
import datetime
import importlib.util
import os
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

HOOK = Path(__file__).resolve().parents[1] / "hooks" / "index_refresh.py"
_spec = importlib.util.spec_from_file_location("index_refresh", HOOK)
refresh = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(refresh)


class RefreshDecision(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        root = Path(self._tmp.name)
        (root / "system" / "index").mkdir(parents=True)
        for name, value in (("ROOT", root), ("DB", root / "system" / "index" / "vault.db"),
                            ("LOCK", root / "system" / "index" / "refresh.lock"),
                            ("LOG", root / "system" / "index" / "refresh.log")):
            p = patch.object(refresh, name, value)
            p.start()
            self.addCleanup(p.stop)
        self.addCleanup(self._tmp.cleanup)

    def make_db(self, hours_old):
        stamp = (datetime.datetime.now() - datetime.timedelta(hours=hours_old)).isoformat(timespec="seconds")
        con = sqlite3.connect(refresh.DB)
        con.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        con.execute("INSERT INTO meta VALUES ('updated_at', ?)", (stamp,))
        con.commit()
        con.close()

    def test_no_index_means_no_spawn(self):
        self.assertIn("no index yet", refresh.decide())

    def test_fresh_index_is_left_alone(self):
        self.make_db(hours_old=3)
        self.assertTrue(refresh.decide().startswith("fresh"))

    def test_stale_index_spawns_one_detached_update_and_locks(self):
        self.make_db(hours_old=refresh.REFRESH_AFTER_HOURS + 1)
        self.assertEqual(refresh.decide(), "spawn")
        with patch.object(refresh.subprocess, "Popen") as popen:
            refresh.spawn()
        argv = popen.call_args.args[0]
        self.assertEqual(argv[-4:-1], [str(refresh.ROOT / "system" / "af.py"), "index", "update"])
        self.assertEqual(argv[-1], str(refresh.LOCK))
        self.assertTrue(refresh.LOCK.is_file())
        self.assertEqual(refresh.decide(), "a refresh is already running")

    def test_a_stale_lock_does_not_block_forever(self):
        self.make_db(hours_old=refresh.REFRESH_AFTER_HOURS + 1)
        refresh.LOCK.write_text("dead\n", encoding="utf-8")
        old = (datetime.datetime.now() - datetime.timedelta(hours=refresh.LOCK_STALE_HOURS + 1)).timestamp()
        os.utime(refresh.LOCK, (old, old))
        self.assertEqual(refresh.decide(), "spawn")

    def test_main_never_fails_the_session(self):
        refresh.DB.write_text("not a database", encoding="utf-8")
        with patch.object(refresh.sys, "argv", ["index_refresh.py", "--harness", "claude"]):
            self.assertEqual(refresh.main(), 0)


if __name__ == "__main__":
    unittest.main()
