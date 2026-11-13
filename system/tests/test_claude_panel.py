"""Static checks for the Claude panel driver (system/tools/claude_panel.ps1, version 2), the bridge
extension it talks to (system/vscode-bridge/) and the bridge client (system/tools/panel_bridge.py).

Nothing here runs the driver against a window, opens VS Code, or sends an event. The driver is parsed
with the Windows PowerShell 5.1 parser and read as text; some of its pure functions (the window title
rule, the text and label rules, the model key writer) are cut out of the parse tree and run on their
own in a child PowerShell against temp files. The bridge extension is read as text. The packager is
run into a temp folder, and the client is run against a fake bridge thread in a temp folder.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
DRIVER = ROOT / "system" / "tools" / "claude_panel.ps1"
BRIDGE_DIR = ROOT / "system" / "vscode-bridge"
EXTENSION = BRIDGE_DIR / "extension.js"
BUILD = BRIDGE_DIR / "build.py"
CLIENT = ROOT / "system" / "tools" / "panel_bridge.py"
POWERSHELL = sys.platform == "win32" and shutil.which("powershell.exe")

# The controls the driver may name, and nothing else.
CONTROLS = {"Message input", "Send message", "Switch model", "New session"}
# The actions of the installed bridge 0.1.0, and of the source (0.2.0, which drops group-pick). The driver
# may use only the ones both have.
INSTALLED_ACTIONS = {"ping", "tabs", "open", "reveal", "close-tab", "group-pick"}
BRIDGE_ACTIONS = {"ping", "tabs", "open", "reveal", "close-tab"}
# What the driver may import from Windows: the idle time and the window's state and title, nothing that sends input.
ALLOWED_IMPORTS = {"GetLastInputInfo", "IsIconic", "IsWindow", "GetWindowTextW"}
# Words that would mean synthetic input, clipboard use or taking the foreground.
FORBIDDEN = [
    "SendInput", "keybd_event", "mouse_event", "SendKeys", "SetCursorPos", "clipboard", "SetForegroundWindow",
    "AttachThreadInput", "GetForegroundWindow", "BringWindowToTop", "MOUSEINPUT", "KEYBDINPUT", "GetGUIThreadInfo",
]

# Windows PowerShell 5.1 has none of these; checked as text, since a newer parser accepts them.
PS7_ONLY = [
    (re.compile(r"&&"), "&& pipeline chain"),
    (re.compile(r"\|\|"), "|| pipeline chain"),
    (re.compile(r"\?\?"), "?? null-coalescing"),
    (re.compile(r"\?\."), "?. null-conditional member access"),
    (re.compile(r"\s\?\s[^\n]*\s:\s"), "?: ternary"),
    (re.compile(r"-AsHashtable\b", re.IGNORECASE), "-AsHashtable"),
]


def source() -> str:
    return DRIVER.read_text(encoding="utf-8")


def code_lines(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))


def powershell_only(text: str) -> str:
    """The script's PowerShell code: no comment lines, and not the C# in the Add-Type here-string."""
    return code_lines(re.sub(r"@'\n.*?\n'@", "", text, flags=re.DOTALL))


def csharp_block(text: str) -> str:
    return re.search(r"@'\n(.*?)\n'@", text, flags=re.DOTALL).group(1)


def function_text(name: str) -> str:
    return re.search(rf"function {re.escape(name)}\b.*?\n\}}\n", source(), re.DOTALL).group(0)


def run_powershell(script: str, env: dict | None = None) -> subprocess.CompletedProcess:
    import os

    with tempfile.TemporaryDirectory() as d:
        path = Path(d) / "check.ps1"
        path.write_text(script, encoding="ascii")
        full_env = dict(os.environ)
        full_env.update(env or {})
        return subprocess.run(
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", str(path)],
            capture_output=True, text=True, timeout=120, env=full_env,
        )


