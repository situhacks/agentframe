"""Static checks for the laptop kit in system/tools/laptop/.

The scripts change power, sharing and firewall settings and start long-running sessions on another
machine, so nothing here runs them whole. Each .ps1 is parsed with the Windows PowerShell 5.1
parser and checked as text for 7-only syntax, and every kit file is scanned for private facts: user,
host and drive names other than the documented defaults, UNC hosts, IP and email addresses, Discord
ids, home paths, and tokens. Two pieces are evaluated alone, under their script's own param block:
start-agent.ps1's marked command-line block and the statements that build register-agent-logon.ps1's
argument line. Both only assign variables.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
KIT = ROOT / "system" / "tools" / "laptop"
TOOLS = ROOT / "system" / "tools"
SCRIPTS = ["setup-laptop.ps1", "start-agent.ps1", "register-agent-logon.ps1", "timers.ps1"]

ALLOWED_DRIVE_PATHS = {"C:\\afhub"}
DRIVE_PATH = re.compile(r"\b[A-Za-z]:[\\/][^\s'\"`)\]|,;]*")
UNC_HOST = re.compile(r"\\\\[A-Za-z0-9][A-Za-z0-9_.-]*\\")
TOKENS = [
    re.compile(r"[A-Za-z0-9_-]{23,28}\.[A-Za-z0-9_-]{6,7}\.[A-Za-z0-9_-]{27,}"),  # Discord bot token
    re.compile(r"discord(?:app)?\.com/api/webhooks/\d"),
    re.compile(r"\bsk-[A-Za-z0-9_-]{20,}"),
]
IPV4 = re.compile(r"(?<![\d.])(?:\d{1,3}\.){3}\d{1,3}(?![\d.])")
ALLOWED_IPV4 = ("127.", "0.0.0.0", "192.0.2.", "198.51.100.", "203.0.113.")  # loopback, any, documentation ranges
EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
ALLOWED_EMAIL_DOMAINS = ("example.com", "example.org", "example.net")
DISCORD_ID = re.compile(r"(?<!\d)\d{17,20}(?!\d)")
POSIX_HOME = re.compile(r"/[A-Za-z]/Users/(?!<)[^/\s'\"`]+", re.IGNORECASE)
# Windows PowerShell 5.1 has none of these; checked as text, since a newer parser accepts them.
PS7_ONLY = [
    (re.compile(r"&&"), "&& pipeline chain"),
    (re.compile(r"\|\|"), "|| pipeline chain"),
    (re.compile(r"\?\?"), "?? null-coalescing"),
    (re.compile(r"\?\."), "?. null-conditional member access"),
    (re.compile(r"\s\?\s[^\n]*\s:\s"), "?: ternary"),
    (re.compile(r"-AsHashtable\b", re.IGNORECASE), "-AsHashtable"),
]


def kit_files() -> list[Path]:
    return [KIT / s for s in SCRIPTS] + [KIT / "README.md"]


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


def private_findings(text: str) -> list[str]:
    """Drive paths outside the defaults, UNC host names, this machine's user and host names, tokens."""
    found = []
    for m in DRIVE_PATH.finditer(text):
        path = m.group(0).rstrip(".:")
        if path not in ALLOWED_DRIVE_PATHS and not any(path.startswith(a + "\\") for a in ALLOWED_DRIVE_PATHS):
            found.append(f"drive path {path!r}; use a parameter, $PSScriptRoot, or an environment variable")
    for m in UNC_HOST.finditer(text):
        found.append(f"UNC host {m.group(0)!r}; write \\\\<laptop-name>\\ or \\\\$env:COMPUTERNAME\\")
    for var in ("USERNAME", "COMPUTERNAME", "USERDOMAIN"):
        value = os.environ.get(var, "")
        if len(value) >= 3 and re.search(r"\b" + re.escape(value) + r"\b", text, re.IGNORECASE):
            found.append(f"this machine's {var} appears; use $env:{var} or a placeholder")
    for pat in TOKENS:
        if pat.search(text):
            found.append(f"something shaped like a credential ({pat.pattern[:30]}...)")
    for m in IPV4.finditer(text):
        if not m.group(0).startswith(ALLOWED_IPV4):
            found.append(f"IPv4 address {m.group(0)!r}; use a placeholder or a documentation address (192.0.2.x)")
    for m in EMAIL.finditer(text):
        if not m.group(0).lower().endswith(ALLOWED_EMAIL_DOMAINS):
            found.append(f"email address {m.group(0)!r}; use a placeholder")
    for m in DISCORD_ID.finditer(text):
        found.append(f"a {len(m.group(0))}-digit number shaped like a Discord id; use a placeholder")
    for m in POSIX_HOME.finditer(text):
        found.append(f"home path {m.group(0)!r}; use $env:USERPROFILE or a placeholder")
    return found


