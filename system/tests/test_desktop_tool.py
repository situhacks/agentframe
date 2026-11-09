"""system/tools/desktop.ps1: static checks, and live checks on windows these tests open themselves.

The whole module skips unless AF_DESKTOP_TESTS=1, because the live tests open forms with bank and
password-manager titles and take the foreground. Set it only on the machine that runs the tool.
With it set, live tests still skip without an interactive, unlocked Windows desktop. Run with -rs.
No test captures the full screen or acts on a window it did not open; only `windows` lists the others.
"""

import os

import pytest

if os.environ.get("AF_DESKTOP_TESTS") != "1":
    pytest.skip("desktop tool tests run only with AF_DESKTOP_TESTS=1", allow_module_level=True)

import json
import re
import shutil
import struct
import subprocess
import sys
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "system" / "tools" / "desktop.ps1"
DOC = ROOT / "library" / "process" / "desktop-tool.md"
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)
POWERSHELL = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass"]
REFUSED = 3
USAGE = 2

FIXTURE = r"""
param([string]$Title, [string]$StateFile, [string]$CommandFile, [int]$Left = 200)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing

$script:Pressed = 0
$script:LabelClicks = 0
$script:CommandsDone = 0
$script:OnType = ''
$script:Thief = $null

function Write-State {
    $json = @{
        title = $form.Text; text = $editor.Text; secret_length = $secret.TextLength
        pressed = $script:Pressed; label_clicks = $script:LabelClicks; commands_done = $script:CommandsDone
    } | ConvertTo-Json -Compress
    $temp = "$StateFile.tmp"
    [IO.File]::WriteAllText($temp, $json)
    for ($try = 0; $try -lt 10; $try++) {
        try { Move-Item -LiteralPath $temp -Destination $StateFile -Force; return } catch { Start-Sleep -Milliseconds 20 }
    }
}

function Start-Thief([string]$ThiefTitle) {
    $script:Thief = New-Object Windows.Forms.Form
    $script:Thief.Text = $ThiefTitle
    $script:Thief.TopMost = $true
    $script:Thief.StartPosition = 'Manual'
    $script:Thief.Location = New-Object Drawing.Point(($Left + 60), 260)
    $script:Thief.Show()
    $script:Thief.Activate()
    $holder = New-Object Windows.Forms.Timer
    $holder.Interval = 40
    $holder.Add_Tick({ if ($script:Thief) { $script:Thief.Activate() } })
    $holder.Start()
}

$form = New-Object Windows.Forms.Form
$form.Text = $Title
$form.StartPosition = 'Manual'
$form.Location = New-Object Drawing.Point($Left, 200)
$form.ClientSize = New-Object Drawing.Size(480, 320)
$form.BackColor = [Drawing.Color]::Magenta
$form.TopMost = $true

$editor = New-Object Windows.Forms.TextBox
$editor.Name = 'editor'; $editor.AccessibleName = 'Editor'; $editor.Multiline = $true
$editor.Location = New-Object Drawing.Point(10, 10); $editor.Size = New-Object Drawing.Size(300, 60)

$secret = New-Object Windows.Forms.TextBox
$secret.Name = 'secret'; $secret.AccessibleName = 'Secret'; $secret.UseSystemPasswordChar = $true
$secret.Location = New-Object Drawing.Point(10, 80); $secret.Size = New-Object Drawing.Size(300, 24)

$button = New-Object Windows.Forms.Button
$button.Name = 'pressMe'; $button.Text = 'Press Me'
$button.Location = New-Object Drawing.Point(10, 115); $button.Size = New-Object Drawing.Size(120, 30)

$disabled = New-Object Windows.Forms.Button
$disabled.Name = 'neverEnabled'; $disabled.Text = 'Never Enabled'; $disabled.Enabled = $false
$disabled.Location = New-Object Drawing.Point(140, 115); $disabled.Size = New-Object Drawing.Size(120, 30)

$label = New-Object Windows.Forms.Label
$label.Name = 'fallbackLabel'; $label.Text = 'Fallback Target'; $label.BackColor = [Drawing.Color]::White
$label.Location = New-Object Drawing.Point(10, 155); $label.Size = New-Object Drawing.Size(160, 30)

$form.Controls.AddRange(@($editor, $secret, $button, $disabled, $label))

$button.Add_Click({ $script:Pressed++; Write-State })
$label.Add_Click({ $script:LabelClicks++; Write-State })
$secret.Add_TextChanged({ Write-State })
$editor.Add_TextChanged({
    if ($script:OnType) {
        $action = $script:OnType
        $script:OnType = ''
        if ($action.StartsWith('thief=')) { Start-Thief $action.Substring(6) }
        elseif ($action.StartsWith('title=')) { $form.Text = $action.Substring(6) }
        elseif ($action -eq 'focus=secret') { [void]$secret.Focus() }
    }
    Write-State
})

$commands = New-Object Windows.Forms.Timer
$commands.Interval = 40
$commands.Add_Tick({
    if (-not (Test-Path -LiteralPath $CommandFile)) { return }
    $line = [IO.File]::ReadAllText($CommandFile, [Text.Encoding]::UTF8)
    Remove-Item -LiteralPath $CommandFile -Force
    if ($line.StartsWith('title=')) { $form.Text = $line.Substring(6) }
    elseif ($line -eq 'focus=secret') { [void]$secret.Focus() }
    elseif ($line -eq 'focus=editor') { [void]$editor.Focus() }
    elseif ($line.StartsWith('thief=')) { Start-Thief $line.Substring(6) }
    elseif ($line.StartsWith('ontype=')) { $script:OnType = $line.Substring(7) }
    $script:CommandsDone++
    Write-State
})
$commands.Start()

$failsafe = New-Object Windows.Forms.Timer
$failsafe.Interval = 240000
$failsafe.Add_Tick({ $form.Close() })
$failsafe.Start()

$form.Add_Shown({ [void]$editor.Focus(); Write-State })
[Windows.Forms.Application]::Run($form)
"""

