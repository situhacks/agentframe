"""Static checks for system/tools/claude_panel.ps1, the driver that types into the Claude panel of
the VS Code window.

Nothing here runs the driver, opens a window or sends a key. The script is parsed with the Windows
PowerShell 5.1 parser and read as text; three of its pure functions (the window title rule, the typed
text rule, the chunking) are cut out of the parse tree and run on their own in a child PowerShell.
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
DRIVER = ROOT / "system" / "tools" / "claude_panel.ps1"
POWERSHELL = sys.platform == "win32" and shutil.which("powershell.exe")

# The controls the driver may name, and nothing else.
CONTROLS = {"Message input", "Send message", "Switch model", "New session"}

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
    """The script's PowerShell code: no comment lines, and not the C# in the Add-Type here-string, which csc compiles."""
    return code_lines(re.sub(r"@'\n.*?\n'@", "", text, flags=re.DOTALL))


def run_powershell(script: str) -> subprocess.CompletedProcess:
    with tempfile.TemporaryDirectory() as d:
        path = Path(d) / "check.ps1"
        path.write_text(script, encoding="ascii")
        return subprocess.run(
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", str(path)],
            capture_output=True, text=True, timeout=120,
        )


def run_functions(names: list[str], body: str):
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
    proc = run_powershell(script)
    if proc.returncode != 0:
        raise AssertionError(f"the child PowerShell failed: {proc.stderr.strip()}")
    return json.loads(proc.stdout)


@unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
class ParseTests(unittest.TestCase):
    def test_the_driver_parses_in_windows_powershell(self):
        proc = run_powershell(
            "$t = $null; $e = $null\n"
            f"[void][System.Management.Automation.Language.Parser]::ParseFile('{DRIVER}', [ref]$t, [ref]$e)\n"
            "$e | ForEach-Object { 'line ' + $_.Extent.StartLineNumber + ': ' + $_.Message }\n"
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.strip(), "", f"parse errors: {proc.stdout}")


class TextTests(unittest.TestCase):
    def test_the_driver_is_ascii_so_windows_powershell_reads_it_right(self):
        bad = [i for i, b in enumerate(DRIVER.read_bytes()) if b > 127]
        self.assertEqual(bad, [], "non-ASCII bytes: 5.1 reads a BOM-less file as the ANSI code page")

    def test_no_powershell_7_only_syntax(self):
        text = powershell_only(source())
        self.assertIn("function Find-Window", text)
        self.assertNotIn("SendInput", text, "the C# block is cut out, so the check cannot see it")
        self.assertEqual([label for pat, label in PS7_ONLY if pat.search(text)], [])

    def test_it_has_no_click_or_mouse_action_and_no_pointer_input_at_all(self):
        text = source()
        for word in ["mouse", "click", "cursor", "MOUSEINPUT", "INPUT_MOUSE", "mouse_event", "SetCursorPos", "ClickablePoint", "SendKeys", "Clipboard"]:
            self.assertNotIn(word.lower(), text.lower(), f"the driver must not mention {word!r}")
        actions = re.findall(r"^    '(\w+)'\s*\{", text, re.MULTILINE)
        self.assertEqual(actions, ["list", "send", "new", "model", "selftest"])

    def test_it_runs_nothing_else_and_never_sets_the_input_text_through_a_pattern(self):
        text = code_lines(source())
        for banned in ["Invoke-Expression", "iex ", "Start-Process", "Stop-Process", "Invoke-WebRequest", "SetValue", "Set-Clipboard"]:
            self.assertNotIn(banned, text, f"{banned!r} is not for this driver")

    def test_text_arrives_only_through_a_file_never_as_an_argument(self):
        text = source()
        params = re.search(r"param\((.*?)\n\)", text, re.DOTALL).group(1)
        names = re.findall(r"\$(\w+)", params)
        self.assertEqual(names, ["Action", "Session", "TextFile", "Model", "IdleSeconds", "WaitMinutes"])
        self.assertIn("[System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)", text)