def ps7_only_syntax(text: str) -> list[str]:
    """PowerShell 7-only constructs, by text, outside comment lines."""
    code = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
    return [label for pat, label in PS7_ONLY if pat.search(code)]


@unittest.skipUnless(sys.platform == "win32" and shutil.which("powershell.exe"), "needs Windows PowerShell")
class ParseTests(unittest.TestCase):
    def test_every_kit_script_parses_in_windows_powershell(self):
        errors = parse_errors([KIT / s for s in SCRIPTS])
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
    def test_kit_scripts_are_ascii_so_windows_powershell_reads_them_right(self):
        for s in SCRIPTS:
            data = (KIT / s).read_bytes()
            bad = [i for i, b in enumerate(data) if b > 127]
            self.assertEqual(bad, [], f"{s} has non-ASCII bytes; 5.1 reads a BOM-less file as the ANSI code page")


class PrivacyTests(unittest.TestCase):
    def test_kit_files_exist(self):
        for f in kit_files():
            self.assertTrue(f.exists(), f"missing {f.relative_to(ROOT)}")

    def test_no_private_fact_in_the_kit(self):
        problems = {}
        for f in kit_files():
            found = private_findings(f.read_text(encoding="utf-8"))
            if found:
                problems[f.name] = found
        self.assertEqual(problems, {}, f"private facts in the laptop kit: {problems}")

    def test_the_scan_flags_a_hard_coded_vault_path_and_host(self):
        sample = "$vault = 'D:\\Vaults\\Mine'\nnet use \\\\desk01\\hub\n"
        found = private_findings(sample)
        self.assertTrue(any("drive path" in f for f in found))
        self.assertTrue(any("UNC host" in f for f in found))
        self.assertEqual(private_findings("$HubPath = 'C:\\afhub'\n\\\\<laptop-name>\\afhub\n"), [])

    def test_the_scan_flags_addresses_ids_and_home_paths(self):
        sample = "ping 10.1.2.3\nmail someone@mailhost.io\nchannel 123456789012345678\ncd /c/Users/someone/vault\n"
        found = " | ".join(private_findings(sample))
        for kind in ("IPv4", "email", "Discord id", "home path"):
            self.assertIn(kind, found)
        clean = "route 0.0.0.0/0 and 127.0.0.1 and 192.0.2.10\nuser@example.com\n/c/Users/<name>/vault\n"
        self.assertEqual(private_findings(clean), [])


class SyntaxTextTests(unittest.TestCase):
    def test_kit_scripts_use_no_powershell_7_only_syntax(self):
        problems = {s: ps7_only_syntax((KIT / s).read_text(encoding="utf-8")) for s in SCRIPTS}
        problems = {k: v for k, v in problems.items() if v}
        self.assertEqual(problems, {}, f"PowerShell 7-only syntax; Windows PowerShell 5.1 runs these: {problems}")

    def test_the_text_check_catches_each_construct(self):
        sample = "a && b\nc || d\n$x = $y ?? 1\n$n = $o?.Name\n$t = $c ? 1 : 2\nConvertFrom-Json -AsHashtable\n"
        self.assertEqual(len(ps7_only_syntax(sample)), len(PS7_ONLY))
        self.assertEqual(ps7_only_syntax("# a && b in a comment\n$a = 1\n"), [])


