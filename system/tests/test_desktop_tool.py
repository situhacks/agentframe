"""system/tools/desktop.ps1, live: checks on windows these tests open themselves.

The whole module skips unless AF_DESKTOP_TESTS=1, because the live tests open forms with bank and
password-manager titles, take the foreground, move the pointer and send input. Set it only on the machine
that runs the tool, and only while the operator is away. The module skips unless he has been idle for 120 s
when it starts, and it keeps watching. It remembers the last-input tick as of its last known state and reads it
again before every test, every tool call and every helper window it opens. The only tick it ever adopts after a
gated tool call is the own_tick the TOOL wrote to the state file for that call, or the unchanged tick; a tick
it reads itself after a call that sent nothing is never adopted. If the clock is anything else, or the tool
exits 4 for a reason the test did not set up (expect_active), he is back: it closes every helper window, puts
his pointer and foreground window back (the foreground only if it is one of these helpers' or none; the pointer
only if he has not moved it), and skips the rest with "the operator came back". At the start it records his
pointer and foreground window and puts both back at the end, except when the clock moved since the last known
tick (Ctrl+C is his input too): then only the helpers are closed. With it set, live tests still skip without an
interactive, unlocked Windows desktop. Run with -rs.
No test captures the full screen or acts on a window it did not open; only `windows` lists the others.
The checks that need no desktop are in test_desktop_static.py and run in the normal suite.

Every command runs with its own AF_ROOT, so the run state is a temp file and never the operator's. A run lasts
at most 5 minutes, so before each test the harness plants a fresh run in that state file: his last input is the
one it saw at the start, and the tool's own last tick is the one it last adopted. The default -IdleSeconds then
works as it does for a real job (the tool's own-tick rule). A test that must begin a run of its own waits until
the harness has seen the clock still for 3 s (still_for) and passes -IdleSeconds 3, so the tool's own guard covers
its start-up; the value 0 is used nowhere. The restore tests come last because each ends a run, and the test of
his input in the middle of a type is the very last, because it makes the clock his.
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
import threading
import time
import uuid
from pathlib import Path

from desktop_fixtures import FIXTURE, NAMED_EXE, WPF_FIXTURE

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "system" / "tools" / "desktop.ps1"
DOC = ROOT / "library" / "process" / "desktop-tool.md"
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)
POWERSHELL = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass"]
REFUSED = 3
USAGE = 2
ACTIVE = 4
BUSY = 1
GATED = {"focus", "invoke", "click", "type", "key", "restore"}
AWAY = 3  # seconds of stillness a test run waits for, and the -IdleSeconds it then passes
WRAP = 2 ** 32

STATE_ROOT = Path(tempfile.mkdtemp(prefix="af-desktop-state-"))
atexit.register(shutil.rmtree, STATE_ROOT, ignore_errors=True)
STATE_FILE = STATE_ROOT / "system" / "logs" / "desktop" / "run.json"
HEARTBEAT = Path(tempfile.mkdtemp(prefix="af-desktop-beat-")) / "beat"
atexit.register(shutil.rmtree, HEARTBEAT.parent, ignore_errors=True)

DENIED_TITLES = [
    "KeePass Database", "1Password Vault", "Bitwarden Web Vault", "LastPass Sign In", "Acme Online Banking",
    "Acme Brokerage Account", "Acme Trading Desk", "Acme Investing", "Acme Wealth Portal", "Acme Financial",
    "Acme Credit Union", "Acme Crypto Wallet", "Reset Your Password", "TD Sign In", "RBC Sign In",
    "HSBC Accounts", "Chase Mobile", "PayPal Checkout", "Fidelity Accounts", "Windows Security", "Control Panel",
]
ALLOWED_LOOKALIKES = ["stdout td notes", "settings.json notes", "Bmore rbcx notes", "purchase order notes",
                      "Pending tasks"]


# --- The operator's side of the screen, read with the same Win32 calls an ordinary program uses. ---------

class LASTINPUTINFO(ctypes.Structure):
    _fields_ = [("cbSize", ctypes.c_uint), ("dwTime", ctypes.c_uint)]


def win32():
    user32 = ctypes.windll.user32
    try:
        user32.SetProcessDPIAware()  # the tool is DPI aware; so must these coordinates be
    except OSError:
        pass
    return user32


def last_input_tick() -> int:
    info = LASTINPUTINFO()
    info.cbSize = ctypes.sizeof(info)
    if not win32().GetLastInputInfo(ctypes.byref(info)):
        return -1
    return info.dwTime


def tick_now() -> int:
    return ctypes.windll.kernel32.GetTickCount() & 0xFFFFFFFF


def pointer() -> tuple[int, int]:
    point = ctypes.wintypes.POINT()
    win32().GetCursorPos(ctypes.byref(point))
    return point.x, point.y


def window_title(hwnd) -> str:
    buffer = ctypes.create_unicode_buffer(512)
    win32().GetWindowTextW(hwnd, buffer, 512)
    return buffer.value


def foreground_title() -> str:
    return window_title(win32().GetForegroundWindow())


def window_pid(hwnd) -> int:
    pid = ctypes.wintypes.DWORD()
    win32().GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    return pid.value


def raise_window(hwnd) -> None:
    """Bring a window forward without sending it input, the way the tool does (shared input with the front thread)."""
    user32 = win32()
    if user32.IsIconic(hwnd):
        user32.ShowWindow(hwnd, 9)
    front = user32.GetForegroundWindow()
    front_thread = user32.GetWindowThreadProcessId(front, None) if front else 0
    mine = ctypes.windll.kernel32.GetCurrentThreadId()
    attached = bool(front_thread and front_thread != mine and user32.AttachThreadInput(mine, front_thread, True))
    try:
        user32.BringWindowToTop(hwnd)
        user32.SetForegroundWindow(hwnd)
    finally:
        if attached:
            user32.AttachThreadInput(mine, front_thread, False)


def wait_until(condition, timeout=5.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if condition():
            return True
        time.sleep(0.05)
    return condition()


# --- Watching for him: one clock read per tool call, test and own action. ---------------------------------

class Watch:
    expected = None          # the last-input tick straight after the harness's latest tool call or action
    operator_tick = None     # his last input before the module began (he was idle 120 s then)
    pointer = None           # where the pointer was after the harness's latest action
    pointer_at_start = None  # where he left it
    foreground_at_start = 0  # the window he had in front
    came_back = False
    helpers = []             # everything that must be closed if he comes back
    pids = set()             # the helper processes: a foreground window of theirs is not his choice


def track(process):
    """Register a helper process so it is closed if the operator comes back."""
    Watch.helpers.append(process)
    Watch.pids.add(process.pid)
    return process


def close_helpers():
    for helper in list(Watch.helpers):
        try:
            helper.kill()
            helper.wait(timeout=10)
        except Exception:
            pass


def put_back():
    """His pointer and foreground window as the module found them; see the module docstring for when it holds back."""
    user32 = win32()
    if Watch.pointer_at_start and (not Watch.came_back or pointer() == Watch.pointer):
        user32.SetCursorPos(*Watch.pointer_at_start)
    hwnd = Watch.foreground_at_start
    if hwnd and user32.IsWindow(hwnd) and user32.IsWindowVisible(hwnd):
        front = user32.GetForegroundWindow()
        if front != hwnd and (not Watch.came_back or not front or window_pid(front) in Watch.pids):
            raise_window(hwnd)


def operator_returned(detail="the clock is not where the harness left it"):
    Watch.came_back = True
    close_helpers()
    put_back()
    pytest.skip(f"the operator came back ({detail})")


def check_away():
    """Skip if he is back: the clock is not where the harness left it."""
    if Watch.expected is None:
        return
    if Watch.came_back:
        pytest.skip("the operator came back")
    if last_input_tick() != Watch.expected:
        operator_returned()


def still_for(seconds=AWAY):
    """Wait until the harness has seen the clock still for this long (it is checked all the while)."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        check_away()
        time.sleep(0.2)
    check_away()


