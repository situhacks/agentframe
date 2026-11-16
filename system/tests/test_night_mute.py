"""night_mute.ps1: the night window, the idle threshold, and that it never unmutes.

Runs the script with -DryRun -Once and a pretend clock and idle time, so no speaker state changes.
Windows only (PowerShell and Core Audio).
"""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "system" / "tools" / "night_mute.ps1"


def run(*args, af_root):
    env = dict(os.environ, AF_ROOT=str(af_root))
    p = subprocess.run(
        ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(SCRIPT), *args],
        capture_output=True, text=True, env=env, timeout=120,
    )
    return p.returncode, p.stdout.strip()


@unittest.skipUnless(sys.platform == "win32", "Windows only")
class NightMuteTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def dry(self, now, idle, *extra):
        return run("-Now", now, "-IdleSeconds", str(idle), "-DryRun", "-Once", *extra, af_root=self.root)

    def test_idle_past_the_threshold_inside_the_window_mutes(self):
        self.assertEqual(self.dry("23:45", 20 * 60), (0, "would mute"))
        self.assertEqual(self.dry("03:10", 3 * 3600), (0, "would mute"))

    def test_idle_under_the_threshold_does_nothing(self):
        self.assertEqual(self.dry("23:45", 19 * 60), (0, ""))

    def test_daytime_never_mutes_however_long_idle(self):
        for now in ("06:00", "09:30", "14:00", "23:29"):
            code, out = self.dry(now, 10 * 3600)
            self.assertEqual((code, out), (0, "outside the window; nothing to do"), now)

    def test_the_window_and_threshold_follow_the_options(self):
        self.assertEqual(self.dry("22:15", 6 * 60, "-Start", "22:00", "-IdleMinutes", "5"), (0, "would mute"))
        self.assertEqual(self.dry("21:59", 6 * 60, "-Start", "22:00", "-IdleMinutes", "5")[1], "outside the window; nothing to do")

    def test_each_run_logs_a_line(self):
        self.dry("23:45", 30 * 60)
        log = (self.root / "system" / "logs" / "night-mute.log").read_text(encoding="utf-8-sig")
        self.assertIn("watching until 06:00", log)

    def test_check_reports_and_changes_nothing(self):
        code, out = run("-Check", af_root=self.root)
        self.assertEqual(code, 0)
        self.assertRegex(out, r"output devices \d+, not muted \d+$")

    def test_now_implies_once_so_a_fixed_clock_cannot_loop(self):
        code, _ = run("-Now", "23:45", "-IdleSeconds", "60", "-DryRun", af_root=self.root)
        self.assertEqual(code, 0)

    def test_start_equal_to_end_is_no_window(self):
        self.assertEqual(self.dry("23:45", 3600, "-Start", "23:30", "-End", "23:30")[1], "outside the window; nothing to do")

    def test_a_bad_start_time_exits_2_and_logs(self):
        code, _ = self.dry("23:45", 3600, "-Start", "11:30pm")
        self.assertEqual(code, 2)
        log = (self.root / "system" / "logs" / "night-mute.log").read_text(encoding="utf-8-sig")
        self.assertIn("bad -Start", log)

    def test_the_script_never_unmutes(self):
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("SetMute(1, IntPtr.Zero)", text)
        self.assertNotIn("SetMute(0", text)
        self.assertEqual(text.count(".SetMute("), 1)  # the one call, and it mutes


if __name__ == "__main__":
    unittest.main()
