"""Static checks for system/tools/timers.ps1, the script that shows, registers or removes
AgentFrame's scheduled jobs on this machine, and for the batch task's two scripts.

Nothing here runs them or touches a scheduled task. The scripts are parsed with the Windows
PowerShell 5.1 parser, checked as text for 7-only syntax and for non-ASCII bytes, and each job
timers.ps1 lists is matched against the task name its register-*.ps1 script sets.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
TOOLS = ROOT / "system" / "tools"
TIMERS = TOOLS / "timers.ps1"
BATCH_SCRIPTS = [TOOLS / "watch-batch.ps1", TOOLS / "register-watch-batch.ps1"]
SCRIPTS = [TIMERS, *BATCH_SCRIPTS]
RETIRED = ["AgentFrame loop watch", "AgentFrame bills watch", "AgentFrame loop sweep", "AgentFrame reminders"]

# Windows PowerShell 5.1 has none of these; checked as text, since a newer parser accepts them.
PS7_ONLY = [
    (re.compile(r"&&"), "&& pipeline chain"),
    (re.compile(r"\|\|"), "|| pipeline chain"),
    (re.compile(r"\?\?"), "?? null-coalescing"),
    (re.compile(r"\?\."), "?. null-conditional member access"),
    (re.compile(r"\s\?\s[^\n]*\s:\s"), "?: ternary"),
    (re.compile(r"-AsHashtable\b", re.IGNORECASE), "-AsHashtable"),
]


def parse_errors(paths: list[Path]) -> dict[str, list[str]]:
    """Parse each file with Windows PowerShell's parser; returns path -> error lines. Runs nothing."""
    quoted = ",".join("'" + str(p).replace("'", "''") + "'" for p in paths)
    command = (
        "$out = @{}; foreach ($f in @(" + quoted + ")) { "
        "$t = $null; $e = $null; "
        "[void][System.Management.Automation.Language.Parser]::ParseFile($f, [ref]$t, [ref]$e); "
        "$out[$f] = @($e | ForEach-Object { 'line ' + $_.Extent.StartLineNumber + ': ' + $_.Message }) }; "
        "$out | ConvertTo-Json -Depth 3 -Compress"
    )
    proc = subprocess.run(
        ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", command],
        capture_output=True, text=True, timeout=120,
    )
    if proc.returncode != 0:
        raise AssertionError(f"the PowerShell parser could not run: {proc.stderr.strip()}")
    data = json.loads(proc.stdout)
    return {k: (v if isinstance(v, list) else ([v] if v else [])) for k, v in data.items()}


def ps7_only_syntax(text: str) -> list[str]:
    """PowerShell 7-only constructs, by text, outside comment lines."""
    code = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
    return [label for pat, label in PS7_ONLY if pat.search(code)]


@unittest.skipUnless(sys.platform == "win32" and shutil.which("powershell.exe"), "needs Windows PowerShell")
class ParseTests(unittest.TestCase):
    def test_timers_and_the_batch_scripts_parse_in_windows_powershell(self):
        errors = parse_errors(SCRIPTS)
        bad = {Path(k).name: v for k, v in errors.items() if v}
        self.assertEqual(bad, {}, f"parse errors: {bad}")

    def test_the_parser_check_catches_an_error_and_7_only_syntax(self):
        with tempfile.TemporaryDirectory() as d:
            broken = Path(d) / "broken.ps1"
            broken.write_text("if ($x) { 'open'\n", encoding="utf-8")
            seven = Path(d) / "seven.ps1"
            seven.write_text("git status && echo ok\n$a = $b ? 1 : 2\n", encoding="utf-8")
            errors = parse_errors([broken, seven])
        self.assertTrue(errors[str(broken)], "an unclosed brace must be reported")
        self.assertTrue(errors[str(seven)], "&& and ?: must be reported by the 5.1 parser")


class EncodingTests(unittest.TestCase):
    def test_the_scripts_are_ascii_so_windows_powershell_reads_them_right(self):
        for path in SCRIPTS:
            bad = [i for i, b in enumerate(path.read_bytes()) if b > 127]
            self.assertEqual(bad, [], f"{path.name} has non-ASCII bytes; 5.1 reads a BOM-less file as the ANSI code page")