DISCORD_CHANNEL = "plugin:discord@claude-plugins-official"
BLOCK_START = re.compile(r"^# =+ THE SESSION COMMAND LINE =+$")
BLOCK_END = re.compile(r"^# =+$")


def command_line_block() -> str:
    """The marked block at the top of start-agent.ps1: the one place the session command line is built."""
    lines = (KIT / "start-agent.ps1").read_text(encoding="utf-8").splitlines()
    start = next(i for i, line in enumerate(lines) if BLOCK_START.match(line))
    end = next(i for i in range(start + 1, len(lines)) if BLOCK_END.match(lines[i]))
    return "\n".join(lines[start + 1:end])


def run_powershell_file(body: str) -> dict:
    """Runs a short PowerShell script written to a temporary file and returns its JSON output."""
    with tempfile.TemporaryDirectory() as d:
        path = Path(d) / "probe.ps1"
        path.write_text(body, encoding="utf-8")
        proc = subprocess.run(
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", str(path)],
            capture_output=True, text=True, timeout=120,
        )
    if proc.returncode != 0:
        raise AssertionError(f"PowerShell failed: {proc.stderr.strip()}")
    return json.loads(proc.stdout)


def ps_quote(text: str) -> str:
    return "'" + text.replace("'", "''") + "'"


def session_args(*switches: str) -> list[str]:
    """$SessionArgs from the marked block alone, under start-agent.ps1's own param block. Starts nothing."""
    with tempfile.TemporaryDirectory() as d:
        block = Path(d) / "block.ps1"
        block.write_text(command_line_block(), encoding="utf-8")
        body = (
            "$t = $null; $e = $null\n"
            f"$ast = [System.Management.Automation.Language.Parser]::ParseFile({ps_quote(str(KIT / 'start-agent.ps1'))}, [ref]$t, [ref]$e)\n"
            f"$blockText = [System.IO.File]::ReadAllText({ps_quote(str(block))})\n"
            "$sb = [scriptblock]::Create($ast.ParamBlock.Extent.Text + \"`n\" + $blockText + \"`n\" + '$SessionArgs')\n"
            f"$argv = @(& $sb {' '.join(switches)})\n"
            "ConvertTo-Json -InputObject @($argv) -Compress\n"
        )
        return run_powershell_file(body)


def register_arg_line(*options: str) -> dict:
    """The argument line register-agent-logon.ps1 gives the task, from its statements that build $argLine
    (from `$argLine =` up to `$action =`), under its own param block, plus start-agent.ps1's parameter
    names. Registers nothing."""
    body = (
        "$t = $null; $e = $null\n"
        f"$reg = [System.Management.Automation.Language.Parser]::ParseFile({ps_quote(str(KIT / 'register-agent-logon.ps1'))}, [ref]$t, [ref]$e)\n"
        f"$start = [System.Management.Automation.Language.Parser]::ParseFile({ps_quote(str(KIT / 'start-agent.ps1'))}, [ref]$t, [ref]$e)\n"
        "$statements = @($reg.EndBlock.Statements)\n"
        "$from = -1; $to = -1\n"
        "for ($i = 0; $i -lt $statements.Count; $i++) {\n"
        "    $s = $statements[$i].Extent.Text\n"
        "    if ($from -lt 0 -and $s -match '^\\$argLine\\s*=') { $from = $i }\n"
        "    if ($from -ge 0 -and $to -lt 0 -and $s -match '^\\$action\\s*=') { $to = $i }\n"
        "}\n"
        "$build = ($statements[$from..($to - 1)] | ForEach-Object { $_.Extent.Text }) -join \"`n\"\n"
        "$sb = [scriptblock]::Create($reg.ParamBlock.Extent.Text + \"`n`$script = 'start-agent.ps1'`n\" + $build + \"`n`$argLine\")\n"
        f"$line = & $sb {' '.join(options)}\n"
        "$names = @($start.ParamBlock.Parameters | ForEach-Object { $_.Name.VariablePath.UserPath })\n"
        "ConvertTo-Json -InputObject @{ line = \"$line\"; startParams = $names } -Compress\n"
    )
    return run_powershell_file(body)