def run_functions(names: list[str], body: str, env: dict | None = None):
    """Run `body` in a child PowerShell after defining the driver's named functions, cut from its parse tree.
    The body prints JSON; it is returned parsed."""
    script = (
        "$t = $null; $e = $null\n"
        f"$ast = [System.Management.Automation.Language.Parser]::ParseFile('{DRIVER}', [ref]$t, [ref]$e)\n"
        "$defs = $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)\n"
        f"foreach ($name in @({','.join(repr(n) for n in names)})) {{\n"
        "  $d = @($defs | Where-Object { $_.Name -eq $name })\n"
        "  if ($d.Count -ne 1) { throw \"function $name not found\" }\n"
        "  . ([scriptblock]::Create($d[0].Extent.Text))\n"
        "}\n"
        + body
    )
    proc = run_powershell(script, env)
    if proc.returncode != 0:
        raise AssertionError(f"the child PowerShell failed: {proc.stderr.strip()} {proc.stdout.strip()}")
    return json.loads(proc.stdout)


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class ParseTests(unittest.TestCase):
    def test_the_driver_parses_in_windows_powershell_5_1(self):
        proc = run_powershell(
            "$t = $null; $e = $null\n"
            f"[void][System.Management.Automation.Language.Parser]::ParseFile('{DRIVER}', [ref]$t, [ref]$e)\n"
            "$e | ForEach-Object { 'line ' + $_.Extent.StartLineNumber + ': ' + $_.Message }\n"
            "'version ' + $PSVersionTable.PSVersion.Major + '.' + $PSVersionTable.PSVersion.Minor\n"
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        lines = proc.stdout.strip().splitlines()
        self.assertEqual(lines, [lines[-1]], f"parse errors: {proc.stdout}")
        self.assertTrue(lines[-1].startswith("version 5.1"), f"the parser used is not Windows PowerShell 5.1: {lines[-1]}")


class TextTests(unittest.TestCase):
    def test_the_driver_is_ascii_so_windows_powershell_reads_it_right(self):
        bad = [i for i, b in enumerate(DRIVER.read_bytes()) if b > 127]
        self.assertEqual(bad, [], "non-ASCII bytes: 5.1 reads a BOM-less file as the ANSI code page")

    def test_no_powershell_7_only_syntax(self):
        text = powershell_only(source())
        self.assertIn("function Find-Window", text)
        self.assertEqual([label for pat, label in PS7_ONLY if pat.search(text)], [])

    def test_no_synthetic_input_no_clipboard_and_no_foreground_code_anywhere_in_the_file(self):
        text = source().lower()
        for word in FORBIDDEN:
            self.assertNotIn(word.lower(), text, f"the driver must not mention {word!r}")
        code = code_lines(source()).lower()
        for word in ["mouse", "click", "cursor", "keystroke", "typebatch", "unicode key"]:
            self.assertNotIn(word, code, f"the driver code must not mention {word!r}")

    def test_it_imports_only_the_idle_time_and_the_window_state_from_windows(self):
        imports = set(re.findall(r"DllImport\(\"[^\"]+\"[^)]*\)\]\s*(?:public\s+)?static extern \w+ (\w+)\(", csharp_block(source())))
        self.assertEqual(imports, ALLOWED_IMPORTS)

    def test_it_runs_nothing_else_and_never_sets_the_input_text_through_a_pattern(self):
        text = code_lines(source())
        for banned in ["Invoke-Expression", "iex ", "Start-Process", "Stop-Process", "Invoke-WebRequest", "SetValue(", "Set-Clipboard", "SendWait"]:
            self.assertNotIn(banned, text, f"{banned!r} is not for this driver")

    def test_the_actions_are_the_six_and_no_more(self):
        text = source()
        actions = re.findall(r"^    '([\w-]+)'\s*\{", text, re.MULTILINE)
        self.assertEqual(actions, ["list", "send", "new", "model", "close-untitled", "selftest"])
        params = re.search(r"param\((.*?)\n\)", text, re.DOTALL).group(1)
        self.assertEqual(re.findall(r"\$(\w+)", params), ["Action", "Session", "TextFile", "Model", "GroupId", "IdleSeconds", "WaitMinutes"])
        self.assertIn("[int]$IdleSeconds = 120", text)
        self.assertIn("[double]$WaitMinutes = 20", text)

    def test_text_arrives_only_through_a_file_never_as_an_argument(self):
        text = source()
        self.assertIn("[System.IO.File]::ReadAllText($Path, $Utf8)", text)
        self.assertNotIn("[string]$Text", text.split("# ---------------------------------------------------------------- main")[1])


class ControlTests(unittest.TestCase):
    def test_the_only_controls_named_are_the_ones_the_probes_proved(self):
        text = source()
        named = set(re.findall(r"Find-Named\s+\S+\s+'([^']+)'", text))
        self.assertEqual(named, CONTROLS)
        self.assertIn("'sessions-list-row-'", text)
        for part in ["*statusDot*", "*Running*", "*Idle*", "(^|\\s)active_", "*modelItem_*", "*activeModelItem_*"]:
            self.assertIn(part, text)
        for other in ["New group", "Show command menu", "commandItem", "Permissions", "Effort", "Search sessions", "Message actions"]:
            self.assertNotIn(other, text, f"{other!r} is a control the driver must not touch")

    def test_only_the_allowed_ui_automation_patterns_are_used(self):
        text = code_lines(source())
        used = set(re.findall(r"System\.Windows\.Automation\.(\w+Pattern)\b", text))
        self.assertEqual(used, {"InvokePattern", "ExpandCollapsePattern", "SelectionItemPattern", "WindowPattern", "ValuePattern"})
        # ValuePattern is for reading only.
        self.assertNotIn("SetValue", text)
        self.assertNotIn("SetFocus", text)

    def test_send_is_invoked_in_one_place_after_the_readback_and_an_idle_check(self):
        send = function_text("Send-Now")
        self.assertLess(send.index("Test-Ready"), send.index(".Invoke()"))
        self.assertLess(send.index("Wait-Idle"), send.index("Test-Ready"))
        self.assertLess(send.index("IdleMs()"), send.index(".Invoke()"), "idle is re-checked right before Invoke")
        self.assertEqual(len(re.findall(r"Send message", code_lines(source()))), 1)


class WindowRuleTests(unittest.TestCase):
    def test_every_window_lookup_goes_through_the_one_function_that_applies_the_title_rule(self):
        text = code_lines(source())
        self.assertEqual(text.count("RootElement"), 1, "the desktop is searched in one place only")
        self.assertEqual(text.count("Get-Process"), 1)
        find = function_text("Find-Window")
        for part in ["RootElement", "Test-VaultTitle", "'Chrome_WidgetWin_1'", "ProcessName -ne 'Code'", "Stop-Panel 2 \"no VS Code window", "more than one VS Code window"]:
            self.assertIn(part, find)
        self.assertEqual(text.count("Find-Window"), 2, "defined once, called once (Open-Window)")
        self.assertIn("Find-Window", function_text("Open-Window"))

    def test_a_minimized_window_is_restored_by_the_window_pattern_and_minimized_again(self):
        text = source()
        ensure = function_text("Ensure-Screen")
        self.assertIn("IsIconic", ensure)
        self.assertIn("Set-WindowState 'Normal'", ensure)
        self.assertLess(ensure.index("Wait-Idle"), ensure.index("Set-WindowState 'Normal'"))
        restore = function_text("Restore-Screen")
        self.assertIn("Set-WindowState 'Minimized'", restore)
        self.assertIn("SetWindowVisualState", function_text("Set-WindowState"))
        self.assertIn("InvokePattern", restore, "his previous chat is put back by Invoke on its row")

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_the_title_rule_refuses_a_window_without_the_vault_name(self):
        cases = [
            ("zz-chat - AgentFrame - Visual Studio Code", "AgentFrame", True),
            ("AgentFrame - Visual Studio Code", "AgentFrame", True),
            ("agentframe - visual studio code", "AgentFrame", True),
            ("notes - Other Project - Visual Studio Code", "AgentFrame", False),
            ("AgentFrame - Notepad", "AgentFrame", False),
            ("AgentFrame", "AgentFrame", False),
            ("", "AgentFrame", False),
            ("anything - Visual Studio Code", "", False),
        ]
        got = run_functions(
            ["Test-VaultTitle"],
            "$cases = " + json.dumps([[t, v] for t, v, _ in cases]).replace("'", "''").join(["'", "'"]) + " | ConvertFrom-Json\n"
            "$out = @(foreach ($c in $cases) { [bool](Test-VaultTitle $c[0] $c[1]) })\n"
            "ConvertTo-Json -InputObject $out -Compress\n",
        )
        self.assertEqual(got, [expected for _, _, expected in cases])

    def test_the_worktree_folder_resolves_to_the_vault_root(self):
        text = source()
        self.assertIn(r"'\\\.claude\\worktrees\\[^\\]+$'", text)
        self.assertIn("$LogDir = Join-Path $VaultRoot 'system\\logs'", text)
        self.assertIn("$BridgeBase = Join-Path $VaultRoot 'system\\logs\\panel-bridge'", text)


class TextRuleTests(unittest.TestCase):
    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_a_text_that_starts_with_a_slash_is_read_exactly_as_written(self):
        cases = ["/rename zz-new", "/compact Keep the plan.", " /status", "plain text", "line one\nline two", "/compact a \"quoted\" word"]
        with tempfile.TemporaryDirectory() as d:
            files = []
            for i, text in enumerate(cases):
                p = Path(d) / f"t{i}.txt"
                p.write_bytes(text.replace("\n", "\r\n").encode("utf-8") + b"\r\n\r\n")
                files.append(str(p))
            body = (
                "$Utf8 = New-Object System.Text.UTF8Encoding($false); $MaxChars = 200000\n"
                f"$files = '{json.dumps(files)}' | ConvertFrom-Json\n"
                "$out = @(foreach ($f in $files) { Read-Text $f })\n"
                "ConvertTo-Json -InputObject $out -Compress\n"
            )
            got = run_functions(["Stop-Panel", "Read-Text"], body.replace("\\\\", "\\\\"))
        self.assertEqual(got, cases, "no leading space is added anywhere, line breaks are LF and trailing breaks are dropped")

    def test_the_driver_has_no_leading_space_rule(self):
        text = source()
        self.assertNotIn("Format-TypedText", text)
        self.assertNotIn("leading space", text.lower().replace("no leading space", ""))
        read = function_text("Read-Text")
        self.assertNotIn("' /'", read)
        self.assertNotIn('" /"', read)

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_the_input_must_equal_the_text_exactly_with_a_tolerance_only_for_line_breaks(self):
        cases = [
            ("OK", "OK", True),
            ("ok", "OK", False),
            ("OK ", "OK", False),
            ("/rename a", "/rename a", True),
            (" /rename a", "/rename a", False),
            ("one\ntwo", "one\ntwo", True),
            ("one two", "one\ntwo", True),
            ("one  two", "one\ntwo", True),
            ("onetwo!", "one\ntwo", False),
            ("one two", "one two", True),
            ("one  two", "one two", False),
        ]
        body = (
            "$cases = '" + json.dumps(cases).replace("'", "''") + "' | ConvertFrom-Json\n"
            "$out = @(foreach ($c in $cases) { [bool](Test-SameText $c[0] $c[1]) })\n"
            "ConvertTo-Json -InputObject $out -Compress\n"
        )
        got = run_functions(["ConvertTo-PlainValue", "Test-SameText"], body)
        self.assertEqual(got, [c[2] for c in cases])

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_a_tab_label_is_cut_at_25_characters_with_an_ellipsis(self):
        body = (
            "$a = Get-TabLabel 'short'\n"
            "$b = Get-TabLabel ('x' * 25)\n"
            "$c = Get-TabLabel ('y' * 40)\n"
            "ConvertTo-Json -Compress -InputObject @($a, $b, $c.Length, [int]$c[$c.Length - 1], $c.Substring(0, 24))\n"
        )
        got = run_functions(["Get-TabLabel"], body)
        self.assertEqual(got, ["short", "x" * 25, 25, 0x2026, "y" * 24])

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_model_names_match_whole_words_at_the_start_of_an_option(self):
        cases = [
            ("Haiku 5.5 Fastest for quick answers", "Haiku 5.5", True),
            ("haiku 5.5", "Haiku 5.5", True),
            ("Haiku 4.5 Fast", "Haiku 5.5", False),
            ("Opus 5.5 Most capable", "Opus 5", False),
            ("Opus 5 Capable", "Opus 5", True),
            ("Fable 5.2", "Fable 5", False),
        ]
        body = (
            "$cases = '" + json.dumps(cases).replace("'", "''") + "' | ConvertFrom-Json\n"
            "$out = @(foreach ($c in $cases) { [bool](Test-ModelMatch $c[0] $c[1]) })\n"
            "ConvertTo-Json -InputObject $out -Compress\n"
        )
        got = run_functions(["Test-ModelMatch"], body)
        self.assertEqual(got, [c[2] for c in cases])


class ModelKeyTests(unittest.TestCase):
    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_the_key_is_written_back_byte_for_byte_and_only_its_line_changes(self):
        original = (
            '{\r\n  "permissions": {"allow": ["Bash(ls:*)"]},\r\n  "model": "fable",\r\n'
            '  "env": {"model": "not this one"},\r\n  "statusLine": {"type": "command"}\r\n}\r\n'
        ).encode("utf-8")
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "settings.json").write_bytes(original)
            body = (
                "$Utf8 = New-Object System.Text.UTF8Encoding($false)\n"
                "function Stop-Panel([int]$Code, [string]$Reason) { throw ('exit ' + $Code) }\n"
                "$k0 = Read-ModelLine\n"
                "$h0 = Get-SettingsHash\n"
                "Write-ModelValue 'haiku'\n"
                "$mid = (Read-ModelLine).Value\n"
                "$changed = Get-SettingsHash\n"
                "Write-ModelValue $k0.Value\n"
                "$h1 = Get-SettingsHash\n"
                "ConvertTo-Json -Compress -InputObject @($k0.Value, $mid, [bool]($h0 -ne $changed), [bool]($h0 -eq $h1))\n"
            )
            got = run_functions(["Get-ConfigDir", "Read-ModelLine", "Write-ModelValue", "Get-SettingsHash"], body, env={"CLAUDE_CONFIG_DIR": d})
            after = (Path(d) / "settings.json").read_bytes()
        self.assertEqual(got, ["fable", "haiku", True, True])
        self.assertEqual(after, original)
        self.assertEqual(hashlib.sha256(after).hexdigest(), hashlib.sha256(original).hexdigest())

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_two_model_lines_or_none_means_no_pick_is_made(self):
        for text in ('{\n  "model": "a",\n  "model": "b"\n}\n', '{\n  "theme": "dark"\n}\n'):
            with tempfile.TemporaryDirectory() as d:
                (Path(d) / "settings.json").write_text(text, encoding="utf-8")
                body = (
                    "$Utf8 = New-Object System.Text.UTF8Encoding($false)\n"
                    "function Stop-Panel([int]$Code, [string]$Reason) { throw ('exit ' + $Code) }\n"
                    "$r = 'none'\n"
                    "try { [void](Read-ModelLine) } catch { $r = $_.Exception.Message }\n"
                    "ConvertTo-Json -Compress -InputObject @($r)\n"
                )
                got = run_functions(["Get-ConfigDir", "Read-ModelLine"], body, env={"CLAUDE_CONFIG_DIR": d})
            self.assertEqual(got, ["exit 5"])

    def test_every_pick_saves_first_restores_right_after_and_exits_6_on_a_mismatch(self):
        pick = function_text("Select-Model")
        self.assertLess(pick.index("Read-ModelLine"), pick.index(".Expand()"), "saved before the picker opens")
        self.assertLess(pick.index("Get-SettingsHash"), pick.index(".Expand()"))
        self.assertLess(pick.index(".Invoke()"), pick.index("Write-ModelValue"))
        self.assertIn("finally", pick)
        self.assertIn("Stop-Panel 6", pick)
        self.assertIn("MODEL KEY NOT RESTORED", pick)
        self.assertIn("Fix ~\\.claude\\settings.json by hand", pick)
        self.assertEqual(len(re.findall(r"Select-Model \$", code_lines(source()))), 2, "called from new and from model, nowhere else")