class SyntaxTextTests(unittest.TestCase):
    def test_the_scripts_use_no_powershell_7_only_syntax(self):
        for path in SCRIPTS:
            problems = ps7_only_syntax(path.read_text(encoding="utf-8"))
            self.assertEqual(problems, [], f"{path.name}: PowerShell 7-only syntax; Windows PowerShell 5.1 runs this: {problems}")

    def test_the_text_check_catches_each_construct(self):
        sample = "a && b\nc || d\n$x = $y ?? 1\n$n = $o?.Name\n$t = $c ? 1 : 2\nConvertFrom-Json -AsHashtable\n"
        self.assertEqual(len(ps7_only_syntax(sample)), len(PS7_ONLY))
        self.assertEqual(ps7_only_syntax("# a && b in a comment\n$a = 1\n"), [])


class TimersTests(unittest.TestCase):
    def test_timers_use_the_task_names_their_register_scripts_set(self):
        text = TIMERS.read_text(encoding="utf-8")
        pairs = re.findall(r"Script = '([^']+)'; Task = '([^']+)'", text)
        self.assertEqual([task for _, task in pairs], ["AgentFrame batch", "AgentFrame consolidation", "AgentFrame backlog pass"],
                         "timers.ps1 lists the batch task, consolidation and the backlog pass")
        for script, task in pairs:
            source = (TOOLS / script).read_text(encoding="utf-8")
            m = re.search(r"\$taskName = '([^']+)'", source)
            self.assertIsNotNone(m, f"{script} sets no $taskName")
            self.assertEqual(task, m.group(1), f"timers.ps1 names {script}'s job {task!r}, but the script registers {m.group(1)!r}")

    def test_the_register_scripts_it_calls_sit_beside_it(self):
        text = TIMERS.read_text(encoding="utf-8")
        for script, _ in re.findall(r"Script = '([^']+)'; Task = '([^']+)'", text):
            self.assertTrue((TIMERS.parent / script).is_file(), f"{script} is not beside timers.ps1")

    def test_the_four_retired_tasks_are_named_in_one_list_and_nothing_registers_them(self):
        text = TIMERS.read_text(encoding="utf-8")
        listed = re.search(r"\$retired = @\(([^)]*)\)", text)
        self.assertIsNotNone(listed, "timers.ps1 has no $retired list")
        self.assertEqual(re.findall(r"'([^']+)'", listed.group(1)), RETIRED)
        for path in sorted(TOOLS.glob("register-*.ps1")):
            m = re.search(r"\$taskName = '([^']+)'", path.read_text(encoding="utf-8"))
            self.assertNotIn(m.group(1), RETIRED, f"{path.name} still registers a retired task")
        for name in ("register-loop-watch.ps1", "register-bills-watch.ps1", "register-loop-sweep.ps1",
                     "register-daily-reminders.ps1"):
            self.assertFalse((TOOLS / name).exists(), f"{name} should be gone")

    def test_register_and_unregister_both_remove_the_retired_tasks_and_show_names_a_leftover(self):
        text = TIMERS.read_text(encoding="utf-8")
        self.assertIn("function Remove-Retired", text)
        self.assertIn("foreach ($task in $retired)", text)
        flat = " ".join(text.split())
        self.assertIn("if ($Register -or $Unregister) { Remove-Retired }", flat)
        self.assertIn("[retired, still here]", text)
        self.assertIn("Unregister-ScheduledTask -TaskName $task -Confirm:$false", text)

    def test_show_mode_reads_the_batch_tasks_sweep_mode_from_its_arguments(self):
        text = TIMERS.read_text(encoding="utf-8")
        self.assertIn("$t.Script -eq 'register-watch-batch.ps1'", text)
        self.assertIn("-SweepLive", text)
        self.assertIn("live sweep: closes loops on proof", text)
        self.assertIn("report-only sweep", text)

    def test_the_retired_names_appear_nowhere_else_in_the_tools(self):
        for path in sorted(TOOLS.glob("*.ps1")) + sorted(TOOLS.glob("*.py")):
            if path == TIMERS:
                continue
            text = path.read_text(encoding="utf-8")
            for name in RETIRED:
                self.assertNotIn(name, text, f"{path.name} names the retired task {name!r}")


class BatchTaskTests(unittest.TestCase):
    def test_the_batch_task_is_the_name_the_session_start_and_doctor_lines_point_at(self):
        source = (TOOLS / "register-watch-batch.ps1").read_text(encoding="utf-8")
        self.assertEqual(re.search(r"\$taskName = '([^']+)'", source).group(1), "AgentFrame batch")
        rust = (ROOT / "system" / "af" / "src" / "loop_watch.rs").read_text(encoding="utf-8")
        self.assertIn('const TASK: &str = "AgentFrame batch";', rust)
        watch = (ROOT / "system" / "af" / "src" / "watch.rs").read_text(encoding="utf-8")
        self.assertIn("'AgentFrame batch'", watch)


if __name__ == "__main__":
    unittest.main()
