import asyncio
import tempfile
import unittest
from pathlib import Path

from system.server.lib import watcher


class TestGlobMatching(unittest.TestCase):
    def test_recursive_file_glob_matches_direct_and_nested_files(self):
        pattern = watcher._compile_glob("workspace/projects/*/phase-*/**/*.html")
        self.assertTrue(pattern.match("workspace/projects/alpha/phase-1/direct.html"))
        self.assertTrue(pattern.match("workspace/projects/alpha/phase-1/nested/page.html"))
        self.assertFalse(pattern.match("workspace/projects/alpha/phase-1/nested/page.md"))

    def test_recursive_directory_glob_matches_all_descendants(self):
        pattern = watcher._compile_glob(
            "workspace/projects/*/phase-3-planning/design-language/**"
        )
        self.assertTrue(
            pattern.match("workspace/projects/alpha/phase-3-planning/design-language/tokens.yaml")
        )
        self.assertTrue(
            pattern.match(
                "workspace/projects/alpha/phase-3-planning/design-language/preview/assets/tokens.css"
            )
        )


class TestWatchdogWatcher(unittest.TestCase):
    def test_examine_reports_only_matching_changes_and_runs_task_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            changed = root / "workspace/projects/alpha/phase-1/nested/page.html"
            ignored = root / "workspace/projects/alpha/project.md"
            changed.parent.mkdir(parents=True)
            changed.write_text("<p>x</p>", encoding="utf-8")
            ignored.write_text("---", encoding="utf-8")

            calls = []
            subject = watcher.WatchdogWatcher(root)
            subject.watch(
                "workspace/projects/*/phase-*/**/*.html",
                func=lambda paths: calls.append(paths),
                delay=0.5,
            )
            subject._queue_changed_path(ignored)
            self.assertEqual(subject.examine(), (None, None))

            subject._queue_changed_path(changed)
            subject._queue_changed_path(changed)
            path, delay = subject.examine()
            self.assertEqual(path, "workspace/projects/alpha/phase-1/nested/page.html")
            self.assertEqual(delay, 0.5)
            self.assertEqual(calls, [[path]])

    def test_real_observer_dispatches_a_watched_html_change(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            changed = root / "workspace/projects/alpha/phase-1/page.html"
            changed.parent.mkdir(parents=True)
            changed.write_text("<p>before</p>", encoding="utf-8")
            subject = watcher.WatchdogWatcher(root)
            subject.watch("workspace/projects/*/phase-*/**/*.html", delay=0.5)
            seen = []

            async def scenario():
                triggered = asyncio.Event()

                def callback():
                    change = subject.examine()
                    if change[0]:
                        seen.append(change)
                        triggered.set()

                subject.start(callback)
                changed.write_text("<p>after</p>", encoding="utf-8")
                await asyncio.wait_for(triggered.wait(), timeout=3)

            try:
                asyncio.run(scenario())
            finally:
                subject.close()

            self.assertEqual(
                seen,
                [("workspace/projects/alpha/phase-1/page.html", 0.5)],
            )


if __name__ == "__main__":
    unittest.main()