def move_pointer(x, y):
    check_away()
    win32().SetCursorPos(x, y)
    # A tick read here is never adopted: if the move itself moved the clock, the harness cannot tell it from him.
    if last_input_tick() != Watch.expected:
        operator_returned("the clock moved when the harness moved the pointer")
    Watch.pointer = pointer()


# --- Tool calls and the run state ----------------------------------------------------------------------

def run_env(root: Path = STATE_ROOT) -> dict:
    return {**os.environ, "AF_ROOT": str(root)}


def tool_own_tick(env):
    """The own_tick the tool wrote to the state file it used for that call, or None."""
    root = STATE_ROOT if env is None else (Path(env["AF_ROOT"]) if env.get("AF_ROOT") else ROOT)
    try:
        return json.loads((root / "system" / "logs" / "desktop" / "run.json").read_text(encoding="utf-8-sig")).get("own_tick")
    except (OSError, ValueError):
        return None


def adopt_after(command, returncode, env, expect_active, before):
    """After a tool call: keep the unchanged tick, or the one the tool says it made. Anything else is him."""
    after = last_input_tick()
    problem = None
    if returncode == ACTIVE and not expect_active:
        problem = f"{command} exited 4 and the test had not set that up"
    elif after != before:
        own = tool_own_tick(env) if command in GATED else None
        if own is None or after != own:
            problem = f"the clock moved during {command} and the tool did not make that move"
        elif returncode == ACTIVE:
            problem = f"{command} exited 4 after the clock moved"
    if problem:
        operator_returned(problem)
    Watch.expected = after
    Watch.pointer = pointer()


