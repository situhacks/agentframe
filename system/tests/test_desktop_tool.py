"""system/tools/desktop.ps1, live: checks on windows these tests open themselves.

The whole module skips unless AF_DESKTOP_TESTS=1, because the live tests open forms with bank and
password-manager titles, take the foreground, move the pointer and send input. Set it only on the machine
that runs the tool, and only while the operator is away: the tool's own away guard refuses to act while he is
there, and the module skips unless he has been idle for 120 s when it starts. With it set, live tests still
skip without an interactive, unlocked Windows desktop. Run with -rs.
No test captures the full screen or acts on a window it did not open; only `windows` lists the others.
The checks that need no desktop are in test_desktop_static.py and run in the normal suite.

The tests run in file order and are one run of the tool: they share a state file, so each command after the
first is allowed by the tool's own-tick rule (its own input does not lock it out). The restore tests come last
because they end that run, and they start theirs with -IdleSeconds 0, which exists for the tests.
"""

import os

import pytest

if os.environ.get("AF_DESKTOP_TESTS") != "1":
    pytest.skip("desktop tool tests run only with AF_DESKTOP_TESTS=1", allow_module_level=True)

import atexit
import ctypes
import ctypes.wintypes
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
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
ACTIVE = 4
BUSY = 1

# Every command runs with AF_ROOT here, so the run state is a temp file and never the operator's own.
STATE_ROOT = Path(tempfile.mkdtemp(prefix="af-desktop-state-"))
atexit.register(shutil.rmtree, STATE_ROOT, ignore_errors=True)
STATE_FILE = STATE_ROOT / "system" / "logs" / "desktop" / "run.json"

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


def run_env(root: Path = STATE_ROOT) -> dict:
    return {**os.environ, "AF_ROOT": str(root)}


def desktop(*args, timeout=60, env=None):
    return subprocess.run([*POWERSHELL, "-File", str(SCRIPT), *map(str, args)], capture_output=True, text=True,
                          encoding="utf-8", errors="replace", timeout=timeout, creationflags=NO_WINDOW,
                          env=run_env() if env is None else env)


def read_run_state() -> dict:
    return json.loads(STATE_FILE.read_text(encoding="utf-8-sig"))


def assert_active(result):
    assert result.returncode == ACTIVE, f"expected exit 4, got {result.returncode}: {result.stdout} {result.stderr}"
    assert re.search(r"^error: the operator is active \(last input \d+ s ago\); nothing was sent; wait and retry$",
                     result.stderr.strip()), result.stderr
    assert result.stdout == ""


# The operator's side of the screen, read with the same Win32 calls an ordinary program uses.

def win32():
    user32 = ctypes.windll.user32
    try:
        user32.SetProcessDPIAware()  # the tool is DPI aware; so must these coordinates be
    except OSError:
        pass
    return user32


def pointer() -> tuple[int, int]:
    point = ctypes.wintypes.POINT()
    win32().GetCursorPos(ctypes.byref(point))
    return point.x, point.y


def foreground_title() -> str:
    user32 = win32()
    buffer = ctypes.create_unicode_buffer(512)
    user32.GetWindowTextW(user32.GetForegroundWindow(), buffer, 512)
    return buffer.value