class BridgeUseTests(unittest.TestCase):
    def test_the_driver_calls_only_bridge_actions_the_installed_0_1_0_has(self):
        text = code_lines(source())
        used = set(re.findall(r"Invoke-Bridge(?:Ok)?\s+'([\w-]+)'", text))
        self.assertTrue(used, "the driver must use the bridge")
        self.assertLessEqual(used, BRIDGE_ACTIONS & INSTALLED_ACTIONS)
        self.assertNotIn("group-pick", source(), "the driver never sends group-pick (0.1.0 still has it)")
        # The options it passes: only ones 0.1.0 reads.
        for m in re.finditer(r"Invoke-BridgeOk\s+'open'\s+(@\{[^}]*\}|\$\w+)", text):
            self.assertRegex(m.group(1), r"sessionId|\$bargs")
        self.assertIn("all = $false", text)

    def test_a_bridge_that_does_not_answer_ping_in_10_seconds_is_exit_2_with_install_steps(self):
        test = function_text("Test-Bridge")
        self.assertIn("Invoke-Bridge 'ping' $null 10", test)
        self.assertIn("Stop-Panel 2", test)
        for part in ["build.py", "code --install-extension", "do not reload"]:
            self.assertIn(part, test)

    def test_send_reads_the_input_for_a_draft_before_it_closes_the_tab_and_skips_the_close_when_there_is_none(self):
        send = function_text("Do-Send")
        self.assertIn("$s.HadTab = ($rs.Count -gt 0)", send)
        self.assertIn("if ($s.HadTab)", send)
        self.assertIn("Close-Checked $Id $Id 'empty' (Get-TabLabel $chat.Name) $Id", send, "his tab closes only empty, after the check")
        self.assertLess(send.index("Close-Checked"), send.index("Open-WithText"))
        self.assertLess(send.index("Wait-Ready"), send.index("Send-Now"))
        self.assertIn("Confirm-Sent", send)
        self.assertIn("Invoke-Guarded", send)

    def test_new_requires_a_model_and_never_sends_before_the_picker_shows_it(self):
        new = function_text("Do-New")
        self.assertLess(new.index("Stop-Panel 1 '-Model is needed"), new.index("Test-Bridge"))
        self.assertLess(new.index("Select-Model $ModelName"), new.index("Send-Now"))
        self.assertLess(new.index("Get-Pill"), new.index("Send-Now"))
        self.assertIn("groupId", new)

    def test_send_confirms_the_input_cleared_and_the_transcript_effect_before_exit_0(self):
        confirm = function_text("Confirm-Sent")
        for part in ["did not clear", "compact_boundary", "custom", "gained no user line", "Stop-Panel 1"]:
            self.assertIn(part, confirm if part != "custom" else confirm.replace("Title", "custom"))
        self.assertIn("-ceq $title", confirm)

    def test_close_untitled_closes_only_blank_tabs_with_no_transcript_and_no_row(self):
        text = function_text("Do-CloseUntitled")
        for part in ["Get-UntitledBlock", "Close-Checked '' '*' 'empty' $UntitledLabel '' $true", "-ceq $UntitledLabel"]:
            self.assertIn(part, text)
        block = function_text("Get-UntitledBlock")
        for part in ["Find-Transcript", "-notcontains [string]$_.sessionId", "status -ne 'idle'", "Get-RecordAgeSeconds $c) -lt 60", "$PanelEntrypoint"]:
            self.assertIn(part, block)