NAMED_EXE = r"""
using System.Windows.Forms;
public static class NamedExe {
    [System.STAThread]
    public static void Main(string[] args) {
        Form form = new Form();
        form.Text = args[0];
        Timer failsafe = new Timer();
        failsafe.Interval = 120000;
        failsafe.Tick += delegate { form.Close(); };
        failsafe.Start();
        Application.Run(form);
    }
}
"""

DENIED_TITLES = [
    "KeePass Database", "1Password Vault", "Bitwarden Web Vault", "LastPass Sign In", "Acme Online Banking",
    "Acme Brokerage Account", "Acme Trading Desk", "Acme Investing", "Acme Wealth Portal", "Acme Financial",
    "Acme Credit Union", "Acme Crypto Wallet", "Reset Your Password", "TD Sign In", "RBC Sign In",
    "HSBC Accounts", "Chase Mobile", "PayPal Checkout", "Fidelity Accounts", "Windows Security", "Control Panel",
]
ALLOWED_LOOKALIKES = ["stdout td notes", "settings.json notes", "Bmore rbcx notes", "purchase order notes",
                      "Pending tasks"]


def desktop(*args, timeout=60, env=None):
    return subprocess.run([*POWERSHELL, "-File", str(SCRIPT), *map(str, args)], capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=timeout, creationflags=NO_WINDOW, env=env)


def local_denylist(tmp_path: Path, *lines: str) -> dict:
    """An environment whose AF_ROOT holds a local denylist with these lines."""
    root = tmp_path / f"root-{uuid.uuid4().hex}"
    listed = root / "workspace" / "life" / "systems" / "desktop-denylist.txt"
    listed.parent.mkdir(parents=True)
    listed.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return {**os.environ, "AF_ROOT": str(root)}


def assert_refused(result, *expected):
    assert result.returncode == REFUSED, f"expected a refusal, got {result.returncode}: {result.stdout} {result.stderr}"
    assert result.stderr.startswith("refused:"), result.stderr
    for text in expected:
        assert text in result.stderr, result.stderr


def assert_ok(result):
    assert result.returncode == 0, f"exit {result.returncode}: {result.stdout} {result.stderr}"