def wait_until(condition, timeout=5.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if condition():
            return True
        time.sleep(0.05)
    return condition()


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
Add-Type -Namespace AfProbe -Name Desk -MemberDefinition '
[StructLayout(LayoutKind.Sequential)] public struct LII { public uint cbSize; public uint dwTime; }
[DllImport("user32.dll")] public static extern System.IntPtr OpenInputDesktop(uint f, bool i, uint a);
[DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LII p);
public static long IdleMs() { LII l = new LII(); l.cbSize = 8; if (!GetLastInputInfo(ref l)) return -1; return (long)unchecked((uint)Environment.TickCount - l.dwTime); }'
if (-not [Environment]::UserInteractive) { 'not an interactive session'; exit 0 }
if ([AfProbe.Desk]::OpenInputDesktop(0, $false, 0x100) -eq [IntPtr]::Zero) { 'no input desktop: locked, or not the active session'; exit 0 }
$idle = [AfProbe.Desk]::IdleMs()
if ($idle -lt 120000) { 'the operator is at the computer: idle ' + [int]($idle / 1000) + ' s, and the tool needs 120'; exit 0 }
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
        pytest.skip(f"cannot run live: {answer or probe.stderr.strip()}")


@pytest.fixture(scope="module", autouse=True)
def end_the_run():
    yield
    if STATE_FILE.exists() and sys.platform == "win32":
        desktop("restore")  # the default guard: if he is back, nothing is touched


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
def test_a_denied_title_is_refused_and_not_repeated(shared_form, name):
    token = uuid.uuid4().hex
    shared_form.retitle(f"{name} {token}")
    result = desktop("focus", "-Window", shared_form.title)
    assert_refused(result, "off limits", "(off limits)")
    assert token not in result.stderr, "a refusal does not print the title of the window it refused"


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
    result = desktop(args[0], "-Window", form.title, *args[1:])
    assert_refused(result, "off limits")
    assert form.title.split()[-1] not in result.stderr
    time.sleep(0.2)
    state = form.state()
    assert (state["pressed"], state["label_clicks"], state["text"]) == (0, 0, "")


def test_screenshot_refuses_a_denied_window_and_writes_nothing(form, tmp_path):
    form.retitle(f"Acme Brokerage {uuid.uuid4().hex}")
    out = tmp_path / "denied.png"
    assert_refused(desktop("screenshot", "-Window", form.title, "-Out", out), "off limits")
    assert not out.exists()


def test_windows_marks_a_denied_window_and_hides_its_title(form):
    token = uuid.uuid4().hex
    form.retitle(f"Bitwarden Web Vault {token}")
    result = desktop("windows")
    assert_ok(result)
    assert ["powershell", "denied", "(off limits)"] in [line.split("\t")[1:] for line in result.stdout.splitlines()]
    assert token not in result.stdout and "Bitwarden" not in result.stdout


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


# Live: the away guard. A guarded command refuses to send anything while he is, as far as the tool can tell, there.

def guarded_args(form, command):
    if command == "focus":
        return ["focus"]
    if command == "invoke":
        return ["invoke", "-Name", "Press Me"]
    if command == "invoke-by-click":
        return ["invoke", "-Name", "Fallback Target"]
    if command == "click":
        x, y = element_bounds(form.title, "Fallback Target")
        return ["click", "-X", x, "-Y", y]
    if command == "type":
        return ["type", "-Text", "x"]
    return ["key", "-Keys", "x"]


@pytest.mark.parametrize("command", ["focus", "invoke", "invoke-by-click", "click", "type", "key"])
def test_a_gated_command_refuses_when_he_is_active_and_sends_nothing(form, command):
    args = guarded_args(form, command)
    before = STATE_FILE.read_bytes() if STATE_FILE.exists() else None
    result = desktop(args[0], "-Window", form.title, *args[1:], "-IdleSeconds", 999999)
    assert_active(result)
    time.sleep(0.3)
    state = form.state()
    assert (state["pressed"], state["label_clicks"], state["text"]) == (0, 0, "")
    after = STATE_FILE.read_bytes() if STATE_FILE.exists() else None
    assert after == before, "a refused command leaves the run state exactly as it was"


def test_restore_refuses_when_he_is_active_and_leaves_the_run_alone(form):
    assert_ok(desktop("focus", "-Window", form.title))
    before = STATE_FILE.read_bytes()
    assert_active(desktop("restore", "-IdleSeconds", 999999))
    assert STATE_FILE.read_bytes() == before


def test_the_read_only_commands_ignore_the_guard(shared_form, tmp_path):
    assert_ok(desktop("tree", "-Window", shared_form.title, "-Depth", 1, "-IdleSeconds", 999999))
    listed = desktop("windows", "-IdleSeconds", 999999)
    assert_ok(listed)
    assert shared_form.title in listed.stdout
    out = tmp_path / "guarded.png"
    assert_ok(desktop("screenshot", "-Window", shared_form.title, "-Out", out, "-IdleSeconds", 999999))
    assert out.exists()


def test_a_second_input_straight_after_the_first_is_allowed_and_his_tick_is_kept(form):
    assert_ok(desktop("type", "-Window", form.title, "-Text", "one"))
    first = read_run_state()
    assert first["own_tick"] is not None, "a command that sent input recorded the tick of its own last input"
    assert_ok(desktop("key", "-Window", form.title, "-Keys", "ctrl+a delete"))
    x, y = element_bounds(form.title, "Press Me")
    assert_ok(desktop("click", "-Window", form.title, "-X", x, "-Y", y))
    last = read_run_state()
    assert last["operator_tick"] == first["operator_tick"], "his last input is never replaced by the tool's"
    assert last["own_tick"] != first["own_tick"], "the tool's own tick follows its own input"
    form.wait_for(lambda s: s["pressed"] == 1 and s["text"] == "")


def test_the_run_state_holds_the_foreground_and_pointer_found_at_the_start(form):
    assert_ok(desktop("focus", "-Window", form.title))
    recorded = read_run_state()
    assert set(recorded) == {"operator_tick", "own_tick", "tick_at_write", "written_ms", "foreground", "pointer"}
    assert isinstance(recorded["pointer"]["x"], int) and isinstance(recorded["pointer"]["y"], int)
    assert isinstance(recorded["foreground"]["handle"], int) and recorded["foreground"]["process"]
    again = desktop("focus", "-Window", form.title)
    assert_ok(again)
    kept = read_run_state()
    assert kept["foreground"] == recorded["foreground"] and kept["pointer"] == recorded["pointer"], \
        "only the first command of a run records them"


def test_invoke_by_pattern_is_gated_records_state_and_does_not_lock_out_the_next_command(form):
    before = read_run_state()["written_ms"] if STATE_FILE.exists() else 0
    result = desktop("invoke", "-Window", form.title, "-Name", "Press Me")
    assert_ok(result)
    assert "via=invoke" in result.stdout
    assert read_run_state()["written_ms"] > before
    assert_ok(desktop("type", "-Window", form.title, "-Text", "next"))
    form.wait_for(lambda s: s["pressed"] == 1 and s["text"] == "next")


@pytest.mark.parametrize("damage", ["wrong own tick", "older than 30 minutes", "unreadable"])
def test_a_stale_run_state_falls_back_to_the_plain_rule(form, damage):
    assert_ok(desktop("type", "-Window", form.title, "-Text", "a"))
    saved = STATE_FILE.read_text(encoding="utf-8")
    data = json.loads(saved)
    assert data["own_tick"] is not None
    if damage == "wrong own tick":
        STATE_FILE.write_text(json.dumps({**data, "own_tick": data["own_tick"] + 1}), encoding="utf-8")
    elif damage == "older than 30 minutes":
        STATE_FILE.write_text(json.dumps({**data, "written_ms": data["written_ms"] - 31 * 60 * 1000}), encoding="utf-8")
    else:
        STATE_FILE.write_text("{not json", encoding="utf-8")
    try:
        result = desktop("type", "-Window", form.title, "-Text", "b")
        assert_active(result)
        # The tool's own input a moment ago is now plain input: the idle time is seconds, not his two minutes.
        time.sleep(0.2)
        assert form.state()["text"] == "a"
    finally:
        STATE_FILE.write_text(saved, encoding="utf-8")
    assert_ok(desktop("type", "-Window", form.title, "-Text", "c"))
    form.wait_for(lambda s: s["text"] == "ac")


def test_a_second_screen_command_while_one_is_sending_is_refused_and_sends_nothing(form):
    text = "a" * 400
    first = subprocess.Popen([*POWERSHELL, "-File", str(SCRIPT), "type", "-Window", form.title, "-Text", text],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, creationflags=NO_WINDOW,
                             env=run_env())
    try:
        form.wait_for(lambda s: len(s["text"]) > 5, timeout=40)
        second = desktop("key", "-Window", form.title, "-Keys", "b")
        assert second.returncode == BUSY, f"{second.returncode}: {second.stderr}"
        assert "another command that acts on the screen is running; nothing was sent" in second.stderr
    finally:
        out, err = first.communicate(timeout=120)
    assert first.returncode == 0, err
    form.wait_for(lambda s: s["text"] == text, timeout=10)


def test_a_full_screen_screenshot_is_refused_while_a_denied_window_is_visible(form, tmp_path):
    token = uuid.uuid4().hex
    form.retitle(f"Acme Brokerage {token}")
    out = tmp_path / "everything.png"
    result = desktop("screenshot", "-Out", out)
    assert_refused(result, "off-limits window", "-Window")
    assert token not in result.stderr
    assert not out.exists(), "nothing is written when a capture is refused"
    assert_refused(desktop("screenshot", "-Window", form.title, "-Out", out), "off limits")


# Live: restore. These come last: each ends the run, and each starts a new one with -IdleSeconds 0 (a start the
# plain rule would refuse, since the tests' own input has just reset the clock).

def start_new_run():
    assert_ok(desktop("restore", "-IdleSeconds", 0))
    assert not STATE_FILE.exists()


def bring_to_front_in_a_scratch_run(title, tmp_path):
    scratch = tmp_path / f"scratch-{uuid.uuid4().hex}"
    assert_ok(desktop("focus", "-Window", title, "-IdleSeconds", 0, env=run_env(scratch)))
    assert wait_until(lambda: foreground_title() == title), f"could not set up: {foreground_title()!r} is in front"


def test_restore_puts_the_pointer_and_the_foreground_back_and_removes_the_state(live, tmp_path):
    token = uuid.uuid4().hex
    prior, target = Fixture(tmp_path, f"AF prior {token}"), Fixture(tmp_path, f"AF target {token}", left=800)
    try:
        start_new_run()
        bring_to_front_in_a_scratch_run(prior.title, tmp_path)
        win32().SetCursorPos(120, 140)
        assert pointer() == (120, 140)
        assert_ok(desktop("focus", "-Window", target.title, "-IdleSeconds", 0))
        assert STATE_FILE.exists() and read_run_state()["pointer"] == {"x": 120, "y": 140}
        x, y = element_bounds(target.title, "Press Me")
        assert_ok(desktop("click", "-Window", target.title, "-X", x, "-Y", y, "-IdleSeconds", 0))
        target.wait_for(lambda s: s["pressed"] == 1)
        assert pointer() != (120, 140) and foreground_title() == target.title
        result = desktop("restore", "-IdleSeconds", 0)
        assert_ok(result)
        assert "restored the pointer to 120,140" in result.stdout, result.stdout
        assert "restored the foreground window (process 'powershell')" in result.stdout, result.stdout
        assert pointer() == (120, 140)
        assert wait_until(lambda: foreground_title() == prior.title), foreground_title()
        assert not STATE_FILE.exists()
    finally:
        prior.close()
        target.close()


def test_restore_reports_a_foreground_window_that_is_gone_and_still_puts_the_pointer_back(live, tmp_path):
    token = uuid.uuid4().hex
    prior, target = Fixture(tmp_path, f"AF prior {token}"), Fixture(tmp_path, f"AF target {token}", left=800)
    try:
        start_new_run()
        bring_to_front_in_a_scratch_run(prior.title, tmp_path)
        win32().SetCursorPos(150, 170)
        assert_ok(desktop("focus", "-Window", target.title, "-IdleSeconds", 0))
        prior.close()
        x, y = element_bounds(target.title, "Press Me")
        assert_ok(desktop("click", "-Window", target.title, "-X", x, "-Y", y, "-IdleSeconds", 0))
        result = desktop("restore", "-IdleSeconds", 0)
        assert_ok(result)
        assert "could not restore the foreground window (process 'powershell'): it is gone" in result.stdout, result.stdout
        assert "restored the pointer to 150,170" in result.stdout
        assert pointer() == (150, 170)
        assert not STATE_FILE.exists()
    finally:
        prior.close()
        target.close()


def test_restore_with_no_run_says_so_and_changes_nothing(live):
    start_new_run()
    before = pointer()
    result = desktop("restore", "-IdleSeconds", 0)
    assert_ok(result)
    assert "nothing to restore" in result.stdout
    assert pointer() == before


def test_the_run_state_is_under_the_checkout_when_af_root_is_not_set(form):
    without = {k: v for k, v in os.environ.items() if k != "AF_ROOT"}
    default_state = ROOT / "system" / "logs" / "desktop" / "run.json"
    assert not default_state.exists(), "an earlier run left its state; it is git-ignored, delete it"
    try:
        assert_ok(desktop("focus", "-Window", form.title, "-IdleSeconds", 0, env=without))
        assert default_state.exists()
    finally:
        desktop("restore", "-IdleSeconds", 0, env=without)
    assert not default_state.exists()