class ControlTests(unittest.TestCase):
    def test_the_only_controls_named_are_the_ones_the_probes_proved(self):
        text = source()
        named = set(re.findall(r"Find-Named\s+\S+\s+'([^']+)'", text))
        self.assertEqual(named, CONTROLS)
        # The session rows, their dots and the active class, and the model options.
        self.assertIn("'sessions-list-row-'", text)
        for part in ["*statusDot*", "*Running*", "*Idle*", "(^|\\s)active_", "*modelItem_*", "*activeModelItem_*"]:
            self.assertIn(part, text)
        for other in ["New group", "Show command menu", "commandItem", "Permissions", "Effort", "Search sessions", "Message actions"]:
            self.assertNotIn(other, text, f"{other!r} is a control the driver must not touch")

    def test_it_never_invokes_a_command_palette_item_and_says_why(self):
        text = source()
        self.assertIn("a palette item is never invoked", text)
        invoked = re.findall(r"(\$\w+(?:\[\d+\])?)\.Invoke\(\)", text)
        self.assertTrue(invoked, "the driver invokes the New session button, a chat row and a model option")
        self.assertEqual(len(re.findall(r"InvokePattern", text)), 5)


class WindowRuleTests(unittest.TestCase):
    def test_every_window_lookup_goes_through_the_one_function_that_applies_the_title_rule(self):
        text = code_lines(source())
        self.assertEqual(text.count("RootElement"), 1, "the desktop is searched in one place only")
        self.assertEqual(text.count("Get-Process"), 1)
        find = re.search(r"function Find-Window \{(.*?)\n\}\n", text, re.DOTALL).group(1)
        for part in ["RootElement", "Test-VaultTitle", "'Chrome_WidgetWin_1'", "ProcessName -ne 'Code'", "Stop-Panel 2 \"no VS Code window", "more than one VS Code window"]:
            self.assertIn(part, find)
        self.assertIn("function Open-Window", text)
        for action in ["Invoke-Send", "Invoke-New", "Invoke-Model", "Invoke-List"]:
            body = re.search(rf"function {action}\b.*?\n\}}\n", text, re.DOTALL).group(0)
            self.assertIn("Open-Window", body, f"{action} must start by finding the vault's window")

    def test_the_foreground_window_is_checked_against_the_title_rule_before_any_key(self):
        text = source()
        ready = re.search(r"function Assert-Ready.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("$fg -ne $script:Hwnd", ready)
        self.assertIn("Test-VaultTitle ([PanelNative]::Title($fg)) (Get-VaultName)", ready)
        self.assertIn("'Message input'", ready)

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

    def test_the_worktree_folder_resolves_to_the_vault_name(self):
        text = source()
        self.assertIn(r"'\\\.claude\\worktrees\\[^\\]+$'", text)


class TypingTests(unittest.TestCase):
    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_a_leading_slash_gets_exactly_one_leading_space_and_the_text_is_otherwise_kept(self):
        cases = [
            ["/compact keep it", " /compact keep it"],
            [" /compact keep it", " /compact keep it"],
            ["/rename a \"b\" c", " /rename a \"b\" c"],
            ["plain text", "plain text"],
            ["no slash / inside", "no slash / inside"],
            ["line one\r\nline two\r\n\r\n", "line one\nline two"],
            ["tab\there", "tab here"],
            ["\ufeff/status", " /status"],
        ]
        got = run_functions(
            ["Format-TypedText"],
            "$cases = " + json.dumps([c[0] for c in cases]).replace("'", "''").join(["'", "'"]) + " | ConvertFrom-Json\n"
            "$out = @(foreach ($c in $cases) { Format-TypedText $c })\n"
            "ConvertTo-Json -InputObject $out -Compress\n",
        )
        self.assertEqual(got, [c[1] for c in cases])

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_long_text_is_cut_into_batches_that_rejoin_and_never_split_a_surrogate_pair(self):
        got = run_functions(
            ["Split-Text"],
            "$ChunkChars = 400\n"
            "$plain = 'a' * 1000\n"
            "$pairs = ([string][char]0xD83D + [char]0xDE00) * 450\n"
            "$a = @(Split-Text $plain)\n"
            "$b = @(Split-Text $pairs)\n"
            "$ok = $true\n"
            "foreach ($c in $b) { if ([char]::IsLowSurrogate($c[0]) -or [char]::IsHighSurrogate($c[$c.Length - 1])) { $ok = $false } }\n"
            "ConvertTo-Json -InputObject @{ plain = @($a | ForEach-Object { $_.Length }); rejoined = (($a -join '') -eq $plain); pairs_ok = $ok; pairs_rejoined = (($b -join '') -eq $pairs) } -Compress\n",
        )
        self.assertEqual(got["plain"], [400, 400, 200])
        self.assertTrue(got["rejoined"] and got["pairs_ok"] and got["pairs_rejoined"])

    def test_keys_go_out_only_as_unicode_batches_and_enter_and_a_line_break_is_shift_enter(self):
        text = source()
        self.assertEqual(text.count("SendInput(("), 1, "one call of the native SendInput")
        for part in ["TypeBatch", "public static uint Enter()", "Key(0x10, false)", "KEYBDINPUT", "dwFlags = up ? 6u : 4u"]:
            self.assertIn(part, text)
        self.assertEqual(len(re.findall(r"\[PanelNative\]::TypeBatch\(", text)), 1)
        self.assertEqual(len(re.findall(r"\[PanelNative\]::Enter\(", text)), 1)

    def test_a_long_text_is_typed_in_guarded_batches(self):
        text = source()
        loop = re.search(r"foreach \(\$chunk in \(Split-Text \$rest\)\) \{(.*?)\n    \}\n", text, re.DOTALL).group(1)
        self.assertIn("Send-Chunk $chunk", loop)
        chunk = re.search(r"function Send-Chunk.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertLess(chunk.index("Assert-Ready"), chunk.index("TypeBatch"))
        self.assertIn("Save-OwnTick", chunk)


class CanaryTests(unittest.TestCase):
    """The typing path proves the keys reach the message input before it types anything real."""

    def send_text(self) -> str:
        return re.search(r"function Send-Text.*?\n\}\n", source(), re.DOTALL).group(0)

    def test_a_single_space_canary_is_typed_and_read_back_before_any_real_text(self):
        text = source()
        canary = re.search(r"function Send-Canary.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("Send-Chunk ' '", canary)
        self.assertIn("Read-InputValue", canary)
        self.assertIn("Stop-Panel 5", canary)
        self.assertIn("Get-TypingFacts", canary)
        send = self.send_text()
        self.assertLess(send.index("Set-InputFocus"), send.index("Send-Canary"))
        self.assertLess(send.index("Send-Canary"), send.index("Split-Text $rest"))

    def test_the_canary_must_show_as_exactly_one_space_or_nothing_more_is_typed(self):
        canary = re.search(r"function Send-Canary.*?\n\}\n", source(), re.DOTALL).group(0)
        self.assertIn("-ceq ' '", canary)
        self.assertIn("-ceq ($base + ' ')", canary)
        self.assertIn("nothing more was typed", canary)

    def test_the_canary_is_the_leading_space_or_is_taken_back_and_verified(self):
        send = self.send_text()
        self.assertIn("$Text.StartsWith(' ')", send)
        self.assertIn("$rest = $Text.Substring(1)", send)
        self.assertEqual(len(re.findall(r"\[PanelNative\]::Backspace\(", source())), 1)
        self.assertLess(send.index("Backspace"), send.index("Wait-InputValue"))

    def test_enter_waits_for_a_read_back_that_equals_the_intended_text(self):
        send = self.send_text()
        enter = send.index("[PanelNative]::Enter()")
        self.assertIn("-ceq $want", send[:enter])
        self.assertLess(send.index("$got = Wait-InputValue"), send.index("Assert-Ready $Target 'typed'"))
        self.assertLess(send.index("Assert-Ready $Target 'typed'"), send.index("$last = Read-InputValue"))
        self.assertLess(send.index("$last = Read-InputValue"), enter)
        self.assertEqual(send.count("[PanelNative]::Enter()"), 1)
        # every refusal before Enter clears only what the driver typed, then stops without pressing anything
        refusals = re.findall(r"\$note = Clear-OwnText\n\s+Stop-Panel (\d)", send)
        self.assertGreaterEqual(len(refusals), 4)

    def test_the_driver_clears_only_with_the_focus_verified_on_the_message_input(self):
        text = source()
        clear = re.search(r"function Clear-OwnText.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertLess(clear.index("Test-FocusIsInput"), clear.index("ClearInput"))
        self.assertEqual(len(re.findall(r"\[PanelNative\]::ClearInput\(", text)), 1)
        focus = re.search(r"function Test-FocusIsInput.*?\n\}\n", text, re.DOTALL).group(0)
        for part in ["$script:Hwnd", "'Message input'", "$CT::Edit", "messageInput_*", "ProcessId"]:
            self.assertIn(part, focus)

    def test_when_another_window_was_in_front_the_page_is_given_time_and_a_focus_change_before_the_canary(self):
        text = source()
        focus = re.search(r"function Set-InputFocus.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("$script:Saved.Fg -ne $script:Hwnd", focus)
        self.assertIn("Find-Named $win 'Switch model'", focus)
        self.assertLess(focus.index("Start-Sleep -Milliseconds 1200"), focus.index("$boxes[0].SetFocus()"))
        # the focus change never types: no key leaves this function
        self.assertNotIn("TypeBatch", focus)
        self.assertNotIn("[PanelNative]::Enter", focus)

    def test_a_failed_canary_reports_what_had_the_focus(self):
        facts = re.search(r"function Get-TypingFacts.*?\n\}\n", source(), re.DOTALL).group(0)
        for part in ["ControlType.ProgrammaticName", "Current.Name", "Current.ClassName", "ProcessNameOf", "KeyboardFocusFacts"]:
            self.assertIn(part, facts)

    @unittest.skipUnless(POWERSHELL, "needs Windows PowerShell")
    def test_the_read_back_comparison_ignores_only_what_the_page_adds(self):
        cases = [
            ["a b", "a b"],
            ["a b", "a b"],
            ["line\r\nline\r\n", "line\nline"],
            ["x​y", "xy"],
            ["A", "A"],
        ]
        got = run_functions(
            ["ConvertTo-PlainValue"],
            "$cases = " + json.dumps([c[0] for c in cases]).replace("'", "''").join(["'", "'"]) + " | ConvertFrom-Json\n"
            "$out = @(foreach ($c in $cases) { ConvertTo-PlainValue $c })\n"
            "ConvertTo-Json -InputObject $out -Compress\n",
        )
        self.assertEqual(got, [c[1] for c in cases])


class GuardTests(unittest.TestCase):
    def test_defaults_and_exit_codes(self):
        text = source()
        self.assertIn("[int]$IdleSeconds = 120", text)
        self.assertIn("[double]$WaitMinutes = 20", text)
        ready = re.search(r"function Assert-Ready.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("Stop-Panel 3", ready)
        self.assertIn("Stop-Panel 4", ready)
        self.assertIn("Stop-Panel 5", ready)
        self.assertIn("Stop-Panel 3 \"gave up after $WaitMinutes minutes", text)

    def test_the_five_guards_run_in_order_before_each_batch(self):
        ready = re.search(r"function Assert-Ready.*?\n\}\n", source(), re.DOTALL).group(0)
        order = [
            "LastTick() -ne $script:Expected",          # 5: the operator's input since the checks
            "Get-ChatBusy $win $Target",                # 2: the chat is idle
            "Stop-Panel 4",                             # 3: the input is empty
            "$fg -ne $script:Hwnd",                     # 4: the foreground window
            "'Message input'",                          # 4: the focus
            "$active[0] -ne $Target",                   # 4: the active row
        ]
        at = [ready.index(part) for part in order]
        self.assertEqual(at, sorted(at), "guards out of order in Assert-Ready")

    def test_the_operators_idle_time_ignores_the_drivers_own_input(self):
        text = source()
        for part in ["GetLastInputInfo", "claude-panel.tick", "function Save-OwnTick", "function Test-OperatorIdle", "$tick -eq $own"]:
            self.assertIn(part, text)
        wait = re.search(r"function Wait-Guards.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("Test-OperatorIdle", wait)
        self.assertIn("Get-ChatBusy", wait)
        self.assertIn("Start-Sleep -Seconds 5", wait)

    def test_the_chat_is_idle_by_its_row_and_by_its_session_record(self):
        busy = re.search(r"function Get-ChatBusy.*?\n\}\n", source(), re.DOTALL).group(0)
        self.assertIn("-eq 'running'", busy)
        self.assertIn("Get-RecordStatus", busy)
        self.assertIn("-ne 'idle'", busy)

    def test_the_view_and_the_foreground_are_restored_on_every_path(self):
        text = source()
        logged = re.search(r"function Invoke-Logged.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertLess(logged.index("Restore-View"), logged.index("Write-PanelLog"))
        self.assertIn("catch [PanelExit]", logged)


class LogTests(unittest.TestCase):
    def test_one_line_per_run_with_no_message_text(self):
        text = source()
        self.assertIn("system\\logs", text)
        self.assertIn("claude-panel.log", text)
        log = re.search(r"function Write-PanelLog.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("exit={3} held_ms={4}", log)
        self.assertNotIn("$text", log.lower())
        self.assertEqual(len(re.findall(r"AppendAllText", text)), 1)
        self.assertEqual(len(re.findall(r"Write-PanelLog ", text)), 1, "one call, in Invoke-Logged")
        call = re.search(r"Write-PanelLog \$ActionName .*", text).group(0)
        self.assertEqual(call.strip(), "Write-PanelLog $ActionName $LogId $r.Code $r.Reason")

    def test_no_reason_it_prints_contains_the_text_that_was_typed(self):
        # The only variable holding the message is $text/$Text, and Stop-Panel messages are built without it.
        text = code_lines(source())
        for m in re.finditer(r"Stop-Panel \d+ [\"'].*", text):
            self.assertNotRegex(m.group(0), r"\$\{?[Tt]ext\b", m.group(0))


class SelfTestTests(unittest.TestCase):
    def test_selftest_runs_the_path_on_a_scratch_chat_and_prints_pass_or_fail_per_step(self):
        text = source()
        selftest = re.search(r"function Invoke-SelfTest.*?\n\}\n", text, re.DOTALL).group(0)
        for step in ["'new'", "'send /rename'", "'send /compact'", "'model (read only)'"]:
            self.assertIn(step, selftest)
        self.assertIn("'PASS'", text)
        self.assertIn("'FAIL'", text)
        self.assertIn("Select-Model ''", selftest)
        self.assertIn("compact_boundary", text)

    def test_selftest_puts_its_scratch_chat_on_haiku_before_the_first_message(self):
        text = source()
        selftest = re.search(r"function Invoke-SelfTest.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("$SelfTestModel = 'Haiku 4.5'", text)
        self.assertIn("Invoke-New $tmp $SelfTestModel", selftest)
        self.assertIn("zz-selftest-", selftest)
        new = re.search(r"function Invoke-New.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertLess(new.index("Select-Model $ModelName"), new.index("Send-Text"))
        self.assertNotIn("if ($ModelName -ne '')", new)

    def test_new_refuses_without_a_model_before_it_touches_anything(self):
        new = re.search(r"function Invoke-New.*?\n\}\n", source(), re.DOTALL).group(0)
        self.assertLess(new.index("Stop-Panel 1 '-Model is needed"), new.index("Read-TextFile"))
        self.assertLess(new.index("Stop-Panel 1 '-Model is needed"), new.index("Open-Window"))

    def test_model_without_a_name_only_lists_the_options(self):
        text = source()
        model = re.search(r"function Invoke-Model.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertNotIn("-Model is needed", model)
        picker = re.search(r"function Select-Model.*?\n\}\n", text, re.DOTALL).group(0)
        self.assertIn("options: {1}; active: {2}", picker)


class PairingTests(unittest.TestCase):
    def test_the_rust_side_names_this_script_and_the_override(self):
        rust = (ROOT / "system" / "af" / "src" / "panel.rs").read_text(encoding="utf-8")
        self.assertIn('"claude_panel.ps1"', rust)
        self.assertIn('"AF_PANEL_DRIVER"', rust)
        for flag in ["-Session", "-TextFile", "-Model", "-WaitMinutes"]:
            self.assertIn(f'"{flag}"', rust)
            self.assertIn(flag.lstrip("-"), source())

    def test_the_hard_nudge_tells_a_build_chat_to_compact_itself_when_idle(self):
        rust = (ROOT / "system" / "af" / "src" / "context.rs").read_text(encoding="utf-8")
        self.assertIn("af session compact {name} --when-idle", rust)


if __name__ == "__main__":
    unittest.main()