def desktop(*args, timeout=60, env=None, watch=True, expect_active=False):
    """Run the tool. expect_active: the test itself makes the tool refuse with exit 4 (-IdleSeconds 999999, a damaged state)."""
    command = str(args[0])
    if watch:
        check_away()
    before = Watch.expected
    result = subprocess.run([*POWERSHELL, "-File", str(SCRIPT), *map(str, args)], capture_output=True, text=True,
                            encoding="utf-8", errors="replace", timeout=timeout, creationflags=NO_WINDOW,
                            env=run_env() if env is None else env)
    if watch:
        adopt_after(command, result.returncode, env, expect_active, before)
    return result


def adopt_after_background(first, env=None):
    """After a command started with Popen has finished: it must not have been stopped by him, and its tick is the tool's."""
    if first.returncode == ACTIVE:
        operator_returned("the background command was stopped by input the test did not make")
    adopt_after("type", first.returncode, env, False, Watch.expected if Watch.expected is not None else last_input_tick())


def read_run_state() -> dict:
    return json.loads(STATE_FILE.read_text(encoding="utf-8-sig"))


def write_run_state(**fields):
    data = {"operator_tick": Watch.operator_tick, "own_tick": Watch.expected, "run_start_tick": tick_now(),
            "run_start_ms": int(time.time() * 1000), "foreground": None,
            "pointer": dict(zip("xy", pointer())), **fields}
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(json.dumps(data), encoding="utf-8")
    return data


def tool_stderr(result) -> str:
    """The tool's messages without its trace lines (AF_DESKTOP_TRACE=1 may be set for the whole run)."""
    return "\n".join(line for line in result.stderr.splitlines() if not line.startswith("trace:"))


def record_trace(source: str, stderr: str):
    """Append the trace lines of a call to the file named by AF_DESKTOP_TRACE_OUT, if it is set."""
    target = os.environ.get("AF_DESKTOP_TRACE_OUT")
    lines = [line for line in stderr.splitlines() if line.startswith("trace:")]
    if target and lines:
        with open(target, "a", encoding="utf-8") as handle:
            handle.writelines(f"{source}\t{line}\n" for line in lines)


def assert_active(result):
    assert result.returncode == ACTIVE, f"expected exit 4, got {result.returncode}: {result.stdout} {result.stderr}"
    assert re.search(r"^error: the operator is active \(last input \d+ s ago\); nothing was sent; wait and retry$",
                     tool_stderr(result).strip()), result.stderr
    assert result.stdout == ""


def local_denylist(tmp_path: Path, *lines: str) -> dict:
    """An environment whose AF_ROOT holds a local denylist with these lines."""
    root = tmp_path / f"root-{uuid.uuid4().hex}"
    listed = root / "workspace" / "life" / "systems" / "desktop-denylist.txt"
    listed.parent.mkdir(parents=True)
    listed.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return {**os.environ, "AF_ROOT": str(root)}


def assert_refused(result, *expected):
    assert result.returncode == REFUSED, f"expected a refusal, got {result.returncode}: {result.stdout} {result.stderr}"
    stderr = tool_stderr(result)
    assert stderr.startswith("refused:"), result.stderr
    for text in expected:
        assert text in stderr, result.stderr


