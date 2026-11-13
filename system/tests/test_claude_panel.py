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
# The fixed actions of the bridge (version 0.1.0 is installed; the driver may use only these).
BRIDGE_ACTIONS = {"ping", "tabs", "open", "reveal", "close-tab", "group-pick"}
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
        self.assertLessEqual(used, BRIDGE_ACTIONS)
        self.assertNotIn("group-pick", used, "built but not run in this build")
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

    def test_tab_identity_is_proved_before_close_tab_and_a_busy_chat_is_never_closed(self):
        close = function_text("Close-ChatTab")
        self.assertLess(close.index("'reveal'"), close.index("'tabs'") if "'tabs'" in close else close.index("Get-BridgeTabs"))
        self.assertLess(close.index("Get-BridgeTabs"), close.index("'close-tab'"))
        self.assertIn("$same.Count -ne 1", close)
        self.assertIn("isActive", close)
        self.assertIn("Get-ActiveIds", close)
        self.assertLess(close.index("Get-RecordBusy"), close.index("'close-tab'"))
        self.assertIn("Stop-Panel 3", close)
        self.assertEqual(len(re.findall(r"'close-tab'", code_lines(source()))), 2, "one in Close-ChatTab, one in close-untitled")

    def test_send_reads_the_input_for_a_draft_before_it_closes_the_tab_and_skips_the_close_when_there_is_none(self):
        send = function_text("Do-Send")
        self.assertIn("$hadTab = ($null -ne $rec)", send)
        self.assertIn("if ($hadTab)", send)
        self.assertLess(send.index("Stop-Panel 4"), send.index("Close-ChatTab"))
        self.assertLess(send.index("Close-ChatTab"), send.index("'open'"))
        self.assertLess(send.index("Wait-Ready"), send.index("Send-Now"))
        self.assertIn("Confirm-Sent", send)

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
        for part in ["Find-Transcript", "-notcontains $_.sessionId", "Get-SendState) -ne 'disabled'", "unsent draft", "-ceq 'Claude Code'"]:
            self.assertIn(part, text)


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
        self.assertEqual(call.strip(), "Write-PanelLog $ActionName $LogId $r.Code $r.Reason")
        logged = function_text("Invoke-Logged")
        self.assertLess(logged.index("Restore-Screen"), logged.index("Write-PanelLog"))
        self.assertIn("catch [PanelExit]", logged)

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

    def test_no_action_runs_an_arbitrary_command_id_or_code(self):
        text = EXTENSION.read_text(encoding="ascii")
        for m in re.finditer(r"executeCommand\(\s*([^,)]+)", text):
            self.assertIn(
                m.group(1).strip(),
                {"OPEN_COMMAND", "GROUP_COMMAND", "'workbench.action.quickOpenSelectNext'", "'workbench.action.acceptSelectedQuickOpenItem'"},
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
        codes = {int(c) for c in re.findall(r"Stop-Panel (\d)", source())}
        rust = (ROOT / "system" / "af" / "src" / "panel.rs").read_text(encoding="utf-8")
        for c in codes - {1}:
            self.assertIn(f"Some({c})", rust, f"exit {c} has no sentence in panel.rs")
        self.assertEqual(codes, {1, 2, 3, 4, 5, 6})


if __name__ == "__main__":
    unittest.main()