@unittest.skipUnless(sys.platform == "win32" and shutil.which("powershell.exe"), "needs Windows PowerShell")
class CommandLineTests(unittest.TestCase):
    def test_without_discord_the_session_has_no_channels_flag(self):
        argv = session_args()
        self.assertNotIn("--channels", argv)
        self.assertNotIn(DISCORD_CHANNEL, argv)
        for flag in ("--remote-control", "--chrome", "--permission-mode"):
            self.assertIn(flag, argv)

    def test_discord_adds_the_channels_pair(self):
        argv = session_args("-Discord")
        self.assertIn("--channels", argv)
        self.assertEqual(argv[argv.index("--channels") + 1], DISCORD_CHANNEL)
        self.assertEqual(argv.count("--channels"), 1)

    def test_the_session_name_and_permission_mode_reach_the_command_line(self):
        argv = session_args("-SessionName", "'Test box'", "-PermissionMode", "acceptEdits")
        self.assertEqual(argv[argv.index("--remote-control") + 1], "Test box")
        self.assertEqual(argv[argv.index("--permission-mode") + 1], "acceptEdits")

    def test_the_channel_is_named_only_inside_the_marked_block(self):
        text = (KIT / "start-agent.ps1").read_text(encoding="utf-8")
        self.assertEqual(text.count(DISCORD_CHANNEL), command_line_block().count(DISCORD_CHANNEL))
        self.assertEqual(text.count("--channels"), command_line_block().count("--channels"))

    def test_register_passes_discord_through_only_when_given(self):
        off = register_arg_line()
        on = register_arg_line("-Discord", "-LockScreen")
        self.assertNotIn("-Discord", off["line"])
        self.assertRegex(on["line"], r"(?:^|\s)-Discord(?:\s|$)")
        self.assertRegex(on["line"], r"(?:^|\s)-LockScreen(?:\s|$)")

    def test_every_option_register_passes_is_a_start_agent_parameter(self):
        result = register_arg_line("-Discord", "-LockScreen", "-SessionName", "'Test box'",
                                   "-PermissionMode", "auto", "-SyncMinutes", "20", "-NotifyChannel", "status")
        after_file = result["line"].split('.ps1"', 1)[1]
        passed = re.findall(r"(?:^|\s)-([A-Za-z]+)", after_file)
        self.assertEqual(len(passed), 6, result["line"])
        known = {n.lower() for n in result["startParams"]}
        unknown = [p for p in passed if p.lower() not in known]
        self.assertEqual(unknown, [], f"register-agent-logon.ps1 passes options start-agent.ps1 lacks: {unknown}")


class TimersTests(unittest.TestCase):
    def test_timers_use_the_task_names_their_register_scripts_set(self):
        text = (KIT / "timers.ps1").read_text(encoding="utf-8")
        pairs = re.findall(r"Script = '([^']+)'; Task = '([^']+)'", text)
        self.assertEqual(len(pairs), 5, "timers.ps1 lists the loop watch, bills watch, reminders, consolidation and backlog pass")
        for script, task in pairs:
            source = (TOOLS / script).read_text(encoding="utf-8")
            m = re.search(r"\$taskName = '([^']+)'", source)
            self.assertIsNotNone(m, f"{script} sets no $taskName")
            self.assertEqual(task, m.group(1), f"timers.ps1 names {script}'s job {task!r}, but the script registers {m.group(1)!r}")


if __name__ == "__main__":
    unittest.main()