def assert_ok(result):
    assert result.returncode == 0, f"exit {result.returncode}: {result.stdout} {result.stderr}"


class Fixture:
    """A small form in its own PowerShell process; its state file is the test's independent readback."""

    def __init__(self, folder: Path, title: str, left: int = 200):
        check_away()  # a topmost window that takes focus is not opened while he is at the machine
        self.title = title
        self.folder = folder
        self.state_file = folder / f"state-{uuid.uuid4().hex}.json"
        self.command_file = folder / f"command-{uuid.uuid4().hex}.txt"
        script = folder / "fixture.ps1"
        if not script.exists():
            script.write_text(FIXTURE, encoding="utf-8")
        self.process = subprocess.Popen(
            [*POWERSHELL, "-File", str(script), "-Title", title, "-StateFile", str(self.state_file),
             "-CommandFile", str(self.command_file), "-HeartbeatFile", str(HEARTBEAT), "-Left", str(left)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=NO_WINDOW)
        track(self.process)
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


class WpfFixture(Fixture):
    """A WPF window in its own STA PowerShell process: the managed UI Automation client sees a real Button with an
    Invoke pattern and an Edit there (it sees only panes with no patterns in WinForms). State: title, pressed, text."""

    def __init__(self, folder: Path, title: str):
        check_away()
        self.title = title
        self.folder = folder
        self.state_file = folder / f"wpf-state-{uuid.uuid4().hex}.json"
        script = folder / "wpf_fixture.ps1"
        if not script.exists():
            script.write_text(WPF_FIXTURE, encoding="utf-8")
        self.process = subprocess.Popen(
            [*POWERSHELL, "-STA", "-File", str(script), "-Title", title, "-StateFile", str(self.state_file),
             "-HeartbeatFile", str(HEARTBEAT)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=NO_WINDOW)
        track(self.process)
        self.wait_for(lambda s: s["title"] == title, timeout=30)


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
    """Skip with the reason unless a real desktop can take input and he is away. Nothing launches before this runs."""
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
    # His desktop as he left it, and the clock as it is while he is away. From here the harness watches.
    Watch.operator_tick = Watch.expected = last_input_tick()
    Watch.pointer = Watch.pointer_at_start = pointer()
    Watch.foreground_at_start = win32().GetForegroundWindow()
    stop = threading.Event()

    def beat():
        while not stop.is_set():
            try:
                HEARTBEAT.write_text(str(time.time()), encoding="utf-8")
            except OSError:
                pass
            stop.wait(3)

    HEARTBEAT.write_text("start", encoding="utf-8")
    thread = threading.Thread(target=beat, daemon=True)
    thread.start()
    yield
    # The finalizer runs on every path, including after "the operator came back" and after Ctrl+C. Read the clock
    # first: if it moved since the last known tick it was him (Ctrl+C is his input), and nothing of his is touched.
    moved = last_input_tick() != Watch.expected
    close_helpers()
    stop.set()
    thread.join(timeout=5)
    if moved:
        Watch.came_back = True
    if not Watch.came_back:
        if STATE_FILE.exists():
            desktop("restore", watch=False)  # the default guard: if he is back, nothing is touched
        put_back()


@pytest.fixture(autouse=True)
def away_and_in_a_fresh_run(live):
    """Before every test: he is still away, and the run state is a fresh run (a run lasts at most 5 minutes)."""
    check_away()
    write_run_state()


@pytest.fixture
def form(live, tmp_path):
    fixture = Fixture(tmp_path, f"AF Desktop Fixture {uuid.uuid4().hex}")
    yield fixture
    fixture.close()


@pytest.fixture
def wpf_form(live, tmp_path):
    fixture = WpfFixture(tmp_path, f"AF Wpf Fixture {uuid.uuid4().hex}")
    yield fixture
    fixture.close()


@pytest.fixture(scope="module")
def shared_form(live, tmp_path_factory):
    check_away()
    fixture = Fixture(tmp_path_factory.mktemp("desktop"), f"AF Desktop Fixture {uuid.uuid4().hex}")
    yield fixture
    fixture.close()


@pytest.fixture(scope="module")
def named_exe(live, tmp_path_factory):
    """A real executable whose process name the test chooses, so process rules run against a real process."""
    check_away()
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
    assert "is inside a repository" in result.stderr
    assert not out.exists()


def test_tree_prints_type_name_id_bounds_enabled_for_a_wpf_window(wpf_form):
    result = desktop("tree", "-Window", wpf_form.title, "-Depth", 4)
    assert_ok(result)
    assert re.search(r'Button name="Press Me" id="[^"]*" bounds=-?\d+,-?\d+,\d+,\d+ enabled=True', result.stdout), result.stdout
    assert re.search(r'Edit name="[^"]*" id="[^"]*" bounds=-?\d+,-?\d+,\d+,\d+ enabled=True', result.stdout), result.stdout


def test_tree_shows_winforms_controls_as_panes_with_their_names(shared_form):
    # The limit the doc states: a classic WinForms control exposes no control type and no patterns to this client.
    result = desktop("tree", "-Window", shared_form.title, "-Depth", 3)
    assert_ok(result)
    assert re.search(r'Pane name="Press Me" id="[^"]*" bounds=-?\d+,-?\d+,\d+,\d+ enabled=(True|False)', result.stdout), result.stdout
    assert re.search(r'Pane name="Never Enabled" id="[^"]*" bounds=\S+ enabled=(True|False)', result.stdout), result.stdout


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

def test_invoke_uses_the_pattern(wpf_form):
    result = desktop("invoke", "-Window", wpf_form.title, "-Name", "Press Me")
    assert_ok(result)
    assert "via=invoke" in result.stdout
    assert wpf_form.wait_for(lambda s: s["pressed"] == 1)


def test_invoke_does_not_call_a_label_inside_its_own_button_ambiguous_but_still_refuses_two_sibling_buttons(wpf_form):
    # Each WPF button has a child Text element with the same name; the label is not a second match. Two sibling
    # buttons with one name are still ambiguous: the tool refuses (exit 2) and presses neither.
    result = desktop("invoke", "-Window", wpf_form.title, "-Name", "Twin")
    assert result.returncode == USAGE, f"{result.returncode}: {result.stderr}"
    assert "'Twin' matches 2 elements, so none was touched" in tool_stderr(result)
    time.sleep(0.3)
    assert wpf_form.state()["twins"] == 0
    assert_ok(desktop("invoke", "-Window", wpf_form.title, "-Name", "Press Me"))
    assert wpf_form.wait_for(lambda s: s["pressed"] == 1)


def test_invoke_on_a_winforms_button_falls_back_to_a_click_and_the_press_lands(form):
    result = desktop("invoke", "-Window", form.title, "-Name", "Press Me")
    assert_ok(result)
    assert "via=click" in result.stdout, result.stdout
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
    assert_ok(desktop("key", "-Window", form.title, "-Keys", "ctrl+home shift+ctrl+end delete"))
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

    def denied_rows():
        # A denied window's title is hidden, so the window is found by its process name and the denied flag.
        rows = [line.split("\t") for line in desktop("windows").stdout.splitlines()]
        return sum(1 for r in rows if len(r) >= 4 and r[1].lower() == process.lower() and r[2] == "denied")

    already = denied_rows()
    running = track(subprocess.Popen([str(exe), title]))
    try:
        deadline = time.monotonic() + 20
        while denied_rows() <= already:
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
    before = STATE_FILE.read_bytes()
    result = desktop(args[0], "-Window", form.title, *args[1:], "-IdleSeconds", 999999, expect_active=True)
    assert_active(result)
    time.sleep(0.3)
    state = form.state()
    assert (state["pressed"], state["label_clicks"], state["text"]) == (0, 0, "")
    assert STATE_FILE.read_bytes() == before, "a refused command leaves the run state exactly as it was"


def test_restore_refuses_when_he_is_active_and_leaves_the_run_alone(form):
    before = STATE_FILE.read_bytes()
    assert_active(desktop("restore", "-IdleSeconds", 999999, expect_active=True))
    assert STATE_FILE.read_bytes() == before


@pytest.mark.parametrize("command", ["focus", "restore", "type"])
def test_a_negative_idle_seconds_is_bad_usage(form, command):
    before = STATE_FILE.read_bytes()
    extra = ["-Window", form.title, "-Text", "x"] if command == "type" else (["-Window", form.title] if command == "focus" else [])
    result = desktop(command, *extra, "-IdleSeconds", -1)
    assert result.returncode == USAGE and "-IdleSeconds must be 0 or more" in result.stderr, result.stderr
    assert STATE_FILE.read_bytes() == before
    assert form.state()["text"] == ""


def test_the_read_only_commands_ignore_the_guard(shared_form, tmp_path):
    assert_ok(desktop("tree", "-Window", shared_form.title, "-Depth", 1, "-IdleSeconds", 999999))
    listed = desktop("windows", "-IdleSeconds", 999999)
    assert_ok(listed)
    assert shared_form.title in listed.stdout
    out = tmp_path / "guarded.png"
    assert_ok(desktop("screenshot", "-Window", shared_form.title, "-Out", out, "-IdleSeconds", 999999))
    assert out.exists()


def test_a_second_input_straight_after_the_first_is_allowed_and_his_tick_is_kept(form):
    planted = read_run_state()
    assert_ok(desktop("type", "-Window", form.title, "-Text", "one"))
    first = read_run_state()
    assert first["own_tick"] is not None, "a command that sent input recorded the tick of its own last input"
    assert_ok(desktop("key", "-Window", form.title, "-Keys", "ctrl+home shift+ctrl+end delete"))
    x, y = element_bounds(form.title, "Press Me")
    assert_ok(desktop("click", "-Window", form.title, "-X", x, "-Y", y))
    last = read_run_state()
    assert last["operator_tick"] == first["operator_tick"] == planted["operator_tick"], \
        "his last input is never replaced by the tool's"
    assert last["own_tick"] != first["own_tick"], "the tool's own tick follows its own input"
    assert last["run_start_ms"] == planted["run_start_ms"], "rewriting the state does not extend the run"
    form.wait_for(lambda s: s["pressed"] == 1 and s["text"] == "")


def test_the_first_command_of_a_run_records_the_foreground_and_pointer_and_later_ones_keep_them(form):
    # A run of its own. The tests' own input has just reset the clock, which the plain rule would count against the
    # first command, so wait until the clock has been still for 3 s and let the tool's guard check 3 s itself.
    assert_ok(desktop("restore"))
    assert not STATE_FILE.exists()
    still_for()
    assert_ok(desktop("focus", "-Window", form.title, "-IdleSeconds", AWAY))
    recorded = read_run_state()
    assert set(recorded) == {"operator_tick", "own_tick", "run_start_tick", "run_start_ms", "foreground", "pointer"}
    assert isinstance(recorded["pointer"]["x"], int) and isinstance(recorded["pointer"]["y"], int)
    assert isinstance(recorded["foreground"]["handle"], int) and recorded["foreground"]["process"]
    assert_ok(desktop("focus", "-Window", form.title, "-IdleSeconds", AWAY))
    kept = read_run_state()
    assert kept["foreground"] == recorded["foreground"] and kept["pointer"] == recorded["pointer"]
    assert kept["run_start_ms"] == recorded["run_start_ms"], "only the first command of a run records them"


def test_invoke_by_pattern_is_gated_records_state_and_does_not_lock_out_the_next_command(wpf_form):
    before = STATE_FILE.stat().st_mtime_ns
    result = desktop("invoke", "-Window", wpf_form.title, "-Name", "Press Me")
    assert_ok(result)
    assert "via=invoke" in result.stdout
    assert STATE_FILE.stat().st_mtime_ns > before
    assert_ok(desktop("focus", "-Window", wpf_form.title))
    wpf_form.wait_for(lambda s: s["pressed"] == 1)


def test_the_trace_says_whether_each_call_moved_the_clock_and_is_silent_otherwise(form):
    without_trace = {k: v for k, v in run_env().items() if k != "AF_DESKTOP_TRACE"}
    quiet = desktop("focus", "-Window", form.title, env=without_trace)
    assert_ok(quiet)
    assert quiet.stderr == "", "nothing extra is printed without a trace"
    traced = desktop("focus", "-Window", form.title, env={**run_env(), "AF_DESKTOP_TRACE": "1"})
    assert_ok(traced)
    record_trace("focus", traced.stderr)
    assert re.search(r"^trace: SetForegroundWindow clock_moved=(True|False) ms=\d+ accepted=(True|False)$", traced.stderr, re.M), \
        traced.stderr
    verbose = desktop("focus", "-Window", form.title, "-Verbose")
    assert_ok(verbose)
    assert "trace: SetForegroundWindow clock_moved=" in verbose.stderr, verbose.stderr
    # What the lines say (does SetForegroundWindow move the clock at all?) is what a person learns from this run.
    print("TRACE focus:", traced.stderr.strip())


def test_the_pattern_invoke_trace_is_recorded_for_the_operator_to_read(wpf_form):
    """Does a pattern Invoke move the last-input clock? The answer is in the trace line this test writes to the file
    named by AF_DESKTOP_TRACE_OUT, when that is set (the trace is switched on for this one call only)."""
    result = desktop("invoke", "-Window", wpf_form.title, "-Name", "Press Me", env={**run_env(), "AF_DESKTOP_TRACE": "1"})
    assert_ok(result)
    assert "via=invoke" in result.stdout
    lines = [line for line in result.stderr.splitlines() if line.startswith("trace:")]
    assert any(re.fullmatch(r"trace: InvokePattern clock_moved=(True|False) ms=\d+ accepted=(True|False)", line) for line in lines), \
        result.stderr
    record_trace("invoke pattern (WPF Button)", result.stderr)
    wpf_form.wait_for(lambda s: s["pressed"] == 1)


@pytest.mark.parametrize("damage", ["wrong own tick", "clocks that disagree", "run older than 5 minutes", "unreadable"])
def test_a_stale_run_state_falls_back_to_the_plain_rule(form, damage):
    assert_ok(desktop("type", "-Window", form.title, "-Text", "a"))
    saved = STATE_FILE.read_text(encoding="utf-8")
    data = json.loads(saved)
    assert data["own_tick"] is not None
    if damage == "wrong own tick":
        broken = {**data, "own_tick": data["own_tick"] + 1}
    elif damage == "clocks that disagree":
        broken = {**data, "run_start_tick": (data["run_start_tick"] - 60_000) % WRAP}
    elif damage == "run older than 5 minutes":
        six_minutes = 6 * 60_000
        broken = {**data, "run_start_ms": data["run_start_ms"] - six_minutes,
                  "run_start_tick": (data["run_start_tick"] - six_minutes) % WRAP}
    else:
        broken = None
    STATE_FILE.write_text("{not json" if broken is None else json.dumps(broken), encoding="utf-8")
    try:
        result = desktop("type", "-Window", form.title, "-Text", "b", expect_active=True)
        assert_active(result)
        # The tool's own input a moment ago is now plain input: the idle time is seconds, not his two minutes.
        time.sleep(0.2)
        assert form.state()["text"] == "a"
    finally:
        STATE_FILE.write_text(saved, encoding="utf-8")
    assert_ok(desktop("type", "-Window", form.title, "-Text", "c"))
    form.wait_for(lambda s: s["text"] == "ac")


def test_the_native_code_is_compiled_once_and_cached_by_a_hash_of_its_source(form):
    assert_ok(desktop("focus", "-Window", form.title))
    cached = sorted(p.name for p in STATE_FILE.parent.glob("AfDesktop-*.dll"))
    assert len(cached) == 1 and re.fullmatch(r"AfDesktop-[0-9a-f]{16}\.dll", cached[0]), cached
    started = time.monotonic()
    assert_ok(desktop("focus", "-Window", form.title))
    print("TIMING one focus with the cache:", round(time.monotonic() - started, 2), "s")
    assert sorted(p.name for p in STATE_FILE.parent.glob("AfDesktop-*.dll")) == cached


def test_a_second_screen_command_while_one_is_sending_is_refused_and_sends_nothing(form):
    text = "a" * 400
    check_away()
    first = track(subprocess.Popen([*POWERSHELL, "-File", str(SCRIPT), "type", "-Window", form.title, "-Text", text],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                   creationflags=NO_WINDOW, env=run_env()))
    try:
        form.wait_for(lambda s: len(s["text"]) > 5, timeout=40)
        # watch=False: the first command's own typing moves the clock all the time.
        second = desktop("key", "-Window", form.title, "-Keys", "b", watch=False)
        assert second.returncode == BUSY, f"{second.returncode}: {second.stderr}"
        assert "another command that acts on the screen is running; nothing was sent" in second.stderr
    finally:
        out, err = first.communicate(timeout=120)
    adopt_after_background(first)
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


# Live: restore. These come after the guard tests: each ends a run, and each starts a new one after 3 s of stillness
# with -IdleSeconds 3 (the plain rule would refuse a start right after the tests' own input).

def start_new_run():
    assert_ok(desktop("restore"))  # ends the planted run; the tool's own-tick rule lets it through
    assert not STATE_FILE.exists()
    still_for()


def bring_to_front_in_a_scratch_run(title, tmp_path):
    scratch = tmp_path / f"scratch-{uuid.uuid4().hex}"
    assert_ok(desktop("focus", "-Window", title, "-IdleSeconds", AWAY, env=run_env(scratch)))
    assert wait_until(lambda: foreground_title() == title), f"could not set up: {foreground_title()!r} is in front"


def test_restore_puts_the_pointer_and_the_foreground_back_and_removes_the_state(live, tmp_path):
    token = uuid.uuid4().hex
    prior, target = Fixture(tmp_path, f"AF prior {token}"), Fixture(tmp_path, f"AF target {token}", left=800)
    try:
        start_new_run()
        bring_to_front_in_a_scratch_run(prior.title, tmp_path)
        move_pointer(120, 140)
        assert pointer() == (120, 140)
        assert_ok(desktop("focus", "-Window", target.title, "-IdleSeconds", AWAY))
        assert STATE_FILE.exists() and read_run_state()["pointer"] == {"x": 120, "y": 140}
        x, y = element_bounds(target.title, "Press Me")
        assert_ok(desktop("click", "-Window", target.title, "-X", x, "-Y", y, "-IdleSeconds", AWAY))
        target.wait_for(lambda s: s["pressed"] == 1)
        assert pointer() != (120, 140) and foreground_title() == target.title
        result = desktop("restore", "-IdleSeconds", AWAY)
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
        move_pointer(150, 170)
        assert_ok(desktop("focus", "-Window", target.title, "-IdleSeconds", AWAY))
        prior.close()
        x, y = element_bounds(target.title, "Press Me")
        assert_ok(desktop("click", "-Window", target.title, "-X", x, "-Y", y, "-IdleSeconds", AWAY))
        result = desktop("restore", "-IdleSeconds", AWAY)
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
    result = desktop("restore")  # no run is recorded, so there is nothing to gate
    assert_ok(result)
    assert "nothing to restore" in result.stdout
    assert pointer() == before


def test_the_run_state_is_under_the_checkout_when_af_root_is_not_set(form):
    without = {k: v for k, v in os.environ.items() if k != "AF_ROOT"}
    default_state = ROOT / "system" / "logs" / "desktop" / "run.json"
    assert not default_state.exists(), "an earlier run left its state; it is git-ignored, delete it"
    still_for()
    try:
        assert_ok(desktop("focus", "-Window", form.title, "-IdleSeconds", AWAY, env=without))
        assert default_state.exists()
    finally:
        desktop("restore", "-IdleSeconds", AWAY, env=without)
    assert not default_state.exists()


# The last test: it makes a change of the clock that is "his" on purpose, so nothing may run after it that needs the
# tool's default guard.

MOUSEEVENTF_MOVE = 1


def test_his_input_in_the_middle_of_a_type_stops_it_with_exit_4_and_the_count(form):
    text = "a" * 400
    check_away()
    first = track(subprocess.Popen([*POWERSHELL, "-File", str(SCRIPT), "type", "-Window", form.title, "-Text", text],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                   creationflags=NO_WINDOW, env=run_env()))
    try:
        form.wait_for(lambda s: len(s["text"]) > 5, timeout=40)
        # A zero-distance mouse move is input like any other as far as the last-input clock is concerned.
        win32().mouse_event(MOUSEEVENTF_MOVE, 0, 0, 0, 0)
    finally:
        out, err = first.communicate(timeout=120)
    Watch.expected = last_input_tick()
    Watch.operator_tick = Watch.expected  # from here the clock is "his"
    assert first.returncode == ACTIVE, f"{first.returncode}: {out} {err}"
    match = re.search(r"the operator is active \(last input \d+ s ago\); (\d+) of 400 characters were sent; wait and retry", err)
    assert match, err
    sent = int(match.group(1))
    assert 0 < sent < 400
    time.sleep(0.3)
    assert len(form.state()["text"]) == sent, "nothing more was typed after he touched the machine"
    # The state was left alone, so the next command finds a last input that is not the tool's own and waits.
    again = desktop("focus", "-Window", form.title, watch=False)
    assert_active(again)