class LogTests(unittest.TestCase):
    def test_one_line_per_run_with_no_message_text(self):
        text = source()
        self.assertIn("claude-panel.log", text)
        log = function_text("Write-PanelLog")
        self.assertIn("exit={3} secs={4:N1}", log)
        self.assertNotRegex(log, r"\$\{?[Tt]ext\b")
        self.assertEqual(len(re.findall(r"AppendAllText", text)), 1)
        self.assertEqual(len(re.findall(r"Write-PanelLog ", text)), 1, "one call, in Invoke-Logged")
        call = re.search(r"Write-PanelLog \$ActionName .*", text).group(0)
        self.assertEqual(call.strip(), "Write-PanelLog $ActionName $LogId $logCode $logReason")
        logged = function_text("Invoke-Logged")
        self.assertLess(logged.index("Restore-Screen"), logged.index("Write-PanelLog"))
        self.assertIn("Get-PanelExit $_", logged)

    def test_no_reason_it_prints_contains_the_text(self):
        text = code_lines(source())
        for m in re.finditer(r"Stop-Panel \d+ [\"'].*", text):
            self.assertNotRegex(m.group(0), r"\$\{?[Tt]ext\b", m.group(0))

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_a_run_writes_one_line_with_time_action_session_exit_and_seconds(self):
        with tempfile.TemporaryDirectory() as d:
            body = (
                "$Utf8 = New-Object System.Text.UTF8Encoding($false)\n"
                f"$LogDir = '{d}'; $LogFile = Join-Path $LogDir 'claude-panel.log'\n"
                "$script:Notes = @('one note'); $script:Clock = [Diagnostics.Stopwatch]::StartNew()\n"
                "Write-PanelLog 'send' 'abc-123' 5 'a reason'\n"
                "Write-PanelLog 'list' '' 0 'ok'\n"
                "ConvertTo-Json -Compress -InputObject @([System.IO.File]::ReadAllLines($LogFile))\n"
            )
            got = run_functions(["Write-PanelLog"], body)
        self.assertEqual(len(got), 2)
        self.assertRegex(got[0], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d send abc-123 exit=5 secs=\d+\.\d a reason \[one note\]$")
        self.assertRegex(got[1], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d list - exit=0 secs=\d+\.\d \[one note\]$")


class SelfTestTests(unittest.TestCase):
    def test_selftest_runs_the_path_on_a_haiku_chat_and_prints_pass_or_fail_per_step(self):
        text = source()
        selftest = function_text("Do-SelfTest")
        self.assertIn("$SelfTestModel = 'Haiku 5.5'", text)
        self.assertIn("zz-selftest-", selftest)
        self.assertIn("'HHmm'", selftest)
        for step in ["'new (Haiku 5.5, two-line first prompt)'", "send /rename", "'send /compact'", "model set to", "'close the chat tab'"]:
            self.assertIn(step, selftest)
        self.assertIn("'PASS'", text)
        self.assertIn("'FAIL'", text)
        self.assertIn("session id: $id", selftest)

    def test_new_refuses_without_a_model_before_it_touches_anything(self):
        new = function_text("Do-New")
        self.assertLess(new.index("Stop-Panel 1 '-Model is needed"), new.index("Ensure-Screen"))

    def test_model_without_a_name_only_lists_the_options(self):
        picker = function_text("Select-Model")
        self.assertIn("options: {1}; active: {2}", picker)
        self.assertNotIn("-Model is needed", function_text("Do-Model"))


class BridgeExtensionTests(unittest.TestCase):
    def test_the_extension_is_ascii_and_has_only_the_fixed_actions(self):
        data = EXTENSION.read_bytes()
        self.assertEqual([i for i, b in enumerate(data) if b > 127], [])
        text = data.decode("ascii")
        block = re.search(r"const ACTIONS = \{(.*?)\n\};\n", text, re.DOTALL).group(1)
        names = set(re.findall(r"^  '?([\w-]+)'?: async function", block, re.MULTILINE))
        self.assertEqual(names, BRIDGE_ACTIONS)
        for gone in ["group-pick", "addSessionTabToGroup", "quickOpen", "acceptSelected"]:
            self.assertNotIn(gone, text)

    def test_the_source_is_0_2_0_and_close_tab_closes_one_tab_unless_all_is_asked_for(self):
        text = EXTENSION.read_text(encoding="ascii")
        self.assertIn("const VERSION = '0.2.0';", text)
        pkg = json.loads((BRIDGE_DIR / "package.json").read_text(encoding="utf-8"))
        self.assertEqual(pkg["version"], "0.2.0")
        close = re.search(r"'close-tab': async function.*?\n  \}", text, re.DOTALL).group(0)
        self.assertIn("if (!label) throw new Error('label is required');", close)
        self.assertIn("if (args.all !== true) matches = matches.slice(0, 1);", close)
        readme = (BRIDGE_DIR / "README.md").read_text(encoding="utf-8")
        for part in ["0.1.0", "group-pick", "inbox", "0.2.0", "no chat"]:
            self.assertIn(part, readme)

    def test_no_action_runs_an_arbitrary_command_id_or_code(self):
        text = EXTENSION.read_text(encoding="ascii")
        for m in re.finditer(r"executeCommand\(\s*([^,)]+)", text):
            self.assertIn(
                m.group(1).strip(),
                {"OPEN_COMMAND"},
                "every executeCommand names a fixed command id",
            )
        for banned in ["eval(", "new Function", "child_process", "spawn(", "exec(", "execSync", "require(args", "require(req"]:
            self.assertNotIn(banned, text)
        self.assertEqual(set(re.findall(r"require\('([^']+)'\)", text)), {"vscode", "fs", "path"})
        self.assertIn("isClaudeTab", text)
        self.assertEqual(text.count("tabGroups.close("), 1)

    def test_it_activates_only_in_the_vault_window_and_handles_stale_requests(self):
        text = EXTENSION.read_text(encoding="ascii")
        self.assertIn("path.join(root, 'system', 'af')", text)
        self.assertIn("path.join(root, 'AGENTS.md')", text)
        self.assertIn("MAX_AGE_MS = 2 * 60 * 1000", text)
        self.assertIn("renameSync", text, "results are written to a temp file and renamed")
        self.assertIn("bridge.log", text)
        log = re.search(r"function logLine.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertNotIn("prompt", log)

    def test_the_package_manifest_fits_vs_code_1_146(self):
        pkg = json.loads((BRIDGE_DIR / "package.json").read_text(encoding="utf-8"))
        self.assertEqual((pkg["publisher"], pkg["name"]), ("agentframe", "af-bridge"))
        self.assertEqual(pkg["activationEvents"], ["onStartupFinished"])
        self.assertTrue(pkg["main"].endswith("extension.js"))
        self.assertEqual(pkg["engines"]["vscode"], "^1.100.0")

    def test_build_py_writes_a_vsix_with_the_three_required_entries(self):
        with tempfile.TemporaryDirectory() as d:
            proc = subprocess.run([sys.executable, "-I", str(BUILD), "--out", d], capture_output=True, text=True, timeout=60)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            files = list(Path(d).glob("af-bridge-*.vsix"))
            self.assertEqual(len(files), 1)
            version = json.loads((BRIDGE_DIR / "package.json").read_text(encoding="utf-8"))["version"]
            self.assertEqual(files[0].name, f"af-bridge-{version}.vsix")
            with zipfile.ZipFile(files[0]) as z:
                self.assertIsNone(z.testzip())
                names = set(z.namelist())
                self.assertTrue({"[Content_Types].xml", "extension.vsixmanifest", "extension/package.json", "extension/extension.js"} <= names, names)
                manifest = z.read("extension.vsixmanifest").decode("utf-8")
                self.assertIn('Publisher="agentframe"', manifest)
                self.assertIn(f'Version="{version}"', manifest)
                self.assertIn("Microsoft.VisualStudio.Code.Engine", manifest)
                self.assertEqual(z.read("extension/extension.js"), EXTENSION.read_bytes())
        # The real package folder is not touched by the test.
        self.assertNotIn(str(ROOT / "system" / "logs"), proc.stdout)


class ClientTests(unittest.TestCase):
    """panel_bridge.py against a fake bridge: a thread that answers requests in a temp folder."""

    def load(self):
        spec = importlib.util.spec_from_file_location("panel_bridge_under_test", CLIENT)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod

    def run_client(self, answer, args):
        import os

        mod = self.load()
        with tempfile.TemporaryDirectory() as d:
            base = Path(d) / "system" / "logs" / "panel-bridge"
            stop = threading.Event()

            def bridge():
                req = base / "requests"
                res = base / "results"
                while not stop.is_set():
                    for f in sorted(req.glob("*.json")) if req.exists() else []:
                        r = json.loads(f.read_text(encoding="utf-8"))
                        f.unlink()
                        out = answer(r)
                        if out is not None:
                            (res / f.name).write_text(json.dumps(out), encoding="utf-8")
                    time.sleep(0.02)

            t = threading.Thread(target=bridge, daemon=True)
            t.start()
            old = os.environ.get("AF_ROOT")
            os.environ["AF_ROOT"] = d
            try:
                import contextlib
                import io

                buf = io.StringIO()
                with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(io.StringIO()):
                    code = mod.main(args)
            finally:
                stop.set()
                t.join(timeout=2)
                if old is None:
                    os.environ.pop("AF_ROOT", None)
                else:
                    os.environ["AF_ROOT"] = old
            left = list((base / "requests").glob("*")) + list((base / "results").glob("*"))
            return code, buf.getvalue(), left

    def test_exit_0_on_ok_and_the_request_carries_the_action_and_args(self):
        seen = {}

        def answer(r):
            seen.update(r)
            return {"id": r["id"], "ok": True, "action": r["action"], "result": {"closed": 1}}

        code, out, left = self.run_client(answer, ["close-tab", "--json", '{"label": "x", "all": false}', "--timeout", "5"])
        self.assertEqual(code, 0)
        self.assertEqual((seen["action"], seen["args"]), ("close-tab", {"label": "x", "all": False}))
        self.assertEqual(json.loads(out)["result"]["closed"], 1)
        self.assertEqual(left, [], "the request and the result are both gone")

    def test_exit_1_on_an_error_and_2_on_a_timeout(self):
        code, out, _ = self.run_client(lambda r: {"id": r["id"], "ok": False, "action": r["action"], "error": "nope"}, ["tabs", "--timeout", "5"])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(out)["error"], "nope")
        code, _, left = self.run_client(lambda r: None, ["ping", "--timeout", "0.5"])
        self.assertEqual(code, 2)
        self.assertEqual(left, [], "an unanswered request is withdrawn so it cannot run later")


class PairingTests(unittest.TestCase):
    def test_the_rust_side_names_this_script_and_the_override(self):
        rust = (ROOT / "system" / "af" / "src" / "panel.rs").read_text(encoding="utf-8")
        self.assertIn('"claude_panel.ps1"', rust)
        self.assertIn('"AF_PANEL_DRIVER"', rust)
        for flag in ["-Session", "-TextFile", "-Model", "-GroupId", "-WaitMinutes"]:
            self.assertIn(f'"{flag}"', rust)
            self.assertIn(flag.lstrip("-"), source())

    def test_the_hard_nudge_tells_a_build_chat_to_compact_itself_when_idle(self):
        rust = (ROOT / "system" / "af" / "src" / "context.rs").read_text(encoding="utf-8")
        self.assertIn("af session compact {name} --when-idle", rust)

    def test_every_exit_code_the_driver_uses_is_one_the_rust_side_explains(self):
        codes = {int(c) for c in re.findall(r"Stop-Panel (\d)", source())} | {int(c) for c in re.findall(r"Code = (\d);", function_text("Test-SafeToClose"))} - {0}
        rust = (ROOT / "system" / "af" / "src" / "panel.rs").read_text(encoding="utf-8")
        for c in codes - {1}:
            self.assertIn(f"Some({c})", rust, f"exit {c} has no sentence in panel.rs")
        self.assertEqual(codes, {1, 2, 3, 4, 5, 6})


# ---------------------------------------------------------------- the review fixes, unit by unit

# A stand-in for the driver's Add-Type block: the exit type, and the window and idle reads with values a
# test sets, so the driver's own checks run against a staged screen.
MOCK_NATIVE = (
    "Add-Type -TypeDefinition @'\n"
    "using System;\n"
    "public class PanelExit : Exception { public int Code; public string Reason; public PanelExit(int c, string r) : base(r) { Code = c; Reason = r; } }\n"
    "public static class PanelNative { public static bool Iconic; public static double Idle = 999999;\n"
    "  public static bool IsIconic(IntPtr h) { return Iconic; } public static double IdleMs() { return Idle; } }\n"
    "'@\n"
    "function Stop-Panel([int]$Code, [string]$Reason) { throw (New-Object PanelExit($Code, $Reason)) }\n"
    "$IdleSeconds = 120; $script:Win = $null; $script:Hwnd = [IntPtr]::Zero\n"
)


def dead_pid() -> int:
    p = subprocess.Popen([sys.executable, "-c", "pass"])
    p.wait()
    return p.pid


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class UnbrokenCloseCheckTests(unittest.TestCase):
    """Finding 1 and 6: the last thing before a tab close is one check with no wait in it."""

    STAGE = (
        MOCK_NATIVE
        + "$script:St = [pscustomobject]@{ Text = 'hello there' }\n"
        "$script:Tabs = @(); $script:Rows = @(); $script:Rec = $null; $script:Enabled = $false; $script:Value = ''; $script:Block = ''\n"
        "function Get-ClaudeTabs { $script:Tabs }\n"
        "function Get-Chats($w) { $script:Rows }\n"
        "function Get-RecordState([string]$Id) { $script:Rec }\n"
        "function Get-UntitledBlock { $script:Block }\n"
        "function Get-InputPair { [pscustomobject]@{ Input = 'in'; Send = [pscustomobject]@{ Current = [pscustomobject]@{ IsEnabled = $script:Enabled } } } }\n"
        "function Get-InputValue($El) { $script:Value }\n"
        "function Tab([string]$l, [bool]$a) { [pscustomobject]@{ claude = $true; label = $l; isActive = $a } }\n"
        "function Row([string]$i, [bool]$a, [string]$s) { [pscustomobject]@{ Id = $i; Active = $a; Status = $s } }\n"
        "function Rec([string]$b, [string]$t) { [pscustomobject]@{ Count = 1; Busy = $b; Terminal = $t } }\n"
        "function Reset {\n"
        "  [PanelNative]::Iconic = $false; [PanelNative]::Idle = 999999\n"
        "  $script:Tabs = @((Tab 'chat-a' $true), (Tab 'other' $false)); $script:Rows = @((Row 'id-a' $true 'idle'), (Row 'id-b' $false 'idle'))\n"
        "  $script:Rec = (Rec '' ''); $script:Enabled = $false; $script:Value = ''; $script:Block = ''\n"
        "}\n"
        "function Check([string]$RowId, [string]$RecId, [string]$Mode, [string]$Label, [bool]$First = $false) {\n"
        "  $c = Test-SafeToClose $RowId $RecId $Mode $Label $First\n"
        "  '{0}|{1}|{2}' -f $c.Code, $c.Retry, $c.Label\n"
        "}\n"
    )

    def run_cases(self, body: str):
        return run_functions(["Test-SafeToClose", "Test-SameText", "ConvertTo-PlainValue"], self.STAGE + body)

    def test_his_tab_closes_only_when_everything_holds_at_once(self):
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "Reset; $o.ok = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Enabled = $true; $o.draft = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; [PanelNative]::Iconic = $true; $o.minimized = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Rows = @((Row 'id-a' $true 'idle'), (Row 'id-b' $true 'idle')); $o.two_active = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Rows = @((Row 'id-a' $false 'idle'), (Row 'id-b' $true 'idle')); $o.other_active = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Rows = @((Row 'id-a' $true 'running')); $o.row_running = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Rows = @((Row 'id-a' $true 'none')); $o.row_no_status = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Rec = (Rec 'its session record (pid 1) says no status' ''); $o.rec_busy = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Rec = (Rec '' 'a terminal holds it'); $o.terminal = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Tabs = @((Tab 'chat-a' $true), (Tab 'chat-a' $false)); $o.two_labels = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Tabs = @((Tab 'chat-a' $false), (Tab 'other' $true)); $o.not_active_tab = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Tabs = @((Tab 'other' $true)); $o.no_tab = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; [PanelNative]::Idle = 5000; $o.operator = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got, {
            "ok": "0|False|chat-a",
            "draft": "4|False|",
            "minimized": "3|True|",
            "two_active": "5|True|",
            "other_active": "5|True|",
            "row_running": "3|True|",
            "row_no_status": "3|True|",
            "rec_busy": "3|True|",
            "terminal": "5|False|",
            "two_labels": "5|False|",
            "not_active_tab": "5|True|",
            "no_tab": "5|True|",
            "operator": "3|True|",
        })

    def test_the_drivers_own_tab_closes_only_with_its_exact_text_or_an_empty_input(self):
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "Reset; $script:Enabled = $true; $script:Value = 'hello there'; $o.own_text = Check 'id-a' 'id-a' 'text' ''\n"
            "Reset; $script:Enabled = $true; $script:Value = 'hello there, and his words'; $o.his_words = Check 'id-a' 'id-a' 'text' ''\n"
            "Reset; $script:Enabled = $false; $script:Value = ''; $o.empty = Check 'id-a' 'id-a' 'text' ''\n"
            "Reset; $script:Tabs = @((Tab 'chat-a' $true), (Tab 'x' $true)); $script:Enabled = $true; $script:Value = 'hello there'; $o.two_active_tabs = Check 'id-a' 'id-a' 'text' ''\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got, {"own_text": "0|False|chat-a", "his_words": "4|False|", "empty": "0|False|chat-a", "two_active_tabs": "5|True|"})

    def test_untitled_tabs_need_no_active_row_and_the_first_of_many_to_be_the_active_one(self):
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "Reset; $script:Rows = @((Row 'id-a' $false 'idle')); $script:Tabs = @((Tab 'Claude Code' $true), (Tab 'Claude Code' $false))\n"
            "$o.first = Check '' '*' 'empty' 'Claude Code' $true\n"
            "$o.not_first_of_many_allowed = Check '' '*' 'empty' 'Claude Code' $false\n"
            "$script:Block = 'an empty chat started less than 60 seconds ago'; $o.young = Check '' '*' 'empty' 'Claude Code' $true\n"
            "Reset; $script:Tabs = @((Tab 'Claude Code' $false), (Tab 'Claude Code' $true)); $script:Rows = @((Row 'id-a' $false 'idle'))\n"
            "$o.second_selected = Check '' '*' 'empty' 'Claude Code' $true\n"
            "Reset; $script:Tabs = @((Tab 'Claude Code' $true)); $o.row_active = Check '' '*' 'empty' 'Claude Code' $true\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got, {
            "first": "0|False|Claude Code",
            "not_first_of_many_allowed": "5|False|",
            "young": "5|False|",
            "second_selected": "5|True|",
            "row_active": "5|True|",
        })

    def test_nothing_waits_between_the_check_and_the_close(self):
        close = function_text("Close-Checked")
        check_at = close.index("Test-SafeToClose")
        close_at = close.index("Close-TabByLabel")
        self.assertLess(check_at, close_at)
        between = close[check_at:close_at]
        for waiting in ["Start-Sleep", "Wait-", "Show-Chat", "Invoke-Bridge", "Get-"]:
            self.assertNotIn(waiting, between, f"{waiting} between the check and the close")
        # The wait and the reveal come before the check, inside the loop, so a wait means the whole check again.
        loop = close[close.index("while ($true)"):check_at]
        self.assertIn("Wait-Idle", loop)
        self.assertIn("Show-Chat", loop)
        check = function_text("Test-SafeToClose")
        self.assertNotIn("Start-Sleep", check)
        self.assertNotIn("Wait-", check)
        for part in ["IsIconic", "Get-ClaudeTabs", "isActive", "Get-Chats", "Status -ne 'idle'", "Get-RecordState", "Get-InputPair", "IdleMs()"]:
            self.assertIn(part, check)
        self.assertLess(check.index("Get-InputPair"), check.index("IdleMs()"), "the operator's idle time is the last read")

    def test_every_close_goes_through_the_check_and_closes_one_tab(self):
        code = code_lines(source())
        self.assertEqual(len(re.findall(r"Invoke-BridgeOk 'close-tab'", code)), 1, "one call, in Close-TabByLabel")
        self.assertIn("@{ label = $Label; all = $false }", function_text("Close-TabByLabel"))
        calls = [m.start() for m in re.finditer(r"Close-TabByLabel \$", code)]
        self.assertEqual(len(calls), 1, "Close-TabByLabel is called from Close-Checked only")
        self.assertIn("Close-TabByLabel $c.Label", function_text("Close-Checked"))


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class RecordRuleTests(unittest.TestCase):
    """Finding 6: only 'status: idle' is idle, every live record counts, a terminal holder is refused."""

    def test_records_are_read_by_these_rules(self):
        import os

        me, dead = os.getpid(), dead_pid()
        recs = {
            "a": {"pid": me, "sessionId": "s1", "status": "idle", "entrypoint": "claude-vscode"},
            "b": {"pid": dead, "sessionId": "s1", "status": "busy", "entrypoint": "claude-vscode"},
            "c": {"pid": me, "sessionId": "s2", "entrypoint": "claude-vscode"},
            "d": {"pid": me, "sessionId": "s3", "status": "idle", "entrypoint": "cli"},
            "e": {"pid": me, "sessionId": "s4", "status": "idle", "entrypoint": "claude-vscode"},
            "f": {"pid": me, "sessionId": "s4", "status": "busy", "entrypoint": "claude-vscode"},
            "g": {"pid": dead, "sessionId": "s5", "status": "idle", "entrypoint": "claude-vscode"},
        }
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "sessions").mkdir()
            for k, v in recs.items():
                (Path(d) / "sessions" / f"{k}.json").write_text(json.dumps(v), encoding="utf-8")
            (Path(d) / "sessions" / "broken.json").write_text("{not json", encoding="utf-8")
            body = (
                "$Utf8 = New-Object System.Text.UTF8Encoding($false); $PanelEntrypoint = 'claude-vscode'\n"
                "$o = [ordered]@{}\n"
                "foreach ($s in @('s1', 's2', 's3', 's4', 's5')) { $r = Get-RecordState $s; $o[$s] = @($r.Count, $r.Busy, ($r.Terminal -ne '')) }\n"
                "ConvertTo-Json -Compress -Depth 4 -InputObject $o\n"
            )
            got = run_functions(["Get-ConfigDir", "Test-PidAlive", "Get-Records", "Get-ChatRecords", "Get-RecordState"], body, env={"CLAUDE_CONFIG_DIR": d})
        self.assertEqual(got["s1"], [1, "", False], "a dead process's busy record is left over and ignored")
        self.assertEqual(got["s2"][0:1], [1])
        self.assertIn("says no status", got["s2"][1], "no status is not idle")
        self.assertEqual(got["s3"][2], True, "a terminal holds s3")
        self.assertIn("says busy", got["s4"][1], "an idle record does not hide a busy one")
        self.assertEqual(got["s5"], [0, "", False])

    def test_the_guards_and_the_waits_use_these_rules(self):
        guards = function_text("Wait-Guards")
        self.assertIn("Get-RecordState", guards)
        self.assertIn("Stop-Panel 5 $rs.Terminal", guards)
        idle = function_text("Wait-ChatIdle")
        self.assertIn("$rs.Busy -eq ''", idle)
        self.assertIn("Status -eq 'idle'", idle)
        self.assertNotIn("-eq ''))", idle.replace("$rs.Busy -eq ''", ""), "an empty status is never idle")
        self.assertNotIn("function Get-Record(", source(), "no first-match record lookup is left")


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class LockTests(unittest.TestCase):
    """Finding 4: one driver run at a time; a lock left by a dead run is taken over."""

    def test_a_second_run_waits_then_exits_3_and_a_stale_lock_is_taken_over(self):
        with tempfile.TemporaryDirectory() as d:
            body = (
                "$Utf8 = New-Object System.Text.UTF8Encoding($false)\n"
                f"$BridgeBase = '{d}'; $LockFile = Join-Path $BridgeBase 'driver.lock'\n"
                "function Stop-Panel([int]$Code, [string]$Reason) { throw ('exit ' + $Code + ': ' + $Reason) }\n"
                "$script:Lock = $null; $script:WaitUntil = (Get-Date).AddSeconds(3)\n"
                "Enter-Lock 'send' 'sid-1'\n"
                "$holder = Read-LockHolder\n"
                "$first = $script:Lock; $script:Lock = $null\n"
                "$sw = [Diagnostics.Stopwatch]::StartNew(); $second = 'took it'\n"
                "try { Enter-Lock 'send' 'sid-2' } catch { $second = $_.Exception.Message }\n"
                "$waited = $sw.Elapsed.TotalSeconds\n"
                "$first.Dispose()\n"
                "$leftOver = Test-Path $LockFile\n"
                "$script:WaitUntil = (Get-Date).AddSeconds(3)\n"
                "Enter-Lock 'new' ''\n"
                "$holder2 = Read-LockHolder\n"
                "Exit-Lock\n"
                "ConvertTo-Json -Compress -InputObject @($holder, $second, [bool]($waited -ge 2), $leftOver, $holder2, (Test-Path $LockFile))\n"
            )
            got = run_functions(["Enter-Lock", "Exit-Lock", "Read-LockHolder"], body)
        holder, second, waited, left_over, holder2, after = got
        self.assertRegex(holder, r"^pid \d+, send, session sid-1, since ")
        self.assertTrue(second.startswith("exit 3: another driver run held the panel for the whole wait (pid "), second)
        self.assertTrue(waited)
        self.assertTrue(left_over, "the file of a run that ended without letting go stays behind")
        self.assertRegex(holder2, r"^pid \d+, new, session -, since ", "and the next run takes it over at once")
        self.assertFalse(after, "a run that lets go removes the file")

    def test_every_run_that_changes_anything_holds_the_lock(self):
        text = source()
        for action in ["send", "new", "model", "close-untitled", "selftest"]:
            self.assertRegex(text, rf"Invoke-Logged '{action}' \S+ \$true", action)
        self.assertIn("Invoke-Logged 'list' '' $false", text)
        self.assertIn("Enter-Lock 'list'", function_text("Do-List"), "a list that restores a minimized window waits its turn")
        logged = function_text("Invoke-Logged")
        self.assertLess(logged.index("Enter-Lock"), logged.index("& $Body"))
        self.assertLess(logged.index("Restore-Screen"), logged.index("Exit-Lock"), "the view is put back before the lock goes")
        self.assertIn("finally", logged)
        self.assertIn("system\\logs\\panel-bridge", text)
        self.assertIn("$LockFile = Join-Path $BridgeBase 'driver.lock'", text)

    def test_new_identifies_its_chat_by_the_record_its_own_open_created(self):
        new = function_text("Do-New")
        self.assertLess(new.index("$recBefore"), new.index("Invoke-BridgeOk 'open'"))
        self.assertIn("$recBefore -notcontains", new)
        self.assertIn("$new.Count -ne 1", new)
        self.assertIn("$_.Id -eq $s.Id", new, "the new row must be the new record's own")
        self.assertNotIn("$rowsBefore", new)
        self.assertLess(new.index("untitled"), new.index("Invoke-BridgeOk 'open'"), "no other untitled tab may be open")


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class ChangeReportTests(unittest.TestCase):
    """Finding 2: every exit says what changed and where the chat's tab is now."""

    def state(self, fields: dict) -> str:
        body = (
            "New-RunState 'id' 'text'\n"
            + "".join(f"$script:St.{k} = ${str(v).lower()}\n" for k, v in fields.items())
            + "ConvertTo-Json -Compress -InputObject @(Get-StateText)\n"
        )
        return run_functions(["New-RunState", "Test-Touched", "Get-StateText"], body)[0]

    def test_the_state_sentence_follows_what_changed(self):
        self.assertEqual(self.state({}), "The panel was not changed: the chat's tab is as it was.")
        t = self.state({"HadTab": True, "Closed": True, "TabOpen": False})
        self.assertIn("closed the chat's tab", t)
        self.assertIn("The chat's tab is now closed. It was open before the run.", t)
        t = self.state({"HadTab": True, "Closed": True, "OpenCalled": True, "Opened": True, "Prefilled": True, "Cleaned": True, "Reopened": True, "TabOpen": True})
        self.assertIn("opened it with the text in the input, then closed that tab again to take the unsent text away, then opened the chat's tab again", t)
        self.assertIn("The chat's tab is now open.", t)
        t = self.state({"OpenCalled": True, "Opened": True, "Prefilled": True, "LeftText": True, "TabOpen": True})
        self.assertIn("OPEN and still holds the driver's unsent text", t)

    def guarded(self, sent: bool, code: int) -> list:
        body = (
            MOCK_NATIVE
            + "$script:Restored = $false\n"
            "function Restore-Chat { $script:Restored = $true; $script:St.Cleaned = $true; $script:St.TabOpen = $false }\n"
            "New-RunState 'id' 'text'\n"
            "$script:St.Closed = $true; $script:St.HadTab = $true; $script:St.OpenCalled = $true; $script:St.Opened = $true; $script:St.Prefilled = $true\n"
            f"$script:St.Sent = ${str(sent).lower()}\n"
            "$out = $null\n"
            f"try {{ Invoke-Guarded {{ Stop-Panel {code} 'read-back failed' }} }} catch {{ $pe = Get-PanelExit $_; $out = @($pe.Code, $pe.Reason, $script:Restored) }}\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        return run_functions(["New-RunState", "Test-Touched", "Get-StateText", "Get-PanelExit", "Invoke-Guarded"], body)

    def test_a_failure_before_send_puts_the_chat_back_and_says_nothing_was_sent_and_where_the_tab_is(self):
        code, reason, restored = self.guarded(False, 5)
        self.assertEqual(code, 5)
        self.assertTrue(restored)
        self.assertIn("read-back failed. Nothing was sent. The driver closed the chat's tab", reason)
        self.assertIn("The chat's tab is now closed.", reason)

    def test_after_send_nothing_is_put_back_and_the_exit_is_never_a_nothing_was_sent_code(self):
        code, reason, restored = self.guarded(True, 5)
        self.assertEqual(code, 1)
        self.assertFalse(restored)
        self.assertIn("Send was pressed, so the text may have reached the chat", reason)

    def test_a_confirmed_send_is_exit_0_even_when_a_later_cleanup_fails(self):
        send = function_text("Do-Send")
        tail = send[send.index("# The text is sent and confirmed"):]
        self.assertIn("try {", tail)
        self.assertIn("Add-Note ('the text was sent; its tab was left open: '", tail)
        self.assertNotIn("Stop-Panel", tail)

    def test_notes_are_printed_on_every_exit(self):
        body = (
            "$script:Notes = @('one', 'two')\n"
            "$c = Out-Result ([pscustomobject]@{ Code = 5; Reason = 'why'; Value = $null })\n"
        )
        proc = run_powershell(
            "$t = $null; $e = $null\n"
            f"$ast = [System.Management.Automation.Language.Parser]::ParseFile('{DRIVER}', [ref]$t, [ref]$e)\n"
            "$d = @($ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true) | Where-Object { $_.Name -eq 'Out-Result' })\n"
            ". ([scriptblock]::Create($d[0].Extent.Text))\n" + body
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stderr.strip().splitlines(), ["why", "note: one", "note: two"])

    def test_a_restore_closes_only_its_own_tab_and_reopens_his_only_when_no_process_holds_the_chat(self):
        restore = function_text("Restore-Chat")
        self.assertIn("if ($null -eq $s -or $s.Sent", restore, "nothing is put back after Send")
        self.assertIn("if ($s.Opened -and $s.TabOpen)", restore, "only a tab the driver opened is closed")
        self.assertIn("$mode = 'text'", restore)
        self.assertIn("Close-Checked $s.RowId $s.Id $mode $s.OwnLabel $reveal", restore)
        self.assertLess(restore.index("Get-ChatRecords $s.Id).Count -gt 0"), restore.index("'reveal'"))
        self.assertIn("Start-Restoring", restore)
        opened = function_text("Open-WithText")
        self.assertIn("$n0 + 1", opened, "the tab set is compared before and after the open")
        self.assertLess(opened.index("Get-ChatRecords $Id"), opened.index("Invoke-BridgeOk 'open'"))
        self.assertLess(opened.index("$s.OpenCalled = $true"), opened.index("$s.Opened = $true"))


class SendWindowTests(unittest.TestCase):
    """Finding 3: Send is the very button the read-back verified; nothing is searched in between."""

    def test_no_search_between_the_last_check_and_the_invoke(self):
        send = function_text("Send-Now")
        last = send.rindex("Test-Ready")
        invoke = send.index(".Invoke()")
        between = send[last:invoke]
        for search in ["Get-Chats", "FindAll", "Find-Named", "Get-InputPair", "Get-ActiveIds", "Get-ClaudeTabs", "Invoke-Bridge"]:
            self.assertNotIn(search, between, f"{search} between the check and the Invoke")
        for reread in ["Get-InputValue $ready.Pair.Input", "$ready.Pair.Send.Current.IsEnabled", "Test-RowActive $ready.Row", "IdleMs()"]:
            self.assertIn(reread, between)
        self.assertIn("$ready.Pair.Send.GetCurrentPattern", send, "the verified Send element itself is invoked")

    def test_the_input_and_send_are_found_together_in_one_chat_document(self):
        pair = function_text("Get-InputPair")
        self.assertIn("$CT::Document", pair)
        self.assertIn("Find-Named $p 'Send message'", pair, "Send is searched inside the input's own document")
        ready = function_text("Test-Ready")
        self.assertIn("Pair = $pair; Row = $row", ready)


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class ModelMatchTests(unittest.TestCase):
    """Finding 5: a pick is checked on a word boundary, in the options and in the picker's value."""

    NAMES = ["Default", "Opus 5.5", "Fable 5.2", "Sonnet 5.5", "Haiku 5.5", "Fable 5.1", "Haiku 4.5", "Sonnet 5", "Opus 5", "Fable 5",
             "Opus 4.8", "Opus 4.7", "Opus 4.6", "Sonnet 4.6"]

    def test_each_real_name_matches_exactly_its_own_option_and_pill(self):
        options = [n + (" (recommended) Opus 5.5 for most work" if n == "Default" else " Some description") for n in self.NAMES]
        pills = [n + " Extra high" for n in self.NAMES]
        body = (
            "$names = '" + json.dumps(self.NAMES) + "' | ConvertFrom-Json\n"
            "$options = '" + json.dumps(options).replace("'", "''") + "' | ConvertFrom-Json\n"
            "$pills = '" + json.dumps(pills) + "' | ConvertFrom-Json\n"
            "$out = @(foreach ($n in $names) { [string]@($options | Where-Object { Test-ModelMatch $_ $n }).Count + '#' + (@($pills | Where-Object { Test-ModelMatch $_ $n }) -join '|') })\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        got = run_functions(["Test-ModelMatch"], body)
        self.assertEqual(len(got), len(self.NAMES))
        for name, row in zip(self.NAMES, got):
            count, pill = row.split("#", 1)
            self.assertEqual(count, "1", f"{name}: options matched {count}")
            self.assertEqual(pill, name + " Extra high", name)

    def test_a_failed_pick_does_not_pass_as_the_asked_model(self):
        cases = [("Opus 5.5 Extra high", "Opus 5", False), ("Fable 5.2 High", "Fable 5", False), ("Opus 5 High", "Opus 5", True),
                 ("Sonnet 4.6", "Sonnet 4.6", True), ("", "Opus 5", False), ("Haiku 5.5", "", False)]
        body = (
            "$cases = '" + json.dumps(cases) + "' | ConvertFrom-Json\n"
            "$out = @(foreach ($c in $cases) { [bool](Test-ModelMatch $c[0] $c[1]) })\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        self.assertEqual(run_functions(["Test-ModelMatch"], body), [c[2] for c in cases])

    def test_no_prefix_match_is_left_anywhere(self):
        code = code_lines(source())
        self.assertNotRegex(code, r"StartsWith\(\$(Name|ModelName)\s*,", "every model check goes through Test-ModelMatch")
        self.assertIn("Test-ModelMatch $pill $Name", function_text("Select-Model"))
        self.assertIn("Test-ModelMatch $nowPill $ModelName", function_text("Do-New"))


class DeadlineTests(unittest.TestCase):
    """Finding 7: one deadline bounds every step; af's timeout is that deadline plus a margin."""

    def test_the_work_time_is_the_same_number_on_both_sides(self):
        rust = (ROOT / "system" / "af" / "src" / "panel.rs").read_text(encoding="utf-8")
        ps = int(re.search(r"^\$WorkSeconds = (\d+)$", source(), re.MULTILINE).group(1))
        rs = int(re.search(r"pub const DRIVER_WORK_SECS: u64 = (\d+);", rust).group(1))
        self.assertEqual(ps, rs)
        wait_ps = float(re.search(r"\[double\]\$WaitMinutes = (\d+)", source()).group(1))
        wait_rs = float(re.search(r"pub const DRIVER_WAIT_MINUTES: f64 = ([\d.]+);", rust).group(1))
        self.assertEqual(wait_ps, wait_rs)
        restore = int(re.search(r"^\$RestoreSeconds = (\d+)$", source(), re.MULTILINE).group(1))
        self.assertGreater(ps - restore, 600 + 90 + 25, "the longest steps fit before the restore time")

    def test_every_step_wait_is_cut_to_the_deadline(self):
        for name in ["Wait-Ready", "Show-Chat", "Close-Checked", "Open-WithText", "Confirm-Sent", "Wait-ChatIdle"]:
            self.assertIn("Get-StepMs", function_text(name), name)
        self.assertIn("Get-WaitUntil", function_text("Wait-Idle"))
        self.assertIn("$script:RestoreUntil", function_text("Restore-Screen"))
        self.assertIn("finally", function_text("Invoke-Logged"))


class LogPrivacyTests(unittest.TestCase):
    """Finding 9: no reason or note carries a chat title, a tab label or the text."""

    def test_reasons_and_notes_name_no_title_label_or_text(self):
        code = code_lines(source())
        for m in re.finditer(r"(Stop-Panel \d+|Add-Note) \(?[\"'].*", code):
            line = m.group(0)
            for private in [r"\$title", r"\$Title", r"\$label", r"\$Label", r"\$chat\.Name", r"\.Current\.Name", r"\$Text\b", r"\$text\b"]:
                self.assertNotRegex(line, private, line)

    def test_a_failed_selftest_is_logged_with_its_own_exit_code(self):
        logged = function_text("Invoke-Logged")
        self.assertIn("$ActionName -eq 'selftest'", logged)
        self.assertIn("$logCode = [int]$r.Value", logged)
        self.assertIn("Write-PanelLog $ActionName $LogId $logCode $logReason", logged)


class WaiterPairingTests(unittest.TestCase):
    def test_the_context_hook_reads_the_waiter_result_before_the_nudge_can_rearm(self):
        hooks = (ROOT / "system" / "af" / "src" / "hooks.rs").read_text(encoding="utf-8")
        self.assertLess(hooks.index("take_waiter_result"), hooks.index("crate::context::nudge("))
        self.assertIn("stop_with_waiter_result", hooks)


if __name__ == "__main__":
    unittest.main()