class Fixture:
    """A small form in its own PowerShell process; its state file is the test's independent readback."""

    def __init__(self, folder: Path, title: str, left: int = 200):
        self.title = title
        self.folder = folder
        self.state_file = folder / f"state-{uuid.uuid4().hex}.json"
        self.command_file = folder / f"command-{uuid.uuid4().hex}.txt"
        script = folder / "fixture.ps1"
        if not script.exists():
            script.write_text(FIXTURE, encoding="utf-8")
        self.process = subprocess.Popen(
            [*POWERSHELL, "-File", str(script), "-Title", title, "-StateFile", str(self.state_file),
             "-CommandFile", str(self.command_file), "-Left", str(left)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=NO_WINDOW)
        self.wait_for(lambda s: s["title"] == title, timeout=30)

    def state(self) -> dict | None:
        try:
            return json.loads(self.state_file.read_text(encoding="utf-8-sig"))
        except (OSError, ValueError):
            return None

    def wait_for(self, condition, timeout=5.0):
        deadline = time.monotonic() + timeout
        state = None
        while time.monotonic() < deadline:
            state = self.state()
            if state is not None and condition(state):
                return state
            time.sleep(0.03)
        raise AssertionError(f"fixture never reached the expected state; last state: {state}")

    def command(self, line: str):
        done = (self.state() or {}).get("commands_done", 0)
        temp = self.command_file.with_suffix(".tmp")
        temp.write_text(line, encoding="utf-8")
        temp.replace(self.command_file)
        self.wait_for(lambda s: s["commands_done"] > done)
        time.sleep(0.1)

    def retitle(self, title: str):
        self.title = title
        self.command(f"title={title}")

    def close(self):
        self.process.kill()
        self.process.wait(timeout=10)


def element_bounds(title: str, name: str) -> tuple[int, int]:
    result = desktop("tree", "-Window", title, "-Depth", "3")
    assert_ok(result)
    match = re.search(rf'name="{re.escape(name)}".*?bounds=(-?\d+),(-?\d+),(\d+),(\d+)', result.stdout)
    assert match, result.stdout
    x, y, width, height = map(int, match.groups())
    return x + width // 2, y + height // 2


def png_size(path: Path) -> tuple[int, int]:
    return struct.unpack(">II", path.read_bytes()[16:24])


def pixel(path: Path, x: int, y: int) -> tuple[int, int, int]:
    code = (f"Add-Type -AssemblyName System.Drawing; $b=[Drawing.Bitmap]::FromFile('{path}'); "
            f"$c=$b.GetPixel({x},{y}); \"$($c.R),$($c.G),$($c.B)\"; $b.Dispose()")
    out = subprocess.run([*POWERSHELL, "-Command", code], capture_output=True, text=True, creationflags=NO_WINDOW)
    return tuple(int(v) for v in out.stdout.strip().split(","))


PROBE = r"""
Add-Type -Namespace AfProbe -Name Desk -MemberDefinition '[DllImport("user32.dll")] public static extern System.IntPtr OpenInputDesktop(uint f, bool i, uint a); [DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();'
if (-not [Environment]::UserInteractive) { 'not an interactive session'; exit 0 }
if ([AfProbe.Desk]::OpenInputDesktop(0, $false, 0x100) -eq [IntPtr]::Zero) { 'no input desktop: locked, or not the active session'; exit 0 }
'ok'
"""


@pytest.fixture(scope="module")
def live():
    """Skip with the reason unless a real desktop can take input. Nothing here launches before this runs."""
    if sys.platform != "win32":
        pytest.skip("desktop.ps1 is Windows-only")
    if not shutil.which("powershell"):
        pytest.skip("powershell is not on PATH")
    try:
        probe = subprocess.run([*POWERSHELL, "-Command", PROBE], capture_output=True, text=True, timeout=60,
                               creationflags=NO_WINDOW)
    except (OSError, subprocess.TimeoutExpired) as exc:
        pytest.skip(f"could not probe the desktop: {exc}")
    answer = probe.stdout.strip()
    if answer != "ok":
        pytest.skip(f"no interactive desktop: {answer or probe.stderr.strip()}")


@pytest.fixture
def form(live, tmp_path):
    fixture = Fixture(tmp_path, f"AF Desktop Fixture {uuid.uuid4().hex}")
    yield fixture
    fixture.close()


@pytest.fixture(scope="module")
def shared_form(live, tmp_path_factory):
    fixture = Fixture(tmp_path_factory.mktemp("desktop"), f"AF Desktop Fixture {uuid.uuid4().hex}")
    yield fixture
    fixture.close()


@pytest.fixture(scope="module")
def named_exe(live, tmp_path_factory):
    """A real executable whose process name the test chooses, so process rules run against a real process."""
    folder = tmp_path_factory.mktemp("named")
    source = folder / "named.cs"
    source.write_text(NAMED_EXE, encoding="utf-8")
    built = folder / "named.exe"
    code = (f"Add-Type -TypeDefinition ([IO.File]::ReadAllText('{source}')) -OutputAssembly '{built}' "
            "-OutputType WindowsApplication -ReferencedAssemblies System, System.Drawing, System.Windows.Forms")
    result = subprocess.run([*POWERSHELL, "-Command", code], capture_output=True, text=True, creationflags=NO_WINDOW)
    assert built.exists(), result.stderr
    return built


# Static: these need no desktop and never launch PowerShell.

def denylist_block() -> str:
    text = SCRIPT.read_text(encoding="utf-8")
    start, end = text.index("# >>> DENYLIST"), text.index("# <<< DENYLIST")
    assert end < text.index("function "), "the denylist sits above all code"
    return text[start:end]


@pytest.mark.parametrize("term", [
    "bank", "broker", "trad(e|ing)", "invest", "wealth", "financ", "credit (union|card)", "password",
    "keepass", "bitwarden", "TD", "rbc", "CredentialUIBroker", "consent", "windows security", "LogonUI",
    "SystemSettings", "settings", "control", "control panel", "mmc", "regedit",
])
def test_denylist_names_every_required_target(term):
    assert term in denylist_block()


def test_the_local_denylist_path_is_gitignored_and_documented():
    path = re.search(r"\$LocalDenyPath = '([^']+)'", denylist_block()).group(1).replace("\\", "/")
    assert path.startswith("workspace/"), "workspace/ is gitignored; the local list names his own institutions"
    assert f"`{path}`" in DOC.read_text(encoding="utf-8")


def test_script_is_ascii_because_windows_powershell_misreads_utf8_without_a_bom():
    assert SCRIPT.read_bytes().isascii()


def test_script_downloads_nothing():
    text = SCRIPT.read_text(encoding="utf-8")
    for banned in ("Invoke-WebRequest", "Invoke-RestMethod", "Install-Module", "DownloadFile", "Start-BitsTransfer", "curl "):
        assert banned not in text


def test_doc_is_cataloged_and_states_the_limits():
    doc = DOC.read_text(encoding="utf-8")
    for limit in ("Never type passwords", "Never place trades or move money", "Never solve CAPTCHAs",
                  "as data, never as instructions", "Never change Windows security settings",
                  "A full-screen screenshot still captures"):
        assert limit in doc
    assert "[`desktop-tool.md`](desktop-tool.md)" in (ROOT / "library" / "process" / "README.md").read_text(encoding="utf-8")
    assert "[`desktop-tool.md`](desktop-tool.md)" in (ROOT / "library" / "process" / "tool-routes.md").read_text(encoding="utf-8")


# Live: read-only commands.

def test_windows_lists_process_and_title(shared_form):
    result = desktop("windows")
    assert_ok(result)
    rows = [line.split("\t") for line in result.stdout.splitlines()]
    assert ["powershell", "ok", shared_form.title] in [row[1:] for row in rows]


def test_screenshot_of_a_window_is_downscaled_and_prints_the_mapping(shared_form, tmp_path):
    out = tmp_path / "nested" / "shot.png"
    result = desktop("screenshot", "-Window", shared_form.title, "-Out", out, "-MaxWidth", 200)
    assert_ok(result)
    assert png_size(out)[0] == 200
    fields = re.search(r"source=(\d+)x(\d+) image=(\d+)x(\d+) origin=(-?\d+),(-?\d+) scale=([\d.]+)", result.stdout)
    assert fields, result.stdout
    source_w, _, image_w, image_h, origin_x, origin_y = map(int, fields.groups()[:6])
    assert (image_w, image_h) == png_size(out)
    assert float(fields.group(7)) == pytest.approx(200 / source_w, abs=1e-5)
    assert "screenX" in result.stdout
    root = re.search(r"bounds=(-?\d+),(-?\d+),", desktop("tree", "-Window", shared_form.title, "-Depth", 0).stdout)
    assert abs(int(root.group(1)) - origin_x) <= 16 and abs(int(root.group(2)) - origin_y) <= 16
    red, green, blue = pixel(out, image_w // 2, int(image_h * 0.8))
    assert red > 200 and green < 60 and blue > 200, "the form's magenta background is in the capture"


def test_screenshot_keeps_a_small_window_at_full_size(shared_form, tmp_path):
    out = tmp_path / "shot.png"
    result = desktop("screenshot", "-Window", shared_form.title, "-Out", out, "-MaxWidth", 5000)
    assert_ok(result)
    assert re.search(r"scale=1$", result.stdout, re.M), result.stdout


def test_screenshot_rejects_a_path_that_is_not_png(shared_form, tmp_path):
    assert desktop("screenshot", "-Window", shared_form.title, "-Out", tmp_path / "shot.bmp").returncode == USAGE


def test_screenshot_refuses_a_tracked_path_and_writes_nothing(shared_form):
    out = ROOT / "system" / "tools" / f"shot-{uuid.uuid4().hex}.png"
    result = desktop("screenshot", "-Window", shared_form.title, "-Out", out)
    assert result.returncode == USAGE, f"{result.returncode}: {result.stderr}"
    assert "inside the repository" in result.stderr
    assert not out.exists()


def test_tree_prints_type_name_id_bounds_enabled(shared_form):
    result = desktop("tree", "-Window", shared_form.title, "-Depth", 3)
    assert_ok(result)
    assert re.search(r'Button name="Press Me" id="pressMe" bounds=-?\d+,-?\d+,\d+,\d+ enabled=True', result.stdout), result.stdout
    assert re.search(r'Button name="Never Enabled" id="neverEnabled" bounds=\S+ enabled=False', result.stdout), result.stdout


def test_tree_depth_zero_is_the_window_only(shared_form):
    result = desktop("tree", "-Window", shared_form.title, "-Depth", 0)
    assert_ok(result)
    assert len(result.stdout.strip().splitlines()) == 1
    assert shared_form.title in result.stdout


def test_a_missing_window_is_not_a_refusal_and_exits_nonzero(live):
    result = desktop("tree", "-Window", f"no such window {uuid.uuid4().hex}")
    assert result.returncode == USAGE
    assert "no visible window" in result.stderr


def test_an_ambiguous_match_touches_nothing(live, tmp_path):
    token = uuid.uuid4().hex
    first, second = Fixture(tmp_path, f"AF {token} one"), Fixture(tmp_path, f"AF {token} two", left=800)
    try:
        result = desktop("focus", "-Window", token)
        assert result.returncode == USAGE, f"{result.returncode}: {result.stderr}"
        for text in ("matches 2 windows", first.title, second.title):
            assert text in result.stderr, result.stderr
    finally:
        first.close()
        second.close()


# Live: acting on the form.

def test_invoke_uses_the_pattern(form):
    result = desktop("invoke", "-Window", form.title, "-Name", "Press Me")
    assert_ok(result)
    assert "via=invoke" in result.stdout
    assert form.wait_for(lambda s: s["pressed"] == 1)


def test_invoke_falls_back_to_a_click_at_the_centre(form):
    result = desktop("invoke", "-Window", form.title, "-Name", "Fallback Target")
    assert_ok(result)
    assert "via=click" in result.stdout
    assert form.wait_for(lambda s: s["label_clicks"] == 1)


def test_invoke_reports_a_missing_and_a_disabled_element(form):
    assert desktop("invoke", "-Window", form.title, "-Name", "Nothing Here").returncode == USAGE
    result = desktop("invoke", "-Window", form.title, "-Name", "Never Enabled")
    assert result.returncode == USAGE
    assert "disabled" in result.stderr


def test_focus_brings_the_form_forward(form):
    assert_ok(desktop("focus", "-Window", form.title))


def test_type_sends_text_literally(form):
    text = 'Hello {world} +1 %^~ (ok) "q"'
    assert_ok(desktop("type", "-Window", form.title, "-Text", text))
    form.wait_for(lambda s: s["text"] == text)


def test_type_refuses_a_password_field(form):
    form.command("focus=secret")
    assert_refused(desktop("type", "-Window", form.title, "-Text", "not-a-real-one"), "password field")
    assert form.state()["secret_length"] == 0


def test_type_stops_when_the_app_moves_focus_to_a_password_field(form):
    form.command("ontype=focus=secret")
    assert_refused(desktop("type", "-Window", form.title, "-Text", "a" * 50), "password field", "characters were sent")
    time.sleep(0.2)
    # One character can already be queued when the app moves the focus; the doc states this limit.
    assert form.state()["secret_length"] <= 1


def test_key_refuses_a_password_field(form):
    form.command("focus=secret")
    assert_refused(desktop("key", "-Window", form.title, "-Keys", "a b c"), "password field", "No input was sent")
    assert form.state()["secret_length"] == 0


def test_key_sends_chords_in_order(form):
    assert_ok(desktop("type", "-Window", form.title, "-Text", "remove me"))
    form.wait_for(lambda s: s["text"] == "remove me")
    assert_ok(desktop("key", "-Window", form.title, "-Keys", "ctrl+a delete"))
    form.wait_for(lambda s: s["text"] == "")


@pytest.mark.parametrize("keys", ["win+r", "alt+tab", "alt+esc", "ctrl+esc", "ctrl+shift+esc"])
def test_key_refuses_chords_that_leave_the_window(form, keys):
    assert_refused(desktop("key", "-Window", form.title, "-Keys", keys), "No input was sent")


def test_key_with_an_unknown_name_sends_nothing(form):
    assert_ok(desktop("type", "-Window", form.title, "-Text", "keep"))
    form.wait_for(lambda s: s["text"] == "keep")
    assert desktop("key", "-Window", form.title, "-Keys", "ctrl+a nosuchkey").returncode == USAGE
    time.sleep(0.2)
    assert form.state()["text"] == "keep"


def test_click_hits_a_coordinate_in_the_window(form):
    x, y = element_bounds(form.title, "Press Me")
    assert_ok(desktop("click", "-Window", form.title, "-X", x, "-Y", y))
    form.wait_for(lambda s: s["pressed"] == 1)


@pytest.mark.parametrize("args", [
    ("click", "-X", 1, "-Y", 1), ("type", "-Text", "x"), ("key", "-Keys", "enter"), ("focus",), ("tree",),
])
def test_commands_that_need_a_window_require_one(live, args):
    result = desktop(*args)
    assert result.returncode == USAGE
    assert "-Window is required" in result.stderr


def test_a_click_over_another_window_is_refused_and_prints_what_was_found(live, tmp_path):
    token = uuid.uuid4().hex
    target, other = Fixture(tmp_path, f"AF target {token}"), Fixture(tmp_path, f"AF other {token}", left=800)
    try:
        x, y = element_bounds(other.title, "Press Me")
        assert_refused(desktop("click", "-Window", target.title, "-X", x, "-Y", y), "over another window")
        time.sleep(0.2)
        assert other.state()["pressed"] == 0
        assert target.state()["pressed"] == 0
    finally:
        target.close()
        other.close()


def test_raw_input_is_refused_when_another_window_holds_the_foreground(form):
    thief = f"AF Thief {uuid.uuid4().hex}"
    form.command(f"thief={thief}")
    assert_refused(desktop("type", "-Window", form.title, "-Text", "nope"), "not the requested one", thief)
    assert form.state()["text"] == ""


def test_typing_stops_when_focus_is_lost_partway(form):
    thief = f"AF Thief {uuid.uuid4().hex}"
    form.command(f"ontype=thief={thief}")
    assert_refused(desktop("type", "-Window", form.title, "-Text", "a" * 200), "not the requested one", thief, "characters were sent")
    time.sleep(0.2)
    assert 0 < len(form.state()["text"]) < 200


def test_typing_stops_when_the_title_becomes_denied_partway(form):
    form.command(f"ontype=title=RBC Online Banking {uuid.uuid4().hex}")
    assert_refused(desktop("type", "-Window", form.title, "-Text", "a" * 200), "off limits", "characters were sent")
    time.sleep(0.2)
    assert 0 < len(form.state()["text"]) < 200


# Live: the denylist, on fake titles and on real processes with chosen names.

@pytest.mark.parametrize("name", DENIED_TITLES)
def test_a_denied_title_is_refused(shared_form, name):
    shared_form.retitle(f"{name} {uuid.uuid4().hex}")
    assert_refused(desktop("focus", "-Window", shared_form.title), "off limits")


def test_the_exact_settings_title_is_refused(shared_form):
    shared_form.retitle("Settings")
    assert_refused(desktop("focus", "-Window", "Settings"), "off limits")


def test_the_local_denylist_refuses_without_repeating_its_rule(shared_form, tmp_path):
    token = f"zebraco{uuid.uuid4().hex}"
    rule = f"(?i){token}"
    shared_form.retitle(f"{token.upper()} Portal")
    without = {k: v for k, v in os.environ.items() if k != "AF_ROOT"}
    assert_ok(desktop("tree", "-Window", shared_form.title, "-Depth", 0, env=without))
    result = desktop("focus", "-Window", shared_form.title, env=local_denylist(tmp_path, "# his own", "", rule))
    assert_refused(result, "local denylist")
    assert rule not in result.stderr


def test_an_invalid_local_rule_stops_the_script(live, tmp_path):
    result = desktop("windows", env=local_denylist(tmp_path, "(unclosed"))
    assert result.returncode == 1, f"{result.returncode}: {result.stderr}"
    assert "not a valid .NET regex" in result.stderr
    assert result.stdout == ""


@pytest.mark.parametrize("name", ALLOWED_LOOKALIKES)
def test_a_lookalike_title_is_allowed(shared_form, name):
    shared_form.retitle(f"{name} {uuid.uuid4().hex}")
    assert_ok(desktop("tree", "-Window", shared_form.title, "-Depth", 0))


@pytest.mark.parametrize("args", [
    ("tree",), ("focus",), ("invoke", "-Name", "Press Me"), ("click", "-X", 300, "-Y", 300), ("type", "-Text", "x"),
    ("key", "-Keys", "enter"),
])
def test_every_acting_command_refuses_a_denied_window(form, args):
    form.retitle(f"RBC Online Banking {uuid.uuid4().hex}")
    assert_refused(desktop(args[0], "-Window", form.title, *args[1:]), "off limits")
    time.sleep(0.2)
    state = form.state()
    assert (state["pressed"], state["label_clicks"], state["text"]) == (0, 0, "")


def test_screenshot_refuses_a_denied_window_and_writes_nothing(form, tmp_path):
    form.retitle(f"Acme Brokerage {uuid.uuid4().hex}")
    out = tmp_path / "denied.png"
    assert_refused(desktop("screenshot", "-Window", form.title, "-Out", out), "off limits")
    assert not out.exists()


def test_windows_marks_a_denied_window(form):
    form.retitle(f"Bitwarden Web Vault {uuid.uuid4().hex}")
    result = desktop("windows")
    assert_ok(result)
    assert ["denied", form.title] in [line.split("\t")[2:] for line in result.stdout.splitlines()]


@pytest.mark.parametrize("process", ["KeePass", "consent", "SystemSettings"])
def test_a_denied_process_is_refused_under_a_neutral_title(named_exe, tmp_path, process):
    exe = tmp_path / f"{process}.exe"
    shutil.copy(named_exe, exe)
    title = f"AF Neutral {uuid.uuid4().hex}"
    running = subprocess.Popen([str(exe), title])
    try:
        deadline = time.monotonic() + 20
        while title not in desktop("windows").stdout:
            assert time.monotonic() < deadline, "the named window never appeared"
        assert_refused(desktop("focus", "-Window", title), "off limits", process)
    finally:
        running.kill()
        running.wait(timeout=10)
