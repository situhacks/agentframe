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
# What the driver may import from Windows: the idle time, the window's state, title and place in the stack, the
# pointer's position (read, and set by 'archive' alone), the window under a point, which window has the foreground
# (read only) and the shell's full-screen state. Nothing that sends input or takes the foreground.
# GetProcessTimes reads only this process's own start time, for the lock.
ALLOWED_IMPORTS = {
    "GetLastInputInfo", "IsIconic", "IsWindow", "GetWindowTextW", "GetProcessTimes",
    "GetCursorPos", "SetCursorPos", "WindowFromPoint", "GetAncestor", "SetWindowPos", "ShowWindow",
    "GetForegroundWindow", "SHQueryUserNotificationState",
    "GetWindowLongW", "GetWindowPlacement", "SetWindowPlacement", "GetWindow",
}
# Words that would mean synthetic input, clipboard use or taking the foreground. SetWindowVisualState is the
# Window pattern's restore: it takes the foreground and resets the idle time (measured), so it is never used.
FORBIDDEN = [
    "SendInput", "keybd_event", "mouse_event", "SendKeys", "clipboard", "SetForegroundWindow",
    "AttachThreadInput", "BringWindowToTop", "MOUSEINPUT", "KEYBDINPUT", "GetGUIThreadInfo", "SetWindowVisualState",
    "WindowPattern", "SwitchToThisWindow", "AllowSetForegroundWindow", "LockSetForegroundWindow",
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
        for word in ["mouse", "click", "keystroke", "typebatch", "unicode key"]:
            self.assertNotIn(word, code, f"the driver code must not mention {word!r}")
        # The pointer is read and set by the archive helpers only, through the two calls and nothing else.
        self.assertEqual(sorted(set(re.findall(r"\w*cursor\w*", code))), ["getcursorpos", "setcursorpos"])

    def test_it_imports_only_the_idle_time_and_the_window_state_from_windows(self):
        imports = set(re.findall(r"DllImport\(\"[^\"]+\"[^)]*\)\]\s*(?:public\s+)?static extern \w+ (\w+)\(", csharp_block(source())))
        self.assertEqual(imports, ALLOWED_IMPORTS)

    def test_it_runs_nothing_else_and_never_sets_the_input_text_through_a_pattern(self):
        text = code_lines(source())
        for banned in ["Invoke-Expression", "iex ", "Start-Process", "Stop-Process", "Invoke-WebRequest", "SetValue(", "Set-Clipboard", "SendWait"]:
            self.assertNotIn(banned, text, f"{banned!r} is not for this driver")

    def test_the_actions_are_the_seven_and_no_more(self):
        text = source()
        actions = re.findall(r"^    '([\w-]+)'\s*\{", text, re.MULTILINE)
        self.assertEqual(actions, ["list", "send", "new", "model", "archive", "close-untitled", "selftest"])
        params = re.search(r"param\((.*?)\n\)", text, re.DOTALL).group(1)
        self.assertEqual(re.findall(r"\$(\w+)", params), ["Action", "Session", "TextFile", "Model", "GroupId", "IdleSeconds", "WaitMinutes", "CancelFile"])
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
        self.assertEqual(used, {"InvokePattern", "ExpandCollapsePattern", "SelectionItemPattern", "ScrollItemPattern", "ValuePattern"})
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

    def test_a_minimized_window_is_restored_behind_his_window_without_the_foreground_and_minimized_again(self):
        ensure = function_text("Ensure-Screen")
        self.assertIn("IsIconic", ensure)
        self.assertIn("Restore-Minimized", ensure)
        restore_fn = function_text("Restore-Minimized")
        # Under Wait-Idle, then: bottom of the stack while minimized, shown without activation, behind his window.
        for part in ["Wait-Idle 'restore the minimized VS Code window'", "GetPlacement", "PlaceBottom", "ShowNoActivate", "Set-VsCodeBehind $front", "Foreground()", "Add-Note", "$script:WasMinimized = $true"]:
            self.assertIn(part, restore_fn)
        self.assertLess(restore_fn.index("Wait-Idle"), restore_fn.index("GetPlacement"), "the placement is saved before the restore")
        self.assertLess(restore_fn.index("GetPlacement"), restore_fn.index("PlaceBottom"))
        self.assertLess(restore_fn.index("PlaceBottom"), restore_fn.index("ShowNoActivate"))
        self.assertLess(restore_fn.index("ShowNoActivate"), restore_fn.index("Set-VsCodeBehind"))
        self.assertLess(restore_fn.index("Set-VsCodeBehind"), restore_fn.index("Wait-Tree"))
        behind = function_text("Set-VsCodeBehind")
        for part in ["PlaceBehind", "IsWindow", "IsTopmost"]:
            self.assertIn(part, behind)
        self.assertLess(behind.index("IsTopmost"), behind.index("PlaceBehind"), "never behind an always-on-top window")
        final = function_text("Confirm-VsCodeNotTopmost")
        for part in ["IsTopmost", "ClearTopmost", "Add-Note"]:
            self.assertIn(part, final)
        self.assertIn("Confirm-VsCodeNotTopmost", restore_fn)
        self.assertIn("Confirm-VsCodeNotTopmost", function_text("Undo-Raise"))
        restore = function_text("Restore-Screen")
        self.assertIn("MinimizeWithPlacement", restore)
        self.assertIn("MinimizeNoActivate", restore)
        self.assertIn("InvokePattern", restore, "his previous chat is put back by Invoke on its row")
        code = code_lines(source())
        self.assertNotIn("Set-WindowState", code)
        csharp = csharp_block(source())
        # Measured: a placement with showCmd 2 (SW_SHOWMINIMIZED) led to a foreground change; showCmd is always 7.
        self.assertIn("p.showCmd = 7", csharp)
        self.assertNotRegex(csharp, r"showCmd\s*=\s*2")
        self.assertNotRegex(csharp, r"ShowWindow\(h, 2\)")
        self.assertEqual(csharp.count("showCmd ="), 1, "the one place a placement is built")
        for stale in ["restores to the normal size", "goes to the normal size", "back to the normal size"]:
            self.assertNotIn(stale, source(), "SW_SHOWNOACTIVATE restores a window that was maximized to maximized (measured)")
        self.assertIn("ShowWindow(h, 4)", csharp, "SW_SHOWNOACTIVATE")
        self.assertIn("ShowWindow(h, 7)", csharp, "SW_SHOWMINNOACTIVE")
        self.assertIn("new IntPtr(1)", csharp, "HWND_BOTTOM")
        self.assertIn("SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE", csharp)
        # No restore anywhere goes through anything but Restore-Minimized.
        self.assertEqual(len(re.findall(r"Restore-Minimized", code)), 3, "defined once, called by Ensure-Screen and Close-Checked")

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
            got = run_functions(["Stop-Panel", "Read-Text"], body.replace("\\", "\\"))
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
            got = run_functions(["Write-PanelLog", "Add-LogLine"], body)
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
        self.assertEqual(codes, {1, 2, 3, 4, 5, 6, 7})


# ---------------------------------------------------------------- the review fixes, unit by unit

# A stand-in for the driver's Add-Type block: the exit type, and the window and idle reads with values a
# test sets, so the driver's own checks run against a staged screen.
MOCK_NATIVE = (
    "Add-Type -TypeDefinition @'\n"
    "using System;\n"
    "public class PanelExit : Exception { public int Code; public string Reason; public PanelExit(int c, string r) : base(r) { Code = c; Reason = r; } }\n"
    "public static class PanelNative { public static bool Iconic; public static double Idle = 999999;\n"
    "  public static bool IsIconic(IntPtr h) { return Iconic; } public static double IdleMs() { return Idle; } public static long OwnStart() { return 42; } }\n"
    "'@\n"
    "function Stop-Panel([int]$Code, [string]$Reason) { throw (New-Object PanelExit($Code, $Reason)) }\n"
    "$IdleSeconds = 120; $script:Win = $null; $script:Hwnd = [IntPtr]::Zero\n"
)


# A staged panel for the behaviour tests: one chat 'id-a' whose tab, process, input and Send the mocks
# below keep in $W, and the bridge and the screen as functions that read and change it. The driver's
# own functions run against it unchanged.
WORLD = (
    MOCK_NATIVE
    + "$UntitledLabel = 'Claude Code'; $PanelEntrypoint = 'claude-vscode'\n"
    "$script:Win = 'win'; $script:Notes = @(); $script:Restoring = $false; $script:St = $null\n"
    "$script:SettleSecs = 5; $script:RestoreSecs = 0\n"
    "$script:WaitUntil = (Get-Date).AddSeconds(-1); $script:StepUntil = (Get-Date).AddSeconds(-1)\n"
    "$script:HardUntil = (Get-Date).AddSeconds(30); $script:RestoreUntil = $script:HardUntil\n"
    "function Add-Note([string]$Note) { $script:Notes += $Note }\n"
    "$W = [pscustomobject]@{ TabOpen = $true; Pid = 101; ProcUntil = $null; Value = ''; Enabled = $false; Linger = 0.0; RevealWorks = $true; Log = @(); Waits = @() }\n"
    "function Write-Waiting([string]$Reason) { $W.Waits += $Reason }\nfunction Clear-Waiting { }\n"
    "function Get-ClaudeTabs { $t = @([pscustomobject]@{ claude = $true; label = 'other'; isActive = (-not $W.TabOpen) }); if ($W.TabOpen) { $t += [pscustomobject]@{ claude = $true; label = 'chat-a'; isActive = $true } }; $t }\n"
    "function Get-Chats($Window) { @([pscustomobject]@{ Id = 'id-a'; Active = $W.TabOpen; Status = 'idle' }, [pscustomobject]@{ Id = 'id-b'; Active = (-not $W.TabOpen); Status = 'idle' }) }\n"
    "function Get-ActiveIds { @(Get-Chats $null | Where-Object { $_.Active } | ForEach-Object { $_.Id }) }\n"
    "function Get-ChatRecords([string]$Id) { if ($Id -eq 'id-a' -and ($W.TabOpen -or ($null -ne $W.ProcUntil -and (Get-Date) -lt $W.ProcUntil))) { @([pscustomobject]@{ pid = $W.Pid; sessionId = 'id-a'; status = 'idle'; entrypoint = 'claude-vscode' }) } else { @() } }\n"
    "function Get-RecordState([string]$Id) { $r = @(Get-ChatRecords $Id); [pscustomobject]@{ Count = $r.Count; Busy = ''; Terminal = '' } }\n"
    "function Get-InputPair { if (-not $W.TabOpen) { return $null }; [pscustomobject]@{ Input = 'in'; Send = [pscustomobject]@{ Current = [pscustomobject]@{ IsEnabled = $W.Enabled } } } }\n"
    "function Get-InputValue($El) { $W.Value }\n"
    "function Show-Chat([string]$Id) { $W.Log += 'show' }\n"
    "function Close-TabByLabel([string]$Label, [int]$Count) { $W.Log += ('close ' + $Label); $W.TabOpen = $false; $W.ProcUntil = (Get-Date).AddSeconds($W.Linger) }\n"
    "function Invoke-BridgeOk([string]$Name, $BArgs) { $W.Log += $Name; if ($Name -eq 'reveal' -and $W.RevealWorks) { $W.TabOpen = $true; $W.Pid = $W.Pid + 1; $W.Value = ''; $W.Enabled = $false }; [pscustomobject]@{} }\n"
)
WORLD_FUNCTIONS = [
    "New-RunState", "Test-Touched", "Get-StateText", "Read-ChatState", "Read-ChatInput", "Get-InputKind", "Test-EmptyValue",
    "Get-Placeholders", "Test-SameText", "ConvertTo-PlainValue", "Test-SafeToClose", "Close-Checked", "Wait-ProcessEnd",
    "Restore-Chat", "Get-TabState", "Test-ProcessLingers", "Get-RevealCommand", "Get-InputRead", "Get-InputWhy", "Start-Restoring", "Get-WaitUntil", "Get-StepMs", "Wait-Idle", "Get-PanelExit", "Invoke-Guarded",
    "Test-CancelRequested", "Stop-IfCancelled",
]


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
        "$script:Recs = @(); function Get-ChatRecords([string]$Id) { $script:Recs }\n"
        "function Tab([string]$l, [bool]$a) { [pscustomobject]@{ claude = $true; label = $l; isActive = $a } }\n"
        "function Row([string]$i, [bool]$a, [string]$s) { [pscustomobject]@{ Id = $i; Active = $a; Status = $s } }\n"
        "function Rec([string]$b, [string]$t) { [pscustomobject]@{ Count = 1; Busy = $b; Terminal = $t } }\n"
        "function Reset {\n"
        "  [PanelNative]::Iconic = $false; [PanelNative]::Idle = 999999\n"
        "  $script:Tabs = @((Tab 'chat-a' $true), (Tab 'other' $false)); $script:Rows = @((Row 'id-a' $true 'idle'), (Row 'id-b' $false 'idle'))\n"
        "  $script:Rec = (Rec '' ''); $script:Enabled = $false; $script:Value = ''; $script:Block = ''; $script:Recs = @()\n"
        "}\n"
        "function Check([string]$RowId, [string]$RecId, [string]$Mode, [string]$Label, [bool]$First = $false, [int]$OwnPid = 0) {\n"
        "  $c = Test-SafeToClose $RowId $RecId $Mode $Label $First $OwnPid\n"
        "  '{0}|{1}|{2}' -f $c.Code, $c.Retry, $c.Label\n"
        "}\n"
    )

    def run_cases(self, body: str):
        return run_functions(["Test-SafeToClose", "Get-InputKind", "Get-InputRead", "Get-InputWhy", "Test-EmptyValue", "Get-Placeholders", "Test-SameText", "ConvertTo-PlainValue"], self.STAGE + body)

    def test_an_unknown_placeholder_with_send_disabled_is_named_as_such_with_its_length_never_its_text(self):
        """Third review, item 2: a value the driver does not know while Send is disabled is not called his draft."""
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "Reset; $script:Value = ('Ask Claude anything' + [char]0x2026); $o.unknown = (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a').Why\n"
            "Reset; $script:Enabled = $true; $script:Value = 'my half-typed thought'; $o.draft = (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a').Why\n"
            "Reset; $script:Value = 'his words'; $o.own_tab = (Test-SafeToClose 'id-a' 'id-a' 'text' '').Why\n"
            "Reset; $script:Value = ('Ask Claude anything' + [char]0x2026); $o.code = (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a').Code\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got["code"], 4)
        self.assertIn("shows 20 characters of text the driver does not recognise while Send is disabled", got["unknown"])
        self.assertIn("if he has no draft there, the placeholder list in the driver (Get-Placeholders) needs an update", got["unknown"])
        self.assertNotIn("draft typed", got["unknown"])
        self.assertNotIn("Ask Claude anything", got["unknown"])
        self.assertIn("does not recognise", got["own_tab"])
        self.assertNotIn("his words", got["own_tab"])
        self.assertEqual(got["draft"], "the chat has a draft typed in its input box (its input is not empty)")

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

    def test_the_check_says_when_a_reveal_or_a_window_restore_may_cure_the_failure(self):
        """Part D: only a tab or a row that was not the active one is cured by revealing; only a minimized window by a restore."""
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "function Flags($c) { '{0}|{1}' -f [bool]$c.Minimized, [bool]$c.Reveal }\n"
            "Reset; [PanelNative]::Iconic = $true; $o.minimized = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a')\n"
            "Reset; $script:Tabs = @((Tab 'chat-a' $false), (Tab 'other' $true)); $o.tab_not_active = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a')\n"
            "Reset; $script:Rows = @((Row 'id-a' $false 'idle'), (Row 'id-b' $true 'idle')); $o.row_not_active = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a')\n"
            "Reset; $script:Tabs = @((Tab 'chat-a' $true), (Tab 'x' $true)); $o.two_active_tabs = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' '')\n"
            "Reset; [PanelNative]::Idle = 5000; $o.operator = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a')\n"
            "Reset; $script:Rec = (Rec 'its session record (pid 1) says busy' ''); $o.busy = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a')\n"
            "Reset; $script:Rows = @((Row 'id-a' $true 'running')); $o.row_running = Flags (Test-SafeToClose 'id-a' 'id-a' 'empty' 'chat-a')\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got, {
            "minimized": "True|False", "tab_not_active": "False|True", "row_not_active": "False|True", "two_active_tabs": "False|True",
            "operator": "False|False", "busy": "False|False", "row_running": "False|False",
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

    def test_a_disabled_send_is_not_enough_the_value_itself_must_read_empty(self):
        """Second review, item 3: the input's value is read; empty is blank or the page's placeholder."""
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "Reset; $script:Value = 'my half-typed thought'; $o.draft_send_disabled = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Value = 'ctrl esc to focus or unfocus Claude'; $o.placeholder = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Value = ('Ask Claude to edit' + [char]0x2026); $o.placeholder_edit = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Value = 'Summarise the last run'; $o.suggestion = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Value = '  '; $o.blank = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Enabled = $true; $script:Value = ''; $o.attachment = Check 'id-a' 'id-a' 'empty' 'chat-a'\n"
            "Reset; $script:Value = 'his words'; $o.text_mode_disabled_his_words = Check 'id-a' 'id-a' 'text' ''\n"
            "Reset; $script:Value = 'ctrl esc to focus or unfocus Claude'; $o.text_mode_empty = Check 'id-a' 'id-a' 'text' ''\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got, {
            "draft_send_disabled": "4|False|",
            "placeholder": "0|False|chat-a",
            "placeholder_edit": "0|False|chat-a",
            "suggestion": "4|False|",
            "blank": "0|False|chat-a",
            "attachment": "4|False|",
            "text_mode_disabled_his_words": "4|False|",
            "text_mode_empty": "0|False|chat-a",
        })

    def test_the_drivers_own_tab_is_proved_by_the_process_its_open_started(self):
        """Second review, items 1 and 11: a tab he reopened has another process, and is never closed as the driver's."""
        got = self.run_cases(
            "$o = [ordered]@{}\n"
            "Reset; $script:Recs = @([pscustomobject]@{ pid = 101 }); $o.own = Check 'id-a' 'id-a' 'empty' 'chat-a' $false 101\n"
            "Reset; $script:Recs = @([pscustomobject]@{ pid = 202 }); $o.his = Check 'id-a' 'id-a' 'empty' 'chat-a' $false 101\n"
            "Reset; $script:Recs = @([pscustomobject]@{ pid = 101 }, [pscustomobject]@{ pid = 202 }); $o.two = Check 'id-a' 'id-a' 'empty' 'chat-a' $false 101\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        self.assertEqual(got, {"own": "0|False|chat-a", "his": "5|False|", "two": "5|False|"})

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
        self.assertEqual(len(re.findall(r"Invoke-Bridge(Ok)? 'close-tab'", code)), 1, "one call, in Close-TabByLabel")
        self.assertIn("@{ label = $Label; all = $false; requireActive = $true }", function_text("Close-TabByLabel"))
        calls = [m.start() for m in re.finditer(r"Close-TabByLabel \$", code)]
        self.assertEqual(len(calls), 1, "Close-TabByLabel is called from Close-Checked only")
        self.assertIn("Close-TabByLabel $c.Label $c.Count", function_text("Close-Checked"))


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
                "Add-Type -TypeDefinition 'public static class PanelNative { public static long OwnStart() { return 42; } }'\n"
                "$Utf8 = New-Object System.Text.UTF8Encoding($false)\n"
                f"$BridgeBase = '{d}'; $LockFile = Join-Path $BridgeBase 'driver.lock'\n"
                "function Stop-Panel([int]$Code, [string]$Reason) { throw ('exit ' + $Code + ': ' + $Reason) }\n"
                "$script:Lock = $null; $script:WaitUntil = (Get-Date).AddSeconds(3)\n"
                "Enter-Lock 'send' 'sid-1'\n"
                "$holder = Read-LockHolder\n"
                "$raw = [System.IO.File]::Open($LockFile, 'Open', 'Read', 'ReadWrite, Delete'); $json = (New-Object System.IO.StreamReader($raw)).ReadToEnd(); $raw.Dispose()\n"
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
                "ConvertTo-Json -Compress -InputObject @($holder, $second, [bool]($waited -ge 2), $leftOver, $holder2, (Test-Path $LockFile), $json)\n"
            )
            got = run_functions(["Enter-Lock", "Exit-Lock", "Read-LockHolder", "Stop-IfCancelled", "Test-CancelRequested", "Test-Touched", "Write-Waiting", "Clear-Waiting", "Add-LogLine"], body)
        holder, second, waited, left_over, holder2, after, raw = got
        self.assertEqual(json.loads(raw)["pid_start"], 42, "the lock records when its process started, so a reused pid is told apart")
        self.assertRegex(holder, r"^pid \d+, send, session sid-1, since ")
        self.assertTrue(second.startswith("exit 3: another driver run held the panel for the whole wait (pid "), second)
        self.assertTrue(waited)
        self.assertTrue(left_over, "the file of a run that ended without letting go stays behind")
        self.assertRegex(holder2, r"^pid \d+, new, session -, since ", "and the next run takes it over at once")
        self.assertFalse(after, "a run that lets go removes the file")

    def test_every_run_that_changes_anything_holds_the_lock(self):
        text = source()
        for action in ["send", "new", "model", "archive", "close-untitled", "selftest"]:
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
        self.assertLess(new.index("$recBefore"), new.index("Invoke-Bridge 'open'"))
        self.assertIn("$recBefore -notcontains", new)
        self.assertIn("$new.Count -ne 1", new)
        self.assertIn("$_.Id -eq $s.Id", new, "the new row must be the new record's own")
        self.assertNotIn("$rowsBefore", new)
        self.assertLess(new.index("untitled"), new.index("Invoke-Bridge 'open'"), "no other untitled tab may be open")
        self.assertLess(new.index("$s.OpenCalled = $true"), new.index("Invoke-Bridge 'open'"), "the open is recorded before it is asked for")


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class ChangeReportTests(unittest.TestCase):
    """Finding 2: every exit says what changed and where the chat's tab is now."""

    def guarded(self, sent: bool, code: int) -> list:
        body = (
            WORLD
            + "$script:Restored = $false\n"
            "function Restore-Chat { $script:Restored = $true }\n"
            "New-RunState 'id-a' 'text'; $script:St.Label = 'chat-a'\n"
            "$script:St.Closed = $true; $script:St.HadTab = $true; $script:St.OpenCalled = $true; $script:St.Opened = $true; $script:St.Prefilled = $true\n"
            "$W.TabOpen = $false\n"
            f"$script:St.Sent = ${str(sent).lower()}\n"
            "$out = $null\n"
            f"try {{ Invoke-Guarded {{ Stop-Panel {code} 'read-back failed' }} }} catch {{ $pe = Get-PanelExit $_; $out = @($pe.Code, $pe.Reason, $script:Restored) }}\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        return run_functions(WORLD_FUNCTIONS, body)
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
        settle = int(re.search(r"^\$SettleSeconds = (\d+)$", source(), re.MULTILINE).group(1))
        self.assertGreater(ps - restore - settle, 600 + 90 + 25, "the longest steps fit before the restore time")
        list_ps = int(re.search(r"^\$ListWorkSeconds = (\d+)$", source(), re.MULTILINE).group(1))
        list_rs = int(re.search(r"pub const DRIVER_LIST_WORK_SECS: u64 = (\d+);", rust).group(1))
        self.assertEqual(list_ps, list_rs, "a list's work time is the same number on both sides")
        self.assertIn("if ($ActionName -eq 'list') { $work = $ListWorkSeconds", function_text("Set-Deadlines"))
        self.assertIn("Set-Deadlines $ActionName", function_text("Invoke-Logged"))

    def test_every_step_wait_is_cut_to_the_deadline(self):
        for name in ["Wait-Ready", "Show-Chat", "Open-WithText", "Confirm-Sent", "Wait-ChatIdle", "Get-OwnPid"]:
            self.assertIn("Get-StepMs", function_text(name), name)
        self.assertIn("Wait-ProcessEnd $RecId", function_text("Close-Checked"))
        self.assertIn("$script:SettleSecs", function_text("Wait-ProcessEnd"), "the process wait has a budget of its own")
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


# ---------------------------------------------------------------- second review: behaviour, not source text


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class RestoreBehaviourTests(unittest.TestCase):
    """Restore-Chat and the message, run against the staged panel: the cleanup's process wait has a budget of
    its own, his tab comes back on every path where it is closed, and the final sentence is a fresh read."""

    def fail_before_send(self, setup: str) -> dict:
        body = (
            WORLD
            + "New-RunState 'id-a' 'hello there'; $s = $script:St; $s.Label = 'chat-a'\n"
            "$s.HadTab = $true; $s.Closed = $true; $s.OpenCalled = $true; $s.Opened = $true; $s.Prefilled = $true; $s.TabOpen = $true; $s.OwnPid = 101\n"
            "$W.TabOpen = $true; $W.Pid = 101; $W.Value = 'hello there'; $W.Enabled = $true\n"
            + setup
            + "$out = [ordered]@{}\n"
            "try { Invoke-Guarded { Stop-Panel 3 'the operator kept using the computer until the deadline (press Send)' } } catch { $pe = Get-PanelExit $_; $out.code = $pe.Code; $out.reason = $pe.Reason }\n"
            "$out.log = @($W.Log); $out.notes = @($script:Notes); $out.tab_open = $W.TabOpen\n"
            "ConvertTo-Json -Compress -Depth 4 -InputObject $out\n"
        )
        return run_functions(WORLD_FUNCTIONS, body)

    def test_the_cleanup_waits_for_the_process_on_its_own_budget_and_his_tab_comes_back(self):
        # The wait for him is spent (he was active until the deadline), he is idle now, and the closed
        # process takes two seconds to end: the old code timed this out and reported the tab OPEN.
        got = self.fail_before_send("$W.Linger = 2; [PanelNative]::Idle = 999999\n")
        self.assertEqual(got["code"], 3)
        self.assertEqual(got["log"][-2:], ["close chat-a", "reveal"], got)
        self.assertTrue(got["tab_open"])
        self.assertIn("then closed that tab again to take the unsent text away, then opened the chat's tab again.", got["reason"])
        self.assertIn("The chat's tab is now open and its input is empty.", got["reason"])
        self.assertNotIn("unsent text:", got["reason"])
        self.assertEqual(got["notes"], [])

    def test_while_he_stays_active_nothing_is_closed_and_the_message_says_what_the_input_really_holds(self):
        got = self.fail_before_send("[PanelNative]::Idle = 0\n")
        self.assertNotIn("close chat-a", got["log"])
        self.assertTrue(got["tab_open"])
        self.assertIn("The chat's tab is OPEN and its input holds the driver's unsent text", got["reason"])
        self.assertTrue(any("was not closed" in n for n in got["notes"]), got)

    def test_a_tab_he_reopened_is_not_closed_as_the_drivers(self):
        got = self.fail_before_send("$W.Pid = 202; $W.Value = ''; $W.Enabled = $false\n")
        self.assertNotIn("close chat-a", got["log"])
        self.assertIn("The chat's tab is now open and its input is empty.", got["reason"])
        self.assertTrue(any("not the one the driver opened" in n for n in got["notes"]), got)

    def test_his_tab_comes_back_when_the_open_never_happened(self):
        got = self.fail_before_send("$s.Opened = $false; $s.Prefilled = $false; $s.TabOpen = $false; $W.TabOpen = $false\n")
        self.assertEqual(got["log"], ["reveal"], got)
        self.assertIn("asked the bridge to open it (no new tab was seen), then opened the chat's tab again.", got["reason"])
        self.assertIn("The chat's tab is now open and its input is empty.", got["reason"])

    def test_the_final_state_is_read_not_inferred_from_what_was_done(self):
        # The reveal is asked for but nothing opens: the message says closed, whatever the flags would say.
        got = self.fail_before_send("$s.Opened = $false; $s.Prefilled = $false; $s.TabOpen = $false; $W.TabOpen = $false; $W.RevealWorks = $false\n")
        self.assertIn("The chat's tab is now closed. It was open before the run.", got["reason"])
        self.assertTrue(any("did not come back" in n for n in got["notes"]), got)

    # Third review, item 1: open or closed comes from the tab list; the process outlives its tab.

    def test_his_tab_closed_while_its_process_lingers_past_the_budget_is_reported_closed_and_not_reopened(self):
        # The reviewer's sequence: his tab is closed, its process lives 30 s against a 5 s budget.
        got = self.fail_before_send("$s.Opened = $false; $s.Prefilled = $false; $s.OpenCalled = $false; $s.TabOpen = $false; $W.TabOpen = $false; $W.ProcUntil = (Get-Date).AddSeconds(30)\n")
        self.assertNotIn("reveal", got["log"], got)
        self.assertFalse(got["tab_open"])
        self.assertIn("The chat's tab is now closed; its process has not ended yet. It was open before the run.", got["reason"])
        self.assertNotIn("now open", got["reason"])
        self.assertTrue(any("was not opened again: its process has not ended yet" in n and "panel_bridge.py reveal" in n and "id-a" in n for n in got["notes"]), got)

    def test_his_tab_is_reopened_once_the_lingering_process_ends_inside_the_budget(self):
        got = self.fail_before_send("$s.Opened = $false; $s.Prefilled = $false; $s.OpenCalled = $false; $s.TabOpen = $false; $W.TabOpen = $false; $W.ProcUntil = (Get-Date).AddSeconds(2)\n")
        self.assertEqual(got["log"], ["reveal"], got)
        self.assertTrue(got["tab_open"])
        self.assertIn("then opened the chat's tab again.", got["reason"])
        self.assertIn("The chat's tab is now open and its input is empty.", got["reason"])

    def test_the_drivers_own_tab_closed_with_its_process_lingering_counts_as_closed_and_says_so(self):
        got = self.fail_before_send("$W.Linger = 30; [PanelNative]::Idle = 999999\n")
        self.assertIn("close chat-a", got["log"])
        self.assertNotIn("reveal", got["log"], got)
        self.assertIn("then closed that tab again to take the unsent text away.", got["reason"])
        self.assertIn("The chat's tab is now closed; its process has not ended yet. It was open before the run.", got["reason"])
        self.assertFalse(any("was not closed" in n for n in got["notes"]), got)
        self.assertTrue(any("the tab the driver opened was closed; its process has not ended yet" in n for n in got["notes"]), got)
        self.assertTrue(any("panel_bridge.py reveal" in n for n in got["notes"]), got)

    def test_a_close_the_bridge_carries_out_after_its_timeout_is_reported_closed(self):
        # The real Close-TabByLabel: the bridge does not answer close-tab, the tab list read right after
        # still shows the tab, then the bridge closes it late and the process lingers.
        got = self.fail_before_send(
            "$s.Closed = $false; $s.OpenCalled = $false; $s.Opened = $false; $s.Prefilled = $false; $s.OwnPid = 0\n"
            "$W.Value = ''; $W.Enabled = $false; $W | Add-Member LateClose 0\n"
            "function Set-Begun { }\n"
            ". ([scriptblock]::Create(@($defs | Where-Object { $_.Name -eq 'Close-TabByLabel' })[0].Extent.Text))\n"
            "function Invoke-Bridge([string]$Name, $BArgs, [int]$TimeoutSec = 25) { $W.Log += ('bridge ' + $Name); $W.LateClose = 2; $null }\n"
            "function Get-ClaudeTabs { if ($W.LateClose -gt 0) { $W.LateClose--; if ($W.LateClose -eq 0) { $W.TabOpen = $false; $W.ProcUntil = (Get-Date).AddSeconds(30) } }; $t = @([pscustomobject]@{ claude = $true; label = 'other'; isActive = (-not $W.TabOpen) }); if ($W.TabOpen) { $t += [pscustomobject]@{ claude = $true; label = 'chat-a'; isActive = $true } }; $t }\n"
            "function Stop-Main { Close-Checked 'id-a' 'id-a' 'empty' 'chat-a' 'id-a' }\n"
            "[PanelNative]::Idle = 999999\n"
            "$out = [ordered]@{}\n"
            "try { Invoke-Guarded { Stop-Main } } catch { $pe = Get-PanelExit $_; $out.code = $pe.Code; $out.reason = $pe.Reason }\n"
            "$out.log = @($W.Log); $out.notes = @($script:Notes); $out.tab_open = $W.TabOpen\n"
            "ConvertTo-Json -Compress -Depth 4 -InputObject $out\n"
            "exit 0\n"
        )
        self.assertEqual(got["code"], 2, got)
        self.assertIn("bridge close-tab", got["log"])
        self.assertIn("asked the bridge to close the chat's tab (it did not confirm).", got["reason"])
        self.assertIn("The chat's tab is now closed; its process has not ended yet. It was open before the run.", got["reason"])
        self.assertNotIn("now open", got["reason"])

    def test_an_unreadable_tab_list_is_reported_unknown_and_nothing_is_reopened(self):
        got = self.fail_before_send(
            "$s.Opened = $false; $s.Prefilled = $false; $s.OpenCalled = $false; $s.TabOpen = $false; $W.TabOpen = $false\n"
            "function Get-ClaudeTabs { throw 'the bridge did not answer' }\n"
        )
        self.assertNotIn("reveal", got["log"], got)
        self.assertIn("The state of the chat's tab is unknown: the bridge's tab list could not be read.", got["reason"])
        self.assertTrue(any("is unknown" in n and "panel_bridge.py reveal" in n for n in got["notes"]), got)

    def test_after_send_nothing_is_put_back(self):
        code, reason, restored = self.guarded_after_send()
        self.assertEqual(code, 1)
        self.assertFalse(restored)
        self.assertIn("Send was pressed, so the text may have reached the chat", reason)

    def guarded_after_send(self):
        body = (
            WORLD
            + "$script:Restored = $false\n"
            "function Restore-Chat { $script:Restored = $true }\n"
            "New-RunState 'id-a' 'text'; $script:St.OpenCalled = $true; $script:St.Opened = $true; $script:St.Sent = $true\n"
            "$out = $null\n"
            "try { Invoke-Guarded { Stop-Panel 5 'read-back failed' } } catch { $pe = Get-PanelExit $_; $out = @($pe.Code, $pe.Reason, $script:Restored) }\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        return run_functions(WORLD_FUNCTIONS, body)


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class CloseLoopBehaviourTests(unittest.TestCase):
    """Close-Checked run with a scripted check: a retry waits 2 seconds, reveals only when the last failure was
    that the tab or the row was not the active one, restores a window that was minimized again, and says why
    it waits; the close follows only a passing check, and a check that cannot pass stops with no close."""

    def run_loop(self, results: str, wait_until: str = "(Get-Date).AddSeconds(30)") -> dict:
        body = (
            MOCK_NATIVE
            + "$script:Restoring = $false; $script:Log = @(); $script:Waits = @()\n"
            f"$script:WaitUntil = {wait_until}; $script:StepUntil = $script:WaitUntil\n"
            "$script:St = [pscustomobject]@{ Closed = $false; CloseAsked = $false; OpenCalled = $false; Opened = $false; TabOpen = $true }\n"
            "function Res([int]$Code, [bool]$Retry, [bool]$Reveal = $false, [bool]$Min = $false) { [pscustomobject]@{ Code = $Code; Why = 'w' + $Code; Retry = $Retry; Label = 'chat-a'; Count = 1; Reveal = $Reveal; Minimized = $Min } }\n"
            f"$script:Queue = [System.Collections.ArrayList]@({results})\n"
            "function Test-SafeToClose { $script:Log += 'check'; $r = $script:Queue[0]; if ($script:Queue.Count -gt 1) { $script:Queue.RemoveAt(0) }; $r }\n"
            "function Wait-Idle([string]$What) { $script:Log += 'wait' }\n"
            "function Show-Chat([string]$Id) { $script:Log += 'show' }\n"
            "function Restore-Minimized { $script:Log += 'restore' }\n"
            "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Log += ('sleep' + $Seconds) }\n"
            "function Write-Waiting([string]$Reason) { $script:Waits += $Reason }\n"
            "function Clear-Waiting { }\n"
            "function Close-TabByLabel([string]$Label, [int]$Count) { $script:Log += ('close ' + $Label + ' ' + $Count) }\n"
            "function Wait-ProcessEnd([string]$Id) { $script:Log += 'gone'; $true }\n"
            "$out = [ordered]@{ code = 0 }\n"
            "try { Close-Checked 'id-a' 'id-a' 'empty' 'chat-a' 'id-a' } catch { $out.code = (Get-PanelExit $_).Code }\n"
            "$out.log = @($script:Log) -join ' '; $out.closed = $script:St.Closed; $out.open = $script:St.TabOpen; $out.waits = @($script:Waits)\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        return run_functions(["Close-Checked", "Get-WaitUntil", "Test-Touched", "Get-PanelExit"], body)

    def test_a_retry_waits_2_seconds_and_reveals_only_after_a_tab_or_row_that_was_not_active(self):
        got = self.run_loop("(Res 3 $true), (Res 5 $true $true), (Res 0 $false)")
        self.assertEqual(got["log"], "wait show check sleep2 wait check sleep2 wait show check close chat-a 1 gone")
        self.assertEqual((got["code"], got["closed"], got["open"]), (0, True, False))
        self.assertEqual(got["waits"], ["before closing the chat tab: w3", "before closing the chat tab: w5"], "why it waits, in the order it changed")

    def test_a_long_wait_for_a_retryable_reason_reveals_once_not_every_second(self):
        """Found live: the driver revealed 107 times in two minutes and logged nothing about why."""
        got = self.run_loop("@(1..40 | ForEach-Object { Res 3 $true }) + @(Res 0 $false)")
        words = got["log"].split()
        self.assertEqual(words.count("show"), 1, got["log"])
        self.assertEqual(words.count("sleep2"), 40, "one 2-second wait per failed try")
        self.assertEqual(words.count("check"), 41)
        self.assertEqual((got["code"], got["closed"]), (0, True))

    def test_a_check_that_waiting_cannot_cure_stops_with_its_code_and_no_close(self):
        got = self.run_loop("(Res 3 $true), (Res 4 $false)")
        self.assertEqual({k: got[k] for k in ("code", "log", "closed", "open")}, {"code": 4, "log": "wait show check sleep2 wait check", "closed": False, "open": True})

    def test_past_the_deadline_a_retry_stops_with_no_close(self):
        got = self.run_loop("(Res 3 $true)", wait_until="(Get-Date).AddSeconds(-1)")
        self.assertEqual({k: got[k] for k in ("code", "log", "closed", "open")}, {"code": 3, "log": "wait show check", "closed": False, "open": True})

    def test_a_window_that_was_minimized_again_is_restored_inside_the_retry(self):
        """Found live: Ensure-Screen runs once, and the window was minimized again while the close retried."""
        got = self.run_loop("(Res 3 $true $false $true), (Res 0 $false)")
        self.assertEqual(got["log"], "wait show check restore wait check close chat-a 1 gone")
        self.assertEqual(got["waits"], ["before closing the chat tab: w3"])
        self.assertTrue(got["closed"])

    def test_the_retry_code_sits_after_the_close_and_the_close_check_stays_unbroken(self):
        close = function_text("Close-Checked")
        self.assertLess(close.index("Close-TabByLabel"), close.index("Write-Waiting"), "the log line is written after the close path, never between the check and the close")
        self.assertIn("Restore-Minimized", close)
        self.assertIn("[bool]$c.Reveal", close)
        self.assertIn("Start-Sleep -Seconds 2", close)

    def test_restore_minimized_never_activates_and_restores_in_the_driver_order(self):
        # A native stand-in that records the calls and keeps the foreground where it was.
        native = (
            "Add-Type -TypeDefinition @'\n"
            "using System;\n"
            "using System.Collections.Generic;\n"
            "public class PanelExit : Exception { public int Code; public string Reason; public PanelExit(int c, string r) : base(r) { Code = c; Reason = r; } }\n"
            "public static class PanelNative { public static List<string> Calls = new List<string>(); public static IntPtr Fg = new IntPtr(9); public static bool MoveFg = false;\n"
            "  public static string Log { get { return string.Join(\" \", Calls.ToArray()); } }\n"
            "  public static double IdleMs() { return 999999; }\n"
            "  public static bool IsWindow(IntPtr h) { return true; }\n"
            "  public static bool IsTopmost(IntPtr h) { return false; }\n"
            "  public static int[] GetPlacement(IntPtr h) { return null; }\n"
            "  public static IntPtr Foreground() { return Fg; }\n"
            "  public static bool PlaceBottom(IntPtr h) { Calls.Add(\"bottom\"); return true; }\n"
            "  public static bool ShowNoActivate(IntPtr h) { Calls.Add(\"show4\"); if (MoveFg) Fg = new IntPtr(5); return true; }\n"
            "  public static bool PlaceBehind(IntPtr h, IntPtr f) { Calls.Add(\"behind\" + f.ToInt64()); return true; }\n"
            "}\n"
            "'@\n"
            "$script:Hwnd = [IntPtr]5; $script:WasMinimized = $false; $script:Notes = @(); $script:Log = @()\n"
            "function Add-Note([string]$Note) { $script:Notes += $Note }\n"
            "function Wait-Idle([string]$What) { $script:Log += 'wait' }\n"
            "function Wait-Tree { $script:Log += 'tree' }\n"
            "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Log += ('sleep' + $Milliseconds) }\n"
        )
        body = (
            native
            + "$out = [ordered]@{}\n"
            "Restore-Minimized\n"
            "$out.native = [PanelNative]::Log; $out.steps = @($script:Log) -join ' '; $out.min = $script:WasMinimized; $out.notes = @($script:Notes)\n"
            "[PanelNative]::Calls.Clear(); [PanelNative]::MoveFg = $true; $script:Notes = @(); $script:Log = @()\n"
            "Restore-Minimized\n"
            "$out.moved_notes = @($script:Notes)\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        got = run_functions(["Restore-Minimized", "Set-VsCodeBehind", "Confirm-VsCodeNotTopmost"], body)
        self.assertEqual(got["native"], "bottom show4 behind9", "bottom while minimized, shown without activation, directly behind his foreground window")
        self.assertEqual(got["steps"], "wait sleep2500 tree")
        self.assertTrue(got["min"])
        self.assertEqual(got["notes"], [])
        self.assertEqual(len(got["moved_notes"]), 1)
        self.assertIn("foreground window changed", got["moved_notes"][0], "a changed foreground is a note, never a failure")


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class WaitingLogAndCancelTests(unittest.TestCase):
    """Why a run waits is logged once per change of reason, and -CancelFile stops a run that only waits."""

    def stage(self, d: str) -> str:
        return (
            MOCK_NATIVE
            + "$Utf8 = New-Object System.Text.UTF8Encoding($false)\n"
            f"$LogDir = '{d}'; $LogFile = Join-Path $LogDir 'claude-panel.log'\n"
            "$script:RunAction = 'send'; $script:RunId = 'abc-123'; $script:LastWait = ''; $script:Restoring = $false; $script:Notes = @()\n"
            "$script:St = $null; $script:Sleeps = 0\n"
            "$script:WaitUntil = (Get-Date).AddMinutes(5); $script:StepUntil = $script:WaitUntil\n"
            "function Add-Note([string]$Note) { $script:Notes += $Note }\n"
            "function Stop-Panel([int]$Code, [string]$Reason) { throw ('exit ' + $Code + ': ' + $Reason) }\n"
            "function Read-Log { if (Test-Path $LogFile) { @([System.IO.File]::ReadAllLines($LogFile)) } else { @() } }\n"
        )

    FUNCS = ["Add-LogLine", "Write-Waiting", "Clear-Waiting", "Test-CancelRequested", "Stop-IfCancelled", "Test-Touched", "Get-WaitUntil", "Wait-Idle", "Wait-Guards", "Wait-RowIdle", "Set-Begun"]

    def test_a_reason_is_logged_once_until_it_changes_and_never_carries_text(self):
        with tempfile.TemporaryDirectory() as d:
            body = (
                self.stage(d)
                + "Write-Waiting 'the operator is using the computer'\n"
                "Write-Waiting 'the operator is using the computer'\n"
                "Write-Waiting 'its session record (pid 7) says busy'\n"
                "Write-Waiting 'the operator is using the computer'\n"
                "Clear-Waiting\n"
                "Write-Waiting 'the operator is using the computer'\n"
                "ConvertTo-Json -Compress -InputObject @(Read-Log)\n"
            )
            got = run_functions(self.FUNCS, body)
        self.assertEqual(len(got), 4, got)
        self.assertRegex(got[0], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d send abc-123 waiting: the operator is using the computer$")
        self.assertTrue(got[1].endswith("waiting: its session record (pid 7) says busy"))
        self.assertTrue(got[2].endswith("waiting: the operator is using the computer"))

    def test_wait_guards_and_wait_idle_explain_a_long_wait_in_the_log(self):
        with tempfile.TemporaryDirectory() as d:
            body = (
                self.stage(d)
                + "$script:Rec = [pscustomobject]@{ Count = 1; Busy = 'its session record (pid 7) says busy'; Terminal = '' }\n"
                "function Get-RecordState([string]$Id) { $script:Rec }\n"
                "[PanelNative]::Idle = 0\n"
                "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Sleeps++\n"
                "  if ($script:Sleeps -eq 3) { [PanelNative]::Idle = 999999 }\n"
                "  if ($script:Sleeps -eq 6) { $script:Rec = [pscustomobject]@{ Count = 1; Busy = ''; Terminal = '' } }\n"
                "  if ($script:Sleeps -eq 8) { [PanelNative]::Idle = 999999 } }\n"
                "Wait-Guards 'abc-123'\n"
                "$afterGuards = @(Read-Log)\n"
                "[PanelNative]::Idle = 0; $script:Sleeps = 5\n"
                "Wait-Idle 'close a chat tab'\n"
                "ConvertTo-Json -Compress -InputObject @{ guards = $afterGuards; all = @(Read-Log) }\n"
            )
            got = run_functions(self.FUNCS, body)
        reasons = [line.split(" waiting: ", 1)[1] for line in got["guards"]]
        self.assertEqual(reasons, ["the operator is using the computer", "its session record (pid 7) says busy"], "one line per change of reason, not one per 2 seconds")
        all_reasons = [line.split(" waiting: ", 1)[1] for line in got["all"]]
        self.assertEqual(all_reasons[-1], "the operator is using the computer (needed to close a chat tab)", "a new wait is logged again")
        for line in got["all"]:
            self.assertRegex(line, r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d send abc-123 waiting: ")

    def test_the_cancel_file_stops_only_a_run_that_has_changed_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            cancel = str(Path(d) / "waiter-x.cancel")
            body = (
                self.stage(d)
                + f"$CancelFile = '{cancel}'\n"
                "$o = [ordered]@{}\n"
                "$o.no_file = Test-CancelRequested\n"
                "[System.IO.File]::WriteAllText($CancelFile, '{}')\n"
                "$o.file_and_untouched = Test-CancelRequested\n"
                "$script:St = [pscustomobject]@{ Closed = $false; CloseAsked = $false; OpenCalled = $false }\n"
                "$o.file_and_state_untouched = Test-CancelRequested\n"
                "$script:St.CloseAsked = $true; $o.after_a_close = Test-CancelRequested\n"
                "$script:St.CloseAsked = $false; $script:St.OpenCalled = $true; $o.after_an_open = Test-CancelRequested\n"
                "$script:St.OpenCalled = $false; $script:Restoring = $true; $o.while_putting_back = Test-CancelRequested\n"
                "$script:Restoring = $false; $CancelFile = ''; $o.no_cancel_file_param = Test-CancelRequested\n"
                "ConvertTo-Json -Compress -InputObject $o\n"
            )
            got = run_functions(self.FUNCS, body)
        self.assertEqual(got, {
            "no_file": False, "file_and_untouched": True, "file_and_state_untouched": True, "after_a_close": False,
            "after_an_open": False, "while_putting_back": False, "no_cancel_file_param": False,
        })

    def test_every_wait_before_the_first_change_stops_with_exit_7_when_the_file_exists(self):
        with tempfile.TemporaryDirectory() as d:
            cancel = str(Path(d) / "waiter-x.cancel")
            Path(cancel).write_text("{}")
            body = (
                self.stage(d)
                + f"$CancelFile = '{cancel}'; $IdleSeconds = 120\n"
                "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Sleeps++; throw 'it slept instead of stopping' }\n"
                "function Get-RecordState([string]$Id) { [pscustomobject]@{ Count = 0; Busy = 'busy'; Terminal = '' } }\n"
                "function Get-Chat([string]$Id) { [pscustomobject]@{ Status = 'running'; Name = 'n' } }\n"
                "$o = [ordered]@{}\n"
                "foreach ($case in @('guards', 'idle-while-idle', 'idle-while-active', 'row')) {\n"
                "  try {\n"
                "    if ($case -eq 'guards') { [PanelNative]::Idle = 0; Wait-Guards 'abc-123' }\n"
                "    elseif ($case -eq 'idle-while-idle') { [PanelNative]::Idle = 999999; Wait-Idle 'x' }\n"
                "    elseif ($case -eq 'idle-while-active') { [PanelNative]::Idle = 0; Wait-Idle 'x' }\n"
                "    else { Wait-RowIdle 'abc-123' }\n"
                "    $o[$case] = 'returned'\n"
                "  } catch { $o[$case] = $_.Exception.Message }\n"
                "}\n"
                "$o.sleeps = $script:Sleeps\n"
                "ConvertTo-Json -Compress -InputObject $o\n"
            )
            got = run_functions(self.FUNCS, body)
        for case in ("guards", "idle-while-idle", "idle-while-active", "row"):
            self.assertEqual(got[case], "exit 7: cancelled; nothing was changed", case)
        self.assertEqual(got["sleeps"], 0)

    def test_a_touched_run_is_not_cancelled_and_finishes_its_wait(self):
        with tempfile.TemporaryDirectory() as d:
            cancel = str(Path(d) / "waiter-x.cancel")
            Path(cancel).write_text("{}")
            body = (
                self.stage(d)
                + f"$CancelFile = '{cancel}'; $IdleSeconds = 120\n"
                "$script:St = [pscustomobject]@{ Closed = $true; CloseAsked = $true; OpenCalled = $false }\n"
                "[PanelNative]::Idle = 999999\n"
                "Wait-Idle 'press Send'\n"
                "ConvertTo-Json -Compress -InputObject 'ok'\n"
            )
            got = run_functions(self.FUNCS, body)
        self.assertEqual(got, 'ok')

    def test_the_lock_wait_stops_for_the_cancel_file_too(self):
        with tempfile.TemporaryDirectory() as d:
            cancel = str(Path(d) / "waiter-x.cancel")
            Path(cancel).write_text("{}")
            body = (
                self.stage(d)
                + f"$CancelFile = '{cancel}'; $BridgeBase = '{d}'; $LockFile = Join-Path $BridgeBase 'driver.lock'; $script:Lock = $null\n"
                "function Read-LockHolder { 'pid 1, send, session s, since t' }\n"
                "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) throw 'it slept instead of stopping' }\n"
                "$held = [System.IO.File]::Open($LockFile, 'OpenOrCreate', 'ReadWrite', 'Read')\n"
                "$msg = ''\n"
                "try { Enter-Lock 'send' 'abc-123' } catch { $msg = $_.Exception.Message }\n"
                "$held.Dispose()\n"
                "ConvertTo-Json -Compress -InputObject @($msg, @(Read-Log).Count)\n"
            )
            got = run_functions(self.FUNCS + ["Enter-Lock", "Exit-Lock"], body)
        self.assertEqual(got[0], "exit 7: cancelled; nothing was changed")

    def test_a_run_that_begins_changing_the_panel_leaves_the_sending_marker_next_to_the_cancel_file(self):
        with tempfile.TemporaryDirectory() as d:
            cancel = str(Path(d) / "waiter-x.cancel")
            body = (
                self.stage(d)
                + f"$CancelFile = '{cancel}'\n"
                "Set-Begun\n"
                "$a = Test-Path (Join-Path $LogDir 'waiter-x.sending')\n"
                "$CancelFile = ''\n"
                "Remove-Item (Join-Path $LogDir 'waiter-x.sending')\n"
                "Set-Begun\n"
                "$b = Test-Path (Join-Path $LogDir 'waiter-x.sending')\n"
                "ConvertTo-Json -Compress -InputObject @($a, $b)\n"
            )
            got = run_functions(self.FUNCS, body)
        self.assertEqual(got, [True, False])

    def test_the_first_change_of_every_action_writes_that_marker(self):
        for name in ["Close-TabByLabel", "Open-WithText", "Do-New"]:
            self.assertIn("Set-Begun", function_text(name), name)
        self.assertIn("Set-Begun", function_text("Do-Model"))
        close = function_text("Close-TabByLabel")
        self.assertLess(close.index("$s.CloseAsked = $true"), close.index("Invoke-Bridge 'close-tab'"))
        # A cancel stops the lock wait and every guard loop.
        for name in ["Enter-Lock", "Wait-Idle", "Wait-Guards", "Wait-RowIdle"]:
            self.assertIn("Stop-IfCancelled", function_text(name), name)
        self.assertIn("Stop-Panel 7 'cancelled; nothing was changed'", function_text("Stop-IfCancelled"))

    def test_exit_7_reads_as_a_cancel_not_as_a_send_failure(self):
        guarded = function_text("Invoke-Guarded")
        self.assertIn("if ($code -eq 7)", guarded)
        body = (
            MOCK_NATIVE
            + "$script:St = [pscustomobject]@{ Sent = $false; Id = ''; Closed = $false; CloseAsked = $false; OpenCalled = $false }\n"
            "function Restore-Chat { }\nfunction Get-StateText { 'The panel was not changed: the chat''s tab is as it was.' }\n"
            "try { Invoke-Guarded { Stop-Panel 7 'cancelled; nothing was changed' } } catch { $pe = Get-PanelExit $_; $o = @($pe.Code, $pe.Reason) }\n"
            "ConvertTo-Json -Compress -InputObject $o\n"
        )
        code, reason = run_functions(["Invoke-Guarded", "Get-PanelExit"], body)
        self.assertEqual(code, 7)
        self.assertEqual(reason, "cancelled; nothing was changed. The panel was not changed: the chat's tab is as it was.")
        self.assertNotIn("Nothing was sent", reason)


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class ArchiveBehaviourTests(unittest.TestCase):
    """The archive stage and Do-Archive run against a staged screen: a native stand-in that records every pointer
    and window call (Calls) and every driver step in order (Seq), and that can run a hook when a named step
    happens, so a test can make him come back, make a full-screen window appear or make the chat start running at
    a chosen moment. The stand-in Wait-Idle fails the run when he is not idle (the real one would wait), so a
    wait that comes after the raise shows up as a 'wait-idle' step after 'raise'. Nothing touches the desktop."""

    NATIVE = (
        "Add-Type -AssemblyName UIAutomationClient\n"
        "Add-Type -TypeDefinition @'\n"
        "using System;\n"
        "using System.Collections.Generic;\n"
        "public class PanelExit : Exception { public int Code; public string Reason; public PanelExit(int c, string r) : base(r) { Code = c; Reason = r; } }\n"
        "public static class PanelNative {\n"
        "  public static double Idle = 999999; public static List<string> Calls = new List<string>(); public static List<string> Seq = new List<string>();\n"
        "  public static Action<string> OnStep;\n"
        "  public static int[] Ptr = new int[] { 10, 20 }; public static IntPtr RootUnder = new IntPtr(5); public static IntPtr RootAfterRaise = new IntPtr(5); public static IntPtr Fg = new IntPtr(9);\n"
        "  public static int Quns = 5; public static bool Alive = true; public static bool Topmost = false; public static bool BehindTopmost = false; public static bool Stubborn = false; public static int FailReads = 0;\n"
        "  public static int[] Placement = new int[] { 2, 2, 0, 0, 0, 0, 100, 100, 900, 700 };\n"
        "  public static IntPtr AboveWin = IntPtr.Zero; public static HashSet<long> TopWins = new HashSet<long>();\n"
        "  public static string Log { get { return string.Join(\" \", Calls.ToArray()); } }\n"
        "  public static string Order { get { return string.Join(\" \", Seq.ToArray()); } }\n"
        "  public static void Ev(string n) { Seq.Add(n); if (OnStep != null) OnStep(n); }\n"
        "  public static double IdleMs() { return Idle; }\n"
        "  public static bool IsWindow(IntPtr h) { return Alive || h.ToInt64() != 7; }\n"
        "  public static bool IsTopmost(IntPtr h) { if (h.ToInt64() == 5) return Topmost; return TopWins.Contains(h.ToInt64()); }\n"
        "  public static IntPtr Above(IntPtr h) { Ev(\"above\"); return AboveWin; }\n"
        "  public static IntPtr Foreground() { return Fg; }\n"
        "  public static int[] PointerPos() { if (FailReads > 0) { FailReads--; return null; } return new int[] { Ptr[0], Ptr[1] }; }\n"
        "  public static bool MovePointer(int x, int y) { Calls.Add(\"move \" + x + \",\" + y); Ptr = new int[] { x, y }; Ev(\"move\"); return true; }\n"
        "  public static IntPtr RootAt(int x, int y) { return RootUnder; }\n"
        "  public static bool RaiseNoActivate(IntPtr h) { Calls.Add(\"raise\"); Topmost = true; RootUnder = RootAfterRaise; Ev(\"raise\"); return true; }\n"
        "  public static bool ClearTopmost(IntPtr h) { Calls.Add(\"notopmost\"); if (!Stubborn) Topmost = false; Ev(\"notopmost\"); return true; }\n"
        "  public static bool PlaceBehind(IntPtr h, IntPtr f) { Calls.Add(\"behind\" + f.ToInt64()); if (BehindTopmost) Topmost = true; Ev(\"behind\"); return true; }\n"
        "  public static bool PlaceBottom(IntPtr h) { Calls.Add(\"bottom\"); Topmost = false; Ev(\"bottom\"); return true; }\n"
        "  public static bool ShowNoActivate(IntPtr h) { Calls.Add(\"show4\"); Ev(\"show4\"); return true; }\n"
        "  public static bool MinimizeNoActivate(IntPtr h) { Calls.Add(\"min7\"); return true; }\n"
        "  public static int[] GetPlacement(IntPtr h) { Calls.Add(\"getplacement\"); return Placement; }\n"
        "  public static bool MinimizeWithPlacement(IntPtr h, int[] q) { Calls.Add(\"minplace flags=\" + q[0] + \" rect=\" + q[6] + \",\" + q[7]); return true; }\n"
        "  public static int NotificationState() { Ev(\"fs?\" + Quns); return Quns; }\n"
        "  public static void Reset() { Idle = 999999; Calls.Clear(); Seq.Clear(); OnStep = null; Ptr = new int[] { 10, 20 }; RootUnder = new IntPtr(5); RootAfterRaise = new IntPtr(5); Fg = new IntPtr(9); Quns = 5; Alive = true; Topmost = false; BehindTopmost = false; Stubborn = false; FailReads = 0; AboveWin = IntPtr.Zero; TopWins.Clear(); }\n"
        "}\n"
        "'@\n"
        "$CT = [System.Windows.Automation.ControlType]\n"
        "$IdleSeconds = 120; $RowPrefix = 'sessions-list-row-'\n"
        "$script:Hwnd = [IntPtr]5; $script:Notes = @(); $script:Log = @(); $script:Sleeps = 0; $script:Restoring = $false; $script:Cancelled = $false\n"
        "$script:WaitUntil = (Get-Date).AddMinutes(5); $script:Waits = @(); $script:RowGone = $false; $script:ButtonAfter = 1; $script:Polls = 0\n"
        "$script:RowStatus = 'idle'; $script:RowActive = $false; $script:Rs = [pscustomobject]@{ Count = 0; Busy = ''; Terminal = '' }; $script:KeepRow = $false\n"
        "$script:Win = [pscustomobject]@{ Current = [pscustomobject]@{ BoundingRectangle = [pscustomobject]@{ X = 0; Y = 0; Width = 2000; Height = 1200 } } }\n"
        "$script:St = [pscustomobject]@{ Sent = $false }\n"
        "function Add-Note([string]$Note) { $script:Notes += $Note }\n"
        "function Stop-Panel([int]$Code, [string]$Reason) { throw (New-Object PanelExit($Code, $Reason)) }\n"
        "$script:LastWait = ''\n"
        "function Clear-Waiting { $script:LastWait = '' }\n"
        "function Wait-Idle([string]$What) { [PanelNative]::Ev('wait-idle'); if ([PanelNative]::IdleMs() -lt 120000) { Stop-Panel 3 'the operator kept using the computer until the deadline' }; Clear-Waiting; [PanelNative]::Ev('idle-ok') }\n"
        "function Stop-IfCancelled { }\n"
        "function Write-Waiting([string]$Reason) { if ($Reason -ne $script:LastWait) { $script:LastWait = $Reason; $script:Waits += $Reason } }\n"
        "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Sleeps++; if ($Seconds -gt 0) { [PanelNative]::Ev('sleep' + $Seconds + 's') } else { [PanelNative]::Ev('sleep' + $Milliseconds) } }\n"
        "function Hook([scriptblock]$Block) { [PanelNative]::OnStep = [System.Action[string]]$Block }\n"
        "function Fake-Row { $r = [pscustomobject]@{ Current = [pscustomobject]@{ IsOffscreen = $false; BoundingRectangle = [pscustomobject]@{ X = 100; Y = 200; Width = 300; Height = 40 } } }\n"
        "  $r | Add-Member -MemberType ScriptMethod -Name TryGetCurrentPattern -Value { param($p, $ref) $false }; $r }\n"
        "function Find-RowElement([string]$Id) { if ($script:RowGone) { $null } else { Fake-Row } }\n"
        "function Get-Chats($Window) { @([pscustomobject]@{ Id = 'id-a'; Status = $script:RowStatus; Active = $script:RowActive }) }\n"
        "function Get-RecordState([string]$Id) { $script:Rs }\n"
        "function Get-ArchiveButton([string]$Id) {\n"
        "  $script:Polls++\n"
        "  if ($script:Polls -lt $script:ButtonAfter) { return $null }\n"
        "  [PanelNative]::Ev('button')\n"
        "  $b = [pscustomobject]@{ }\n"
        "  $b | Add-Member -MemberType ScriptMethod -Name GetCurrentPattern -Value { param($p) $inv = [pscustomobject]@{ }; $inv | Add-Member -MemberType ScriptMethod -Name Invoke -Value { [PanelNative]::Ev('invoke'); if (-not $script:KeepRow) { $script:RowGone = $true } }; $inv }\n"
        "  $b\n"
        "}\n"
        "function Scenario([scriptblock]$Setup) {\n"
        "  [PanelNative]::Reset(); $script:Notes = @(); $script:Waits = @(); $script:LastWait = ''; $script:Sleeps = 0; $script:RowGone = $false; $script:ButtonAfter = 1; $script:Polls = 0\n"
        "  $script:KeepRow = $false; $script:St.Sent = $false; $script:WaitUntil = (Get-Date).AddMinutes(5); $script:RowStatus = 'idle'; $script:RowActive = $false\n"
        "  $script:Rs = [pscustomobject]@{ Count = 0; Busy = ''; Terminal = '' }\n"
        "  & $Setup\n"
        "  $code = 0; $reason = ''\n"
        "  try { Invoke-ArchiveStage 'id-a' } catch { $e = $_.Exception; while ($null -ne $e -and -not ($e -is [PanelExit])) { $e = $e.InnerException }; if ($null -ne $e) { $code = $e.Code; $reason = $e.Reason } else { $code = 99; $reason = $_.Exception.Message } }\n"
        "  [pscustomobject]@{ code = $code; reason = $reason; calls = [PanelNative]::Log; order = [PanelNative]::Order; ptr = ([string][PanelNative]::Ptr[0] + ',' + [string][PanelNative]::Ptr[1]); notes = @($script:Notes); waits = @($script:Waits); sent = $script:St.Sent; sleeps = $script:Sleeps }\n"
        "}\n"
    )
    FUNCS = [
        "Invoke-ArchiveStage", "Get-RowCenter", "Test-PointerAt", "Restore-Pointer", "Undo-Raise", "Set-VsCodeBehind",
        "Test-FullScreenFront", "Get-OperatorBack", "Get-ArchiveBlock", "Confirm-VsCodeNotTopmost",
    ]

    def run_scenarios(self, body: str):
        return run_functions(self.FUNCS, self.NATIVE + "$o = [ordered]@{}\n" + body + "ConvertTo-Json -Compress -Depth 5 -InputObject $o\n")

    @staticmethod
    def after(order: str, step: str) -> list:
        """The steps that came after the first `step`."""
        words = order.split()
        return words[words.index(step) + 1:] if step in words else []

    def test_an_uncovered_row_gets_the_pointer_then_the_archive_button_then_the_pointer_goes_back(self):
        got = self.run_scenarios("$o.ok = Scenario { }\n")["ok"]
        self.assertEqual(got["code"], 0, got)
        self.assertEqual(got["calls"], "move 250,220 move 10,20", "onto the row's centre, and back where it was")
        self.assertEqual(got["ptr"], "10,20")
        self.assertIn("invoke", got["order"])
        self.assertTrue(got["sent"], "the point of no return is recorded before the Invoke")
        self.assertNotIn("raise", got["calls"])

    def test_a_covered_row_is_raised_without_focus_and_everything_is_put_back_in_order(self):
        got = self.run_scenarios("$o.c = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::RootAfterRaise = [IntPtr]5 }\n")["c"]
        self.assertEqual(got["code"], 0, got)
        # raise, then the pointer, then (pointer first) the pointer back, the topmost flag off, VS Code directly behind the cover.
        self.assertEqual(got["calls"], "raise move 250,220 move 10,20 notopmost")
        # Order before the raise: wait for idle, check full screen, then the raise with nothing between.
        self.assertEqual(got["order"].split()[1:6], ["wait-idle", "idle-ok", "fs?5", "above", "raise"], "after the scroll settle (sleep300); the window above VS Code is read just before the raise")

    def test_nothing_waits_after_the_raise_and_he_touching_the_pointer_then_undoes_it_at_once(self):
        """Found in review: the driver raised VS Code and then waited for idle again, so a touch right after the raise
        left VS Code over his work until the step deadline."""
        got = self.run_scenarios(
            "$o.touch = Scenario { [PanelNative]::RootUnder = [IntPtr]7; Hook { param($n) if ($n -eq 'raise') { [PanelNative]::Idle = 5 } } }\n"
            "$o.calm = Scenario { [PanelNative]::RootUnder = [IntPtr]7 }\n"
        )
        touch = got["touch"]
        self.assertEqual(touch["code"], 3, touch)
        self.assertIn("operator came back", touch["reason"])
        self.assertEqual(touch["calls"], "raise notopmost", "no pointer move; the raise is undone")
        steps = self.after(touch["order"], "raise")
        self.assertNotIn("wait-idle", steps, "no wait for idle after the raise")
        self.assertFalse([s for s in steps if s.startswith("sleep")], f"nothing sleeps between the raise and its undo: {steps}")
        # And on the calm path only the button poll and the row-gone confirm sleep after the raise.
        sleeps = [s for s in self.after(got["calm"]["order"], "raise") if s.startswith("sleep")]
        self.assertTrue(all(s in ("sleep150", "sleep200") for s in sleeps), sleeps)
        self.assertNotIn("wait-idle", self.after(got["calm"]["order"], "raise"))

    def test_if_he_comes_back_between_the_idle_wait_and_the_raise_nothing_is_raised(self):
        got = self.run_scenarios(
            "$o.back = Scenario { [PanelNative]::RootUnder = [IntPtr]7; $script:Fired = $false; Hook { param($n) if ($n -eq 'fs?5' -and -not $script:Fired) { $script:Fired = $true; [PanelNative]::Idle = 5 } } }\n"
        )["back"]
        self.assertEqual(got["code"], 3, got)
        self.assertEqual(got["calls"], "", "no raise, no pointer move")
        self.assertEqual(got["order"].split()[1:], ["wait-idle", "idle-ok", "fs?5", "wait-idle"], "idle is waited for again instead of raising over him")

    def test_a_row_that_stays_covered_after_the_raise_stops_with_exit_5_and_the_raise_undone(self):
        got = self.run_scenarios("$o.c = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::RootAfterRaise = [IntPtr]7 }\n")["c"]
        self.assertEqual(got["code"], 5)
        self.assertEqual(got["calls"], "raise notopmost", "no pointer move; the raise is undone")
        self.assertIn("nothing was archived", got["reason"])
        self.assertFalse(got["sent"])

    def test_the_raise_is_undone_to_exactly_where_vs_code_was_and_never_behind_an_always_on_top_window(self):
        """Second review item 1: behind an always-on-top window VS Code became always-on-top, and clearing that put it
        above every normal window, his foreground app included. The window that was directly above VS Code is recorded
        before the raise."""
        got = self.run_scenarios(
            "function Cover { [PanelNative]::RootUnder = [IntPtr]7 }\n"
            "$o.above_normal = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]11 }\n"
            "$o.above_topmost = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]11; [void][PanelNative]::TopWins.Add(11) }\n"
            "$o.above_none = Scenario { Cover }\n"
            "$o.gone_fg_normal = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]7; [PanelNative]::Alive = $false }\n"
            "$o.gone_fg_topmost = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]7; [PanelNative]::Alive = $false; [void][PanelNative]::TopWins.Add(9) }\n"
            "$o.gone_fg_is_vscode = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]7; [PanelNative]::Alive = $false; [PanelNative]::Fg = [IntPtr]5 }\n"
            "$o.gone_topmost_above = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]7; [PanelNative]::Alive = $false; [void][PanelNative]::TopWins.Add(7) }\n"
            "$o.last_check = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]11; [PanelNative]::BehindTopmost = $true }\n"
            "$o.stubborn = Scenario { Cover; [PanelNative]::AboveWin = [IntPtr]11; [PanelNative]::BehindTopmost = $true; [PanelNative]::Stubborn = $true }\n"
        )
        done = "raise move 250,220 move 10,20 notopmost"
        self.assertEqual(got["above_normal"]["calls"], done + " behind11", "the window that was above it still exists and is normal: directly behind it, where it was")
        self.assertEqual(got["above_topmost"]["calls"], done, "it was the top normal window: not topmost has just put it back there")
        self.assertEqual(got["above_none"]["calls"], done, "it was at the very top: nothing more")
        self.assertEqual(got["gone_fg_normal"]["calls"], done + " behind9", "the recorded window is gone: behind his foreground window")
        self.assertEqual(got["gone_fg_topmost"]["calls"], done + " bottom", "his foreground window is always-on-top: never behind it, so the back")
        self.assertTrue(any("put at the back" in n for n in got["gone_fg_topmost"]["notes"]), got["gone_fg_topmost"]["notes"])
        self.assertEqual(got["gone_fg_is_vscode"]["calls"], done + " bottom")
        self.assertTrue(any("put at the back" in n for n in got["gone_fg_is_vscode"]["notes"]))
        self.assertEqual(got["gone_topmost_above"]["calls"], done, "it was recorded as always-on-top, so gone or not it was the top normal window")
        for case in ("above_normal", "above_topmost", "above_none", "gone_fg_normal", "gone_topmost_above"):
            self.assertEqual(got[case]["notes"], [], case)
        # The window above VS Code is read before the raise, never after.
        order = got["above_normal"]["order"].split()
        self.assertLess(order.index("above"), order.index("raise"))
        # And a last read of the extended style clears the bit if something left it set; a note says when it stays.
        self.assertEqual(got["last_check"]["calls"], done + " behind11 notopmost")
        self.assertEqual(got["last_check"]["notes"], [])
        self.assertTrue(any("still in the always-on-top band" in n for n in got["stubborn"]["notes"]), got["stubborn"]["notes"])

    def test_the_full_screen_wait_is_logged_once_not_every_two_seconds(self):
        """Second review item 3: Wait-Idle clears the last waiting reason on every loop, so the full-screen reason was
        logged on each round."""
        got = self.run_scenarios(
            "$o.long = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::Quns = 2; $script:N = 0; Hook { param($n) if ($n -eq 'sleep2s') { $script:N++; if ($script:N -ge 4) { [PanelNative]::Quns = 5 } } } }\n"
        )
        self.assertEqual(got["long"]["code"], 0, got["long"])
        self.assertGreaterEqual(got["long"]["order"].split().count("sleep2s"), 4, "it went round the loop four times")
        self.assertEqual(got["long"]["waits"], ["a full-screen window is in front"], "one line for one reason")

    def test_a_full_screen_window_blocks_the_raise_and_a_video_that_starts_during_the_idle_wait_is_seen(self):
        got = self.run_scenarios(
            "$o.wait = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::Quns = 2; Hook { param($n) if ($n -eq 'sleep2s') { [PanelNative]::Quns = 5 } } }\n"
            "$o.video_after_idle = Scenario { [PanelNative]::RootUnder = [IntPtr]7; $script:Fired = $false; Hook { param($n) if ($n -eq 'idle-ok' -and -not $script:Fired) { $script:Fired = $true; [PanelNative]::Quns = 2 } elseif ($n -eq 'sleep2s') { [PanelNative]::Quns = 5 } } }\n"
            "$o.not_covered = Scenario { [PanelNative]::Quns = 2 }\n"
            "$o.other_states = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::Quns = 6 }\n"
        )
        self.assertEqual(got["wait"]["code"], 0, got["wait"])
        self.assertEqual(got["wait"]["waits"], ["a full-screen window is in front"], "one wait, then it raised")
        self.assertEqual(got["wait"]["calls"], "raise move 250,220 move 10,20 notopmost")
        # Review item 3: the check comes after the last idle wait, so a video that started during it blocks the raise.
        order = got["video_after_idle"]["order"].split()
        self.assertEqual(got["video_after_idle"]["code"], 0, got["video_after_idle"])
        self.assertLess(order.index("fs?2"), order.index("sleep2s"))
        self.assertLess(order.index("sleep2s"), order.index("raise"))
        self.assertEqual(order[order.index("raise") - 1], "above", "a read of the z-order, not a wait")
        self.assertEqual(order[order.index("raise") - 2], "fs?5", "the full-screen check is the step right before it")
        self.assertEqual(order[order.index("raise") - 3], "idle-ok", "and the idle wait came just before that")
        self.assertEqual(got["video_after_idle"]["waits"], ["a full-screen window is in front"])
        self.assertEqual(got["not_covered"]["code"], 0, "an uncovered row needs no raise, so the guard does not apply")
        self.assertEqual(got["not_covered"]["calls"], "move 250,220 move 10,20")
        self.assertEqual(got["other_states"]["code"], 0, "states 5, 6 and 7 do not block")

    def test_a_full_screen_window_that_never_leaves_ends_in_exit_3_with_nothing_raised_or_moved(self):
        got = self.run_scenarios(
            # A clock that moves 20 seconds on every look, so the 60 seconds pass in a few looks.
            "$script:Clock = Get-Date\n"
            "function Get-Date { $script:Clock = $script:Clock.AddSeconds(20); $script:Clock }\n"
            "$o.deadline = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::Quns = 3; $script:WaitUntil = $script:Clock.AddSeconds(-1) }\n"
        )["deadline"]
        self.assertEqual(got["code"], 3, got)
        self.assertIn("full-screen window stayed in front", got["reason"])
        self.assertEqual(got["calls"], "", "nothing raised, nothing moved")
        self.assertGreaterEqual(got["sleeps"], 2, "it waited first; with the run's wait already spent it has 60 seconds of its own")

    def test_with_the_wait_budget_already_spent_a_full_screen_window_still_gets_its_own_wait(self):
        """Review minor item: after a tab close the original deadline may have passed; the wait must not exit 3 at once."""
        got = self.run_scenarios(
            "$o.own = Scenario { [PanelNative]::RootUnder = [IntPtr]7; [PanelNative]::Quns = 2; $script:WaitUntil = (Get-Date).AddSeconds(-30); Hook { param($n) if ($n -eq 'sleep2s') { [PanelNative]::Quns = 5 } } }\n"
        )["own"]
        self.assertEqual(got["code"], 0, got)
        self.assertEqual(got["calls"], "raise move 250,220 move 10,20 notopmost")

    def test_a_chat_that_started_running_while_the_driver_waited_is_not_archived(self):
        """Review item 2: busy and running were read once, before the tab close and waits that can last minutes."""
        got = self.run_scenarios(
            "$o.running = Scenario { Hook { param($n) if ($n -eq 'button') { $script:RowStatus = 'running' } } }\n"
            "$o.record = Scenario { Hook { param($n) if ($n -eq 'button') { $script:Rs = [pscustomobject]@{ Count = 1; Busy = ''; Terminal = '' } } } }\n"
            "$o.in_front = Scenario { Hook { param($n) if ($n -eq 'button') { $script:RowActive = $true } } }\n"
            "$o.fine = Scenario { }\n"
        )
        for case, why in (("running", "row says it is running"), ("record", "live session record again"), ("in_front", "the active row")):
            c = got[case]
            self.assertEqual(c["code"], 3, case)
            self.assertIn(why, c["reason"], case)
            self.assertIn("nothing was archived", c["reason"], case)
            self.assertNotIn("invoke", c["order"], f"{case}: the button was never invoked")
            self.assertFalse(c["sent"], case)
            self.assertEqual(c["calls"], "move 250,220 move 10,20", f"{case}: the pointer goes back")
        self.assertEqual(got["fine"]["code"], 0)
        # The re-read is the last step before the Invoke: nothing waits between them.
        order = got["fine"]["order"].split()
        between = order[order.index("button") + 1: order.index("invoke")]
        self.assertEqual([s for s in between if s.startswith("sleep") or s == "wait-idle"], [])

    def test_a_pointer_that_cannot_be_read_back_after_the_move_is_still_put_back(self):
        """Review item 7: the placed position came from the read-back, so a failed read left the pointer moved."""
        got = self.run_scenarios("$o.m = Scenario { Hook { param($n) if ($n -eq 'move') { [PanelNative]::OnStep = $null; [PanelNative]::FailReads = 1 } } }\n")["m"]
        self.assertEqual(got["code"], 0, got)
        self.assertEqual(got["calls"], "move 250,220 move 10,20")
        self.assertEqual(got["ptr"], "10,20")

    def test_no_archive_button_is_exit_5_and_the_pointer_still_goes_back(self):
        got = self.run_scenarios("$o.n = Scenario { $script:ButtonAfter = 2000000000 }\n")["n"]
        self.assertEqual(got["code"], 5)
        self.assertIn("no archive button appeared", got["reason"])
        self.assertEqual(got["calls"], "move 250,220 move 10,20")
        self.assertFalse(got["sent"])

    def test_if_he_moves_the_pointer_nothing_is_archived_and_the_pointer_is_left_where_he_put_it(self):
        got = self.run_scenarios("$o.m = Scenario { Hook { param($n) if ($n -eq 'button') { [PanelNative]::Ptr = [int[]]@(900, 900) } } }\n")["m"]
        self.assertEqual(got["code"], 3)
        self.assertIn("operator came back", got["reason"])
        self.assertIn("nothing was archived", got["reason"])
        self.assertNotIn("invoke", got["order"])
        self.assertEqual(got["calls"], "move 250,220", "not moved back: it is his pointer now")
        self.assertEqual(got["ptr"], "900,900")
        self.assertTrue(any("left it where he put it" in n for n in got["notes"]), got["notes"])

    def test_if_the_idle_time_falls_after_the_move_nothing_is_archived(self):
        got = self.run_scenarios("$o.i = Scenario { Hook { param($n) if ($n -eq 'move') { [PanelNative]::Idle = 40 } } }\n")["i"]
        self.assertEqual(got["code"], 3)
        self.assertIn("idle time fell", got["reason"])
        self.assertNotIn("invoke", got["order"])
        self.assertEqual(got["calls"], "move 250,220 move 10,20", "the pointer is still where the driver put it, so it goes back")

    def test_a_row_that_stays_in_the_list_after_the_invoke_is_exit_1_with_the_pointer_back(self):
        got = self.run_scenarios("$o.k = Scenario { $script:KeepRow = $true }\n")["k"]
        self.assertEqual(got["code"], 1)
        self.assertIn("still in the list", got["reason"])
        self.assertEqual(got["calls"], "move 250,220 move 10,20")
        self.assertTrue(got["sent"])

    def test_a_row_with_no_usable_place_is_refused_before_anything_moves(self):
        got = self.run_scenarios(
            "function Find-RowElement([string]$Id) { $r = Fake-Row; $r.Current.BoundingRectangle = [pscustomobject]@{ X = [double]::PositiveInfinity; Y = [double]::PositiveInfinity; Width = [double]::NegativeInfinity; Height = [double]::NegativeInfinity }; $r }\n"
            "$o.empty = Scenario { }\n"
            "function Find-RowElement([string]$Id) { $r = Fake-Row; $r.Current.IsOffscreen = $true; $r }\n"
            "$o.offscreen = Scenario { }\n"
            "function Find-RowElement([string]$Id) { $r = Fake-Row; $r.Current.BoundingRectangle = [pscustomobject]@{ X = 5000; Y = 200; Width = 300; Height = 40 }; $r }\n"
            "$o.outside = Scenario { }\n"
            "function Find-RowElement([string]$Id) { $r = Fake-Row; $r.Current.BoundingRectangle = [pscustomobject]@{ X = 100; Y = 200; Width = 0; Height = 40 }; $r }\n"
            "$o.zero = Scenario { }\n"
        )
        for case in ("empty", "offscreen", "outside", "zero"):
            self.assertEqual(got[case]["code"], 5, case)
            self.assertEqual(got[case]["calls"], "", case)
            self.assertIn("no usable place on the screen", got[case]["reason"])

    def test_the_row_leaving_the_list_while_getting_ready_is_exit_5(self):
        got = self.run_scenarios("function Find-RowElement([string]$Id) { $null }\n$o.gone = Scenario { }\n")["gone"]
        self.assertEqual((got["code"], got["calls"]), (5, ""))

    # Do-Archive: the order of the guards, when the bridge is needed, the chat in front, and the list reading.

    DO = (
        "$script:Log = @(); $script:Rows = @('id-a'); $script:Rs = [pscustomobject]@{ Count = 0; Busy = ''; Terminal = '' }; $script:RowStatus = 'idle'; $script:ListReadable = $true\n"
        "function Wait-Guards([string]$Id) { $script:Log += 'guards' }\n"
        "function Ensure-Screen { $script:Log += 'screen' }\n"
        "function Find-Named($win, [string]$Name, $Type) { if ($script:ListReadable) { @('button') } else { @() } }\n"
        "function Find-RowElement([string]$Id) { if ($script:Rows -contains $Id) { 'row' } else { $null } }\n"
        "function Wait-NotInFront([string]$Id) { $script:Log += 'front' }\n"
        "function Wait-RowNotActive([string]$Id) { $script:Log += 'notactive' }\n"
        "function Wait-RowIdle([string]$Id) { $script:Log += 'rowidle'; if ($script:RowStatus -eq 'running') { Stop-Panel 3 'the chat is running (its row says so)' }; [pscustomobject]@{ Name = 'a chat title'; Status = $script:RowStatus } }\n"
        "function Get-RecordState([string]$Id) { $script:Rs }\n"
        "function Test-Bridge { $script:Log += 'bridge' }\n"
        "function Set-TabLabel([string]$Label) { $script:Log += 'label' }\n"
        "function Get-TabLabel([string]$Title) { 'lbl' }\n"
        "function Close-Checked([string]$RowId, [string]$RecId, [string]$Mode, [string]$ExpectLabel, [string]$RevealId = '') { $script:Log += ('close ' + $Mode + ' ' + $RowId + ' ' + $ExpectLabel + ' ' + $RevealId) }\n"
        "function Invoke-ArchiveStage([string]$Id) { $script:Log += 'stage' }\n"
        "function Restore-Chat { $script:Log += 'restore-chat' }\n"
        "function Get-StateText { 'STATE' }\n"
        "function Run-Do { $script:Log = @(); $out = [ordered]@{ code = 0; value = $null }\n"
        "  try { $out.value = Do-Archive 'id-a' } catch { $pe = Get-PanelExit $_; $out.code = $pe.Code; $out.reason = $pe.Reason }\n"
        "  $out.log = @($script:Log) -join ' '; [pscustomobject]$out }\n"
    )

    def run_do(self, body: str):
        script = self.NATIVE + self.DO + "$o = [ordered]@{}\n" + body + "ConvertTo-Json -Compress -Depth 5 -InputObject $o\n"
        return run_functions(["Do-Archive", "New-RunState", "Test-Touched", "Invoke-Guarded", "Get-PanelExit"], script)

    def test_do_archive_needs_the_bridge_only_for_a_chat_with_an_open_tab(self):
        got = self.run_do(
            "$o.no_tab = Run-Do\n"
            "$script:Rs = [pscustomobject]@{ Count = 1; Busy = ''; Terminal = '' }; $o.tab = Run-Do\n"
        )
        self.assertEqual(got["no_tab"]["log"], "guards screen front rowidle stage")
        self.assertEqual(got["no_tab"]["value"], "archived session id-a; its row left the list")
        self.assertEqual(got["tab"]["log"], "guards screen front rowidle bridge label close empty id-a lbl id-a notactive stage")
        self.assertIn("its open tab was closed first", got["tab"]["value"])

    def test_the_chat_he_has_in_front_is_waited_for_before_any_tab_or_row_work(self):
        got = self.run_do("$script:Rs = [pscustomobject]@{ Count = 1; Busy = ''; Terminal = '' }; $o.r = Run-Do\n")["r"]
        words = got["log"].split()
        self.assertLess(words.index("front"), words.index("rowidle"))
        self.assertLess(words.index("front"), words.index("bridge"))
        self.assertLess(words.index("front"), words.index("close"))

    def test_no_row_is_exit_0_worded_as_what_is_known_and_the_tab_is_not_touched(self):
        got = self.run_do("$script:Rows = @(); $script:Rs = [pscustomobject]@{ Count = 1; Busy = ''; Terminal = '' }; $o.r = Run-Do\n")["r"]
        self.assertEqual(got["code"], 0)
        self.assertEqual(got["value"], "no row for this chat is in the list as shown (already archived, never listed, or inside a collapsed group); nothing was changed")
        self.assertEqual(got["log"], "guards screen", "no bridge, no close, no stage")

    def test_an_unreadable_sessions_list_is_exit_2_not_a_no_row_answer(self):
        got = self.run_do("$script:ListReadable = $false; $script:Rows = @(); $o.r = Run-Do\n")["r"]
        self.assertEqual(got["code"], 2)
        self.assertIn("sessions list is not readable", got["reason"])
        self.assertIn("nothing was changed", got["reason"])
        self.assertEqual(got["log"], "guards screen restore-chat", "nothing was touched, so the restore has nothing to put back")

    def test_a_running_a_busy_or_a_terminal_chat_is_refused_before_the_tab_or_the_pointer_is_touched(self):
        got = self.run_do(
            "$script:RowStatus = 'running'; $o.running = Run-Do\n"
            "$script:RowStatus = 'idle'; $script:Rs = [pscustomobject]@{ Count = 1; Busy = 'its session record (pid 7) says busy'; Terminal = '' }; $o.busy = Run-Do\n"
            "$script:Rs = [pscustomobject]@{ Count = 1; Busy = ''; Terminal = 'a process outside the VS Code panel holds the chat' }; $o.terminal = Run-Do\n"
        )
        self.assertEqual(got["running"]["code"], 3)
        self.assertEqual(got["busy"]["code"], 3)
        self.assertEqual(got["terminal"]["code"], 5)
        for case in ("running", "busy", "terminal"):
            self.assertNotIn("close", got[case]["log"], case)
            self.assertNotIn("stage", got[case]["log"], case)
            self.assertIn("Nothing was archived.", got[case]["reason"], case)
            self.assertNotIn("Nothing was sent", got[case]["reason"], case)

    # The chat he has in front and the row that stays active after its tab closes.

    LOOPS = (
        "$script:Active = @('id-a'); $script:PrevActive = @('id-a'); $script:Waits = @(); $script:Sleeps = 0\n"
        "function Get-ActiveIds { @($script:Active) }\n"
        "function Stop-IfCancelled { }\nfunction Clear-Waiting { }\n"
        "function Write-Waiting([string]$Reason) { $script:Waits += $Reason }\n"
        "function Get-WaitUntil { $script:WaitUntil }\n"
        "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Sleeps++; if ($null -ne $script:OnSleep) { & $script:OnSleep } }\n"
        "$script:OnSleep = $null\n"
        "function Try-Run([scriptblock]$B) { try { & $B; [pscustomobject]@{ code = 0; reason = '' } } catch { $pe = Get-PanelExit $_; [pscustomobject]@{ code = $pe.Code; reason = $pe.Reason } } }\n"
    )

    def run_loops(self, body: str):
        script = self.NATIVE + self.LOOPS + "$o = [ordered]@{}\n" + body + "ConvertTo-Json -Compress -Depth 5 -InputObject $o\n"
        return run_functions(["Wait-NotInFront", "Wait-RowNotActive", "Get-PanelExit"], script)

    def test_a_chat_he_has_in_front_is_waited_for_then_refused_at_the_deadline(self):
        got = self.run_loops(
            "$script:OnSleep = { if ($script:Sleeps -ge 2) { $script:Active = @('id-b') } }\n"
            "$o.leaves = Try-Run { Wait-NotInFront 'id-a' }\n"
            "$o.prev = @($script:PrevActive); $o.waits = @($script:Waits)\n"
            "$script:Active = @('id-a'); $script:OnSleep = $null; $script:WaitUntil = (Get-Date).AddSeconds(-1); $script:Waits = @()\n"
            "$o.stays = Try-Run { Wait-NotInFront 'id-a' }\n"
            "$o.stays_sleeps = $script:Sleeps\n"
        )
        self.assertEqual(got["leaves"]["code"], 0)
        self.assertEqual(got["prev"], ["id-b"], "what he has in front when the wait ends is what his view is put back to")
        self.assertEqual(got["waits"], ["the chat is the one he has in front"] * 2)
        self.assertEqual(got["stays"]["code"], 3)
        self.assertEqual(got["stays"]["reason"], "it is the chat he has in front; nothing was changed")
        self.assertEqual(got["stays_sleeps"], 2, "it did not sleep again once the deadline had passed")

    def test_the_row_must_stop_being_the_active_row_within_a_few_seconds_of_its_tab_closing(self):
        got = self.run_loops(
            "$script:OnSleep = { $script:Active = @('id-b') }\n"
            "$o.leaves = Try-Run { Wait-RowNotActive 'id-a' }\n"
            "$script:Active = @('id-a'); $script:OnSleep = $null\n"
            "function Get-Date { param($Format) $script:T = $script:T.AddSeconds(3); $script:T }\n"
            "$script:T = [datetime]'2026-11-13T10:00:00'\n"
            "$o.stays = Try-Run { Wait-RowNotActive 'id-a' }\n"
        )
        self.assertEqual(got["leaves"]["code"], 0)
        self.assertEqual(got["stays"]["code"], 3)
        self.assertIn("still the active row after its tab was closed", got["stays"]["reason"])

    # The window: restore without the foreground, always-on-top, and the maximized placement.

    SCREEN = (
        "$script:PrevActive = @('id-b'); $script:ScreenBegun = $true; $script:WasMinimized = $true; $script:SavedPlacement = $null\n"
        "$script:RestoreUntil = (Get-Date).AddMinutes(5); $script:ActiveNow = @('id-b'); $script:Invoked = 0; $script:IdleOnSleep = $null\n"
        "function Get-Chats($Window) { foreach ($id in @('id-a', 'id-b')) { $e = [pscustomobject]@{ Id = $id; Active = ($script:ActiveNow -contains $id); Element = $null }\n"
        "  $e.Element = [pscustomobject]@{ }; $e.Element | Add-Member -MemberType ScriptMethod -Name GetCurrentPattern -Value { param($p) $i = [pscustomobject]@{ }; $i | Add-Member -MemberType ScriptMethod -Name Invoke -Value { $script:Invoked++; $script:ActiveNow = @('id-b') }; $i }; $e } }\n"
        "function Wait-Tree { }\n"
        "function Start-Sleep { param([int]$Seconds = 0, [int]$Milliseconds = 0) $script:Sleeps++; if ($Seconds -gt 0 -and $script:Sleeps -ge 5) { [PanelNative]::Idle = 999999 } }\n"
    )

    def run_screen(self, body: str, names=None):
        script = self.NATIVE + self.SCREEN + "$o = [ordered]@{}\n" + body + "ConvertTo-Json -Compress -Depth 5 -InputObject $o\n"
        return run_functions(["Restore-Screen", "Restore-Minimized", "Set-VsCodeBehind", "Confirm-VsCodeNotTopmost", "Test-VsCodeCovered"] + (names or []), script)

    def test_a_cancelled_run_or_one_with_nothing_to_put_back_does_not_wait_for_him_at_the_end(self):
        """Review item 6: Restore-Screen waited up to 60 seconds for idle even with nothing visible to restore, so a
        cancelled run outlived the cancel verb's 20 seconds."""
        got = self.run_screen(
            "function Reset-Screen { [PanelNative]::Reset(); $script:Sleeps = 0; $script:Notes = @(); $script:ScreenBegun = $true; $script:WasMinimized = $true; $script:SavedPlacement = $null; $script:Cancelled = $false; $script:Invoked = 0 }\n"
            "Reset-Screen; [PanelNative]::Idle = 0; [PanelNative]::RootUnder = [IntPtr]7; $script:ActiveNow = @('id-b'); Restore-Screen\n"
            "$o.nothing_to_restore = [pscustomobject]@{ sleeps = $script:Sleeps; notes = @($script:Notes); calls = [PanelNative]::Log; invoked = $script:Invoked }\n"
            "Reset-Screen; [PanelNative]::Idle = 0; $script:ActiveNow = @('id-a'); $script:Cancelled = $true; Restore-Screen\n"
            "$o.cancelled = [pscustomobject]@{ sleeps = $script:Sleeps; notes = @($script:Notes); calls = [PanelNative]::Log; invoked = $script:Invoked }\n"
            "Reset-Screen; [PanelNative]::Idle = 0; $script:ActiveNow = @('id-a'); Restore-Screen\n"
            "$o.must_restore_waits = [pscustomobject]@{ sleeps = $script:Sleeps; notes = @($script:Notes); invoked = $script:Invoked }\n"
        )
        self.assertEqual(got["nothing_to_restore"]["sleeps"], 0)
        self.assertEqual(got["nothing_to_restore"]["notes"], [])
        self.assertEqual(got["nothing_to_restore"]["invoked"], 0)
        self.assertIn("min7", got["nothing_to_restore"]["calls"], "the window is covered at its centre, so it is minimized again with no idle wait")
        self.assertEqual(got["cancelled"]["sleeps"], 0, "a cancelled run does not wait")
        self.assertTrue(any("not restored" in n for n in got["cancelled"]["notes"]), got["cancelled"]["notes"])
        self.assertEqual(got["cancelled"]["invoked"], 0)
        self.assertGreater(got["must_restore_waits"]["sleeps"], 0, "a normal run still waits for him before it re-activates his chat")
        self.assertEqual(got["must_restore_waits"]["invoked"], 1)

    def test_a_window_that_was_maximized_is_minimized_again_with_its_saved_placement(self):
        got = self.run_screen(
            "[PanelNative]::Reset(); $script:Notes = @(); $script:SavedPlacement = $null; $script:ActiveNow = @('id-b'); $script:Sleeps = 0\n"
            "[PanelNative]::Placement = [int[]]@(2, 2, 0, 0, 0, 0, 100, 100, 900, 700)\n"
            "Restore-Minimized\n"
            "[PanelNative]::Placement = [int[]]@(0, 1, 0, 0, 0, 0, 5, 5, 5, 5)\n"
            "$o.after_restore = [PanelNative]::Log\n"
            "[PanelNative]::Calls.Clear(); Restore-Screen\n"
            "$o.after_end = [PanelNative]::Log; $o.saved_cleared = ($null -eq $script:SavedPlacement)\n"
            "[PanelNative]::Calls.Clear(); $script:ScreenBegun = $true; $script:WasMinimized = $true; $script:SavedPlacement = $null; Restore-Screen\n"
            "$o.no_saved_placement = [PanelNative]::Log\n"
        )
        self.assertEqual(got["after_restore"], "getplacement bottom show4 behind9", "the placement as found is saved before the restore")
        self.assertEqual(got["after_end"], "minplace flags=2 rect=100,100", "WPF_RESTORETOMAXIMIZED and the normal rectangle are kept")
        self.assertTrue(got["saved_cleared"])
        self.assertEqual(got["no_saved_placement"], "min7")

    def test_the_window_is_minimized_again_only_when_he_is_away_or_it_is_covered_and_not_his_foreground(self):
        """Second review item 2: the only guard was 'not the foreground window', so a VS Code he brought up himself and left
        visible behind another app was minimized under him."""
        got = self.run_screen(
            "function Case([double]$Idle, [int]$Fg, [int]$Root, [bool]$Was = $true) {\n"
            "  [PanelNative]::Reset(); $script:Notes = @(); $script:ScreenBegun = $true; $script:WasMinimized = $Was; $script:ActiveNow = @('id-b'); $script:Sleeps = 0\n"
            "  $script:SavedPlacement = [int[]]@(2, 2, 0, 0, 0, 0, 1, 2, 3, 4); [PanelNative]::Idle = $Idle; [PanelNative]::Fg = [IntPtr]$Fg; [PanelNative]::RootUnder = [IntPtr]$Root\n"
            "  Restore-Screen\n"
            "  [pscustomobject]@{ calls = [PanelNative]::Log; notes = @($script:Notes) } }\n"
            "$o.idle_vscode_foreground = Case 999999 5 5\n"
            "$o.idle_other_foreground = Case 999999 9 5\n"
            "$o.busy_covered_other_foreground = Case 0 9 7\n"
            "$o.busy_visible_other_foreground = Case 0 9 5\n"
            "$o.busy_vscode_foreground = Case 0 5 7\n"
            "$o.not_restored_in_this_run = Case 999999 9 7 $false\n"
        )
        mini = "minplace flags=2 rect=1,2"
        left = "VS Code was left open: it is visible on his screen and he is at the computer"
        self.assertEqual(got["idle_vscode_foreground"]["calls"], mini, "he is away: minimized whatever is in front")
        self.assertEqual(got["idle_other_foreground"]["calls"], mini)
        self.assertEqual(got["busy_covered_other_foreground"]["calls"], mini, "he is at the computer but VS Code is behind another window and not his foreground")
        for case in ("idle_vscode_foreground", "idle_other_foreground", "busy_covered_other_foreground"):
            self.assertEqual(got[case]["notes"], [], case)
        for case in ("busy_visible_other_foreground", "busy_vscode_foreground"):
            self.assertEqual(got[case]["calls"], "", f"{case}: he can see it and is using the computer, so it is left restored")
            self.assertEqual(got[case]["notes"], [left], case)
        self.assertEqual(got["not_restored_in_this_run"], {"calls": "", "notes": []}, "a window the driver did not restore is never minimized")

    def test_the_placement_is_saved_once_and_the_first_one_wins(self):
        got = self.run_screen(
            "[PanelNative]::Reset(); $script:SavedPlacement = $null\n"
            "[PanelNative]::Placement = [int[]]@(2, 2, 0, 0, 0, 0, 100, 100, 900, 700); Restore-Minimized\n"
            "[PanelNative]::Placement = [int[]]@(0, 2, 0, 0, 0, 0, 7, 7, 7, 7); Restore-Minimized\n"
            "$o.flags = $script:SavedPlacement[0]; $o.left = $script:SavedPlacement[6]\n"
        )
        self.assertEqual((got["flags"], got["left"]), (2, 100))

    def test_the_restore_never_places_vs_code_behind_an_always_on_top_window(self):
        """Second review item 1c: behind his always-on-top foreground window VS Code would be always-on-top; it stays at the bottom."""
        got = self.run_screen(
            "function Fresh { [PanelNative]::Reset(); $script:Notes = @(); $script:SavedPlacement = $null }\n"
            "Fresh; Restore-Minimized; $o.normal_fg = [pscustomobject]@{ calls = [PanelNative]::Log; notes = @($script:Notes) }\n"
            "Fresh; [void][PanelNative]::TopWins.Add(9); Restore-Minimized; $o.topmost_fg = [pscustomobject]@{ calls = [PanelNative]::Log; notes = @($script:Notes) }\n"
            "Fresh; [PanelNative]::Fg = [IntPtr]::Zero; Restore-Minimized; $o.no_fg = [PanelNative]::Log\n"
            "Fresh; [PanelNative]::Fg = [IntPtr]5; Restore-Minimized; $o.fg_is_vscode = [PanelNative]::Log\n"
            "Fresh; [PanelNative]::Alive = $true; [PanelNative]::BehindTopmost = $true; Restore-Minimized; $o.last_check = [pscustomobject]@{ calls = [PanelNative]::Log; notes = @($script:Notes) }\n"
            "Fresh; [PanelNative]::BehindTopmost = $true; [PanelNative]::Stubborn = $true; Restore-Minimized; $o.stubborn = @($script:Notes)\n"
        )
        self.assertEqual(got["normal_fg"]["calls"], "getplacement bottom show4 behind9")
        self.assertEqual(got["topmost_fg"]["calls"], "getplacement bottom show4", "his foreground window is always-on-top: VS Code stays at the bottom, where HWND_BOTTOM put it")
        self.assertEqual(got["topmost_fg"]["notes"], [])
        self.assertEqual(got["no_fg"], "getplacement bottom show4")
        self.assertEqual(got["fg_is_vscode"], "getplacement bottom show4")
        self.assertEqual(got["last_check"]["calls"], "getplacement bottom show4 behind9 notopmost", "the topmost bit is read at the end and cleared")
        self.assertEqual(got["last_check"]["notes"], [])
        self.assertTrue(any("always-on-top band" in n for n in got["stubborn"]), got["stubborn"])

    # What a failed archive says, in its own words, and the tab it puts back.

    def archive_failure(self, setup: str, stop: str, row: str = "'row'") -> dict:
        body = (
            WORLD
            + "$script:Restored = $false\n"
            f"function Find-RowElement([string]$Id) {{ {row} }}\n"
            "New-RunState 'id-a' ''; $s = $script:St; $s.Archive = $true; $s.Label = 'chat-a'\n"
            + setup
            + "$out = [ordered]@{}\n"
            f"try {{ Invoke-Guarded {{ {stop} }} 'Nothing was archived.' '' }} catch {{ $pe = Get-PanelExit $_; $out.code = $pe.Code; $out.reason = $pe.Reason }}\n"
            "$out.log = @($W.Log); $out.notes = @($script:Notes); $out.tab_open = $W.TabOpen\n"
            "ConvertTo-Json -Compress -Depth 4 -InputObject $out\n"
        )
        return run_functions(WORLD_FUNCTIONS + ["Get-ArchiveStateText"], body)

    def test_a_failure_before_anything_changed_says_the_panel_was_not_changed(self):
        got = self.archive_failure("$W.TabOpen = $true\n", "Stop-Panel 3 'it is the chat he has in front; nothing was changed'")
        self.assertEqual(got["code"], 3)
        self.assertEqual(got["reason"], "it is the chat he has in front; nothing was changed. Nothing was archived. The panel was not changed: the chat's tab is as it was.")
        self.assertNotIn("Send", got["reason"])

    def test_a_failure_after_the_tab_close_reopens_the_tab_and_says_so(self):
        """Review item 5: a failure after the tab close and before the archive must reopen the tab as send does."""
        got = self.archive_failure(
            "$s.HadTab = $true; $s.Closed = $true; $s.CloseAsked = $true; $W.TabOpen = $false\n",
            "Stop-Panel 3 'the chat started running while the driver waited'",
        )
        self.assertEqual(got["code"], 3)
        self.assertTrue(got["tab_open"])
        self.assertIn("reveal", got["log"])
        self.assertIn("The driver closed the chat's tab, then opened the chat's tab again.", got["reason"])
        self.assertIn("Nothing was archived.", got["reason"])
        self.assertIn("The chat's tab is now open", got["reason"])
        self.assertNotIn("pressed Send", got["reason"])

    def test_a_tab_that_could_not_be_reopened_is_reported_with_the_reveal_command(self):
        got = self.archive_failure(
            "$s.HadTab = $true; $s.Closed = $true; $s.CloseAsked = $true; $W.TabOpen = $false; $W.RevealWorks = $false\n",
            "Stop-Panel 3 'the operator kept using the computer'",
        )
        self.assertFalse(got["tab_open"])
        self.assertIn("The chat's tab is now closed. It was open before the run.", got["reason"])
        self.assertNotIn("opened the chat's tab again", got["reason"])
        self.assertTrue(any("did not come back after reveal" in n and "panel_bridge.py reveal" in n for n in got["notes"]), got["notes"])

    def test_after_the_archive_button_was_invoked_the_message_says_what_the_list_shows(self):
        listed = self.archive_failure("$s.HadTab = $true; $s.Closed = $true; $s.CloseAsked = $true; $s.Sent = $true; $W.TabOpen = $false\n", "Stop-Panel 1 'the Archive button was invoked but the chat''s row is still in the list after 5 seconds'")
        gone = self.archive_failure("$s.Sent = $true\n", "Stop-Panel 1 'x'", row="$null")
        unknown = self.archive_failure("$s.Sent = $true\n", "Stop-Panel 1 'x'", row="throw 'cannot read'")
        for got in (listed, gone, unknown):
            self.assertEqual(got["code"], 1)
            self.assertNotIn("pressed Send", got["reason"])
            self.assertNotIn("The panel was not changed", got["reason"])
            self.assertNotIn("reveal", got["log"], "nothing is put back after the button was invoked")
        self.assertIn("The driver closed the chat's tab, then invoked the row's Archive button. The chat's row is still in the list, so the chat is not known to be archived.", listed["reason"])
        self.assertIn("The chat's row is no longer in the list: it is archived.", gone["reason"])
        self.assertIn("whether the chat is archived is unknown", unknown["reason"])
        self.assertNotIn("Nothing was archived.", listed["reason"])

    def test_the_archive_run_is_one_logged_locked_action_with_the_driver_header_amended(self):
        text = source()
        self.assertRegex(text, r"Invoke-Logged 'archive' \$Session \$true \{ Do-Archive \$Session \}")
        header = " ".join(text.split("param(")[0].replace("# ", "").split())
        for part in ["'archive' moves the pointer onto", "nothing is clicked or typed", "never takes the foreground", "behind the window he has in front",
                     "waits while a", "full-screen window is in front", "exit 7", "A chat he has in front is never archived",
                     "after VS Code is raised nothing waits except the 3 second button poll and the 5 second row-gone confirm",
                     "3 the operator is active or came back", "5 a safety check failed (a record from outside the panel, the row covered by another window or with no usable place, no Archive button)",
                     "saved placement"]:
            self.assertIn(part, header, part)
        self.assertNotIn("no Archive button, the operator came back", header, "the operator coming back is exit 3, not 5")
        self.assertEqual(code_lines(text).count("Archive session"), 1, "the one control archive names")
        archive = function_text("Do-Archive")
        self.assertLess(archive.index("Wait-Guards"), archive.index("Ensure-Screen"))
        self.assertLess(archive.index("Test-Bridge"), archive.index("Close-Checked"))
        self.assertLess(archive.index("Wait-NotInFront"), archive.index("Wait-RowIdle"))
        self.assertLess(archive.index("Close-Checked"), archive.index("Wait-RowNotActive"))
        self.assertLess(archive.index("Wait-RowNotActive"), archive.index("Invoke-ArchiveStage"))
        stage = function_text("Invoke-ArchiveStage")
        for part in ["ScrollIntoView", "RaiseNoActivate", "finally", "Restore-Pointer $old $placed", "Undo-Raise $front", "Get-OperatorBack $placed $idleBefore", "Get-ArchiveBlock $Id"]:
            self.assertIn(part, stage)
        # Review item 1 and 3, as text: one idle wait, in the loop, then the full-screen check, the idle re-check and the raise.
        self.assertEqual(stage.count("Wait-Idle"), 1)
        self.assertLess(stage.index("Wait-Idle"), stage.index("Test-FullScreenFront"))
        self.assertLess(stage.index("Test-FullScreenFront"), stage.index("IdleMs() -lt"))
        self.assertLess(stage.index("IdleMs() -lt"), stage.index("RaiseNoActivate"))
        between = stage[stage.index("IdleMs() -lt"):stage.index("RaiseNoActivate")]
        for waiting in ["Start-Sleep", "Wait-", "Find-RowElement", "Get-Chats"]:
            self.assertNotIn(waiting, between, f"{waiting} between the last idle check and the raise")
        after_raise = stage[stage.index("RaiseNoActivate"):]
        self.assertNotIn("Wait-Idle", after_raise)
        self.assertEqual(after_raise.count("Start-Sleep"), 2, "only the 3 second button poll and the 5 second row-gone confirm sleep after the raise")
        self.assertLess(stage.index("Get-ArchiveBlock"), stage.index(".Invoke()"))
        self.assertLess(stage.index("Get-OperatorBack"), stage.index(".Invoke()"))
        between = stage[stage.index("Get-ArchiveBlock $Id"):stage.index(".Invoke()")]
        for waiting in ["Start-Sleep", "Wait-"]:
            self.assertNotIn(waiting, between, f"{waiting} between the last re-read and the Invoke")
        self.assertLess(stage.index("Restore-Pointer"), stage.index("Undo-Raise"), "the pointer goes back before the raise is undone")
        self.assertEqual(code_lines(text).count("RaiseNoActivate("), 2, "its definition and one raise, inside the archive stage")
        self.assertEqual(code_lines(text).count("MovePointer("), 3, "its definition, and two calls: onto the row, and back")
        self.assertIn("GetParent($btn)", function_text("Get-ArchiveButton"))
        self.assertIn("-ne ($RowPrefix + $Id)", function_text("Get-ArchiveButton"), "the button must belong to the target row")
        self.assertNotIn("Wait-NoFullScreen", code_lines(text))


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class SendNowBehaviourTests(unittest.TestCase):
    """Send-Now run against scripted read-backs: it invokes the very Send it verified, and nothing when the
    input, Send or the row changed after the read-back."""

    PRE = (
        MOCK_NATIVE
        + "Add-Type -AssemblyName UIAutomationClient\n"
        "$script:St = [pscustomobject]@{ Sent = $false }\n"
        "$global:Log = @(); $global:Value = 'hello'; $global:RowActive = $true\n"
        "function NewSend([string]$Id, [bool]$Enabled = $true) {\n"
        "  $o = [pscustomobject]@{ Id = $Id; Current = [pscustomobject]@{ IsEnabled = $Enabled; IsOffscreen = $false } }\n"
        "  $o | Add-Member -MemberType ScriptMethod -Name GetCurrentPattern -Value { param($p) $inv = [pscustomobject]@{ Id = $this.Id }; $inv | Add-Member -MemberType ScriptMethod -Name Invoke -Value { $global:Log += ('invoke ' + $this.Id) }; $inv }\n"
        "  $o\n"
        "}\n"
        "function Ready([string]$Id, [bool]$Enabled = $true) { [pscustomobject]@{ Why = $null; Pair = [pscustomobject]@{ Input = 'in'; Send = (NewSend $Id $Enabled) }; Row = [pscustomobject]@{ Element = 'row' } } }\n"
        "function Wait-Idle([string]$What) { $global:Log += 'wait' }\n"
        "function Test-ReadyQueue { }\n"
        "function Test-Ready([string]$RowId, [string]$Text) { $global:Log += 'ready'; $r = $global:Ready[0]; $global:Ready.RemoveAt(0); [PanelNative]::Idle = $global:Idles[0]; $global:Idles.RemoveAt(0); if ($null -ne $global:AfterRead) { & $global:AfterRead }; $r }\n"
        "function Get-InputValue($El) { $global:Value }\n"
        "function Test-RowActive($Row) { $global:RowActive }\n"
        "$global:AfterRead = $null\n"
    )

    def send(self, setup: str) -> dict:
        body = (
            self.PRE
            + setup
            + "$out = [ordered]@{ code = 0; sent = $false }\n"
            "try { $p = Send-Now 'id-a' 'hello'; $out.invoked_pair = $p.Send.Id } catch { $out.code = (Get-PanelExit $_).Code }\n"
            "$out.sent = $script:St.Sent; $out.log = @($global:Log) -join ' '\n"
            "ConvertTo-Json -Compress -InputObject $out\n"
        )
        return run_functions(["Send-Now", "Test-SameText", "ConvertTo-PlainValue", "Get-PanelExit"], body)

    def test_it_invokes_the_send_it_verified(self):
        got = self.send("$global:Ready = [System.Collections.ArrayList]@((Ready 'send-1')); $global:Idles = [System.Collections.ArrayList]@(999999)\n")
        self.assertEqual(got, {"code": 0, "sent": True, "invoked_pair": "send-1", "log": "wait ready invoke send-1"})

    def test_a_change_after_the_read_back_stops_it_with_nothing_invoked(self):
        cases = {
            "input": "$global:AfterRead = { $global:Value = 'hello and his words' }\n",
            "send_disabled": "$global:Ready = [System.Collections.ArrayList]@((Ready 'send-1' $false))\n",
            "row": "$global:AfterRead = { $global:RowActive = $false }\n",
        }
        for name, extra in cases.items():
            got = self.send("$global:Ready = [System.Collections.ArrayList]@((Ready 'send-1')); $global:Idles = [System.Collections.ArrayList]@(999999)\n" + extra)
            self.assertEqual((got["code"], got["sent"], got["log"]), (5, False, "wait ready"), name)

    def test_when_he_moves_after_the_read_back_it_reads_back_again_and_invokes_the_new_one(self):
        got = self.send("$global:Ready = [System.Collections.ArrayList]@((Ready 'send-1'), (Ready 'send-2')); $global:Idles = [System.Collections.ArrayList]@(0, 999999)\n")
        self.assertEqual(got, {"code": 0, "sent": True, "invoked_pair": "send-2", "log": "wait ready wait ready invoke send-2"})

    def test_a_failed_read_back_sends_nothing(self):
        got = self.send("$r = Ready 'send-1'; $r.Why = 'the input does not hold the text'; $global:Ready = [System.Collections.ArrayList]@($r); $global:Idles = [System.Collections.ArrayList]@(999999)\n")
        self.assertEqual((got["code"], got["sent"], got["log"]), (5, False, "wait ready"))


class SecondReviewPairingTests(unittest.TestCase):
    def test_the_bridge_close_rechecks_the_active_tab_and_the_driver_still_works_with_0_1_0(self):
        text = EXTENSION.read_text(encoding="ascii")
        close = re.search(r"'close-tab': async function.*?\n  \}", text, re.DOTALL).group(0)
        self.assertIn("if (args.requireActive !== false)", close)
        self.assertLess(close.index("isActive"), close.index("tabGroups.close"))
        tab = function_text("Close-TabByLabel")
        self.assertIn("all = $false", tab, "0.1.0 closes every match without it")
        self.assertIn("not the active tab", tab)

    def test_a_bridge_timeout_is_followed_by_a_read_of_the_tabs(self):
        tab = function_text("Close-TabByLabel")
        self.assertLess(tab.index("$s.CloseAsked = $true"), tab.index("Invoke-Bridge 'close-tab'"), "the attempt is recorded first")
        self.assertIn("if ($null -eq $r)", tab)
        self.assertIn("is unknown", tab)
        opened = function_text("Open-WithText")
        self.assertLess(opened.index("$s.OpenCalled = $true"), opened.index("Invoke-Bridge 'open'"))
        self.assertIn("did not answer open", opened)

    def test_a_post_send_close_needs_the_drivers_own_tab(self):
        send = function_text("Do-Send")
        tail = send[send.index("# The text is sent and confirmed"):]
        self.assertIn("if (-not $s.HadTab -and $s.Opened)", tail)
        self.assertIn("Close-Checked $Id $Id 'empty' '' $Id $false $s.OwnPid", tail)
