"""system/tools/desktop.ps1 and its doc: every check here runs without a desktop and touches no window.

Nothing here runs the script. Two checks start a throwaway PowerShell: one only parses the script, one only
compiles its embedded C# (nothing in it is called), and the guard's arithmetic and state-file functions are
loaded one by one from the script text and run on numbers and temp files. Those functions call no Windows
API, which a test below checks. The live tests, which open windows and send input, are test_desktop_tool.py.
"""

import json
import re
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "system" / "tools" / "desktop.ps1"
DOC = ROOT / "library" / "process" / "desktop-tool.md"
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)
POWERSHELL = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass"]
needs_powershell = pytest.mark.skipif(sys.platform != "win32" or not shutil.which("powershell"),
                                      reason="Windows PowerShell is needed")

# The functions the harness may load. None calls Windows; test_the_loaded_functions_call_no_windows_api checks it.
PURE = ["Get-TickAge", "Get-StateField", "New-RunState", "Read-RunState", "Write-RunState", "Test-RunStateFresh",
        "Get-OperatorIdleMs", "Test-RunContinues", "Get-ShownTitle", "Get-ExposedDeniedCount"]

HARNESS = r"""
param([string]$Script, [string]$Names, [string]$CasesFile)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Script, [ref]$null, [ref]$null)
$wanted = $Names -split ','
foreach ($d in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $wanted -contains $n.Name }, $true)) {
    . ([scriptblock]::Create($d.Extent.Text))
}
$cases = [System.IO.File]::ReadAllText($CasesFile) | ConvertFrom-Json
$results = foreach ($c in $cases) {
    $argList = @($c.args)
    try { $v = & $c.fn @argList; [pscustomobject]@{ id = $c.id; ok = $true; value = $v } }
    catch { [pscustomobject]@{ id = $c.id; ok = $false; value = $_.Exception.Message } }
}
ConvertTo-Json -InputObject @($results) -Depth 6 -Compress
"""

WRITER = r"""
param([string]$Script, [string]$Path, [int]$Id, [int]$Count)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Script, [ref]$null, [ref]$null)
foreach ($d in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and @('Get-StateField', 'New-RunState', 'Write-RunState') -contains $n.Name }, $true)) {
    . ([scriptblock]::Create($d.Extent.Text))
}
$failed = 0
for ($i = 0; $i -lt $Count; $i++) {
    $state = New-RunState $Id $null (1000 + $i) (5000 + $i) ([ordered]@{ handle = $Id; pid = $i; process = ('x' * 200) }) ([ordered]@{ x = $i; y = $Id })
    try { Write-RunState $Path $state } catch { $failed++ }
}
"failed=$failed"
"""

T0 = 1_000_000_000_000  # a wall-clock time in ms
WRAP = 4294967296


def powershell(*args, timeout=120):
    return subprocess.run([*POWERSHELL, *map(str, args)], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=timeout, creationflags=NO_WINDOW)


def run_functions(folder: Path, cases: list[dict], names=PURE) -> dict:
    harness = folder / "harness.ps1"
    harness.write_text(HARNESS, encoding="utf-8")
    cases_file = folder / "cases.json"
    cases_file.write_text(json.dumps(cases), encoding="utf-8")
    out = powershell("-File", harness, "-Script", SCRIPT, "-Names", ",".join(names), "-CasesFile", cases_file)
    assert out.returncode == 0, out.stderr + out.stdout
    rows = json.loads(out.stdout)
    return {row["id"]: row for row in rows}


def state(operator_tick=10_000, tick_at_write=50_000, written_ms=T0, own_tick=50_000, **extra):
    return {"operator_tick": operator_tick, "own_tick": own_tick, "tick_at_write": tick_at_write,
            "written_ms": written_ms, **extra}


def function_text(name: str) -> str:
    text = SCRIPT.read_text(encoding="utf-8")
    start = text.index(f"function {name}")
    nxt = re.search(r"^(function |# --- |\$script:Bound)", text[start + 10:], re.M)
    return text[start:start + 10 + nxt.start()] if nxt else text[start:]


# name -> (function, arguments, expected value)
GUARD_CASES = {
    # Unsigned tick arithmetic, across a wrap of the 32-bit clock.
    "age": ("Get-TickAge", [1000, 400], 600),
    "age zero": ("Get-TickAge", [5, 5], 0),
    "age across a wrap": ("Get-TickAge", [100, WRAP - 100], 200),
    # A state written 60 s ago: ticks and wall clock agree.
    "fresh": ("Test-RunStateFresh", [state(), 110_000, T0 + 60_000], True),
    "fresh at exactly 30 minutes": ("Test-RunStateFresh", [state(), 50_000 + 1_800_000, T0 + 1_800_000], True),
    "older than 30 minutes": ("Test-RunStateFresh", [state(), 50_000 + 1_800_001, T0 + 1_800_001], False),
    "written in the future": ("Test-RunStateFresh", [state(), 40_000, T0 - 10_000], False),
    "from before a reboot, ticks restarted small": ("Test-RunStateFresh", [state(), 20_000, T0 + 600_000], False),
    "from before a reboot, ticks restarted larger": ("Test-RunStateFresh", [state(), 80_000, T0 + 300_000], False),
    "written just before a wrap (a tick greater than now is not stale)": (
        "Test-RunStateFresh",
        [state(operator_tick=WRAP - 70_000, tick_at_write=WRAP - 300, own_tick=WRAP - 300), 304, T0 + 604], True),
    "his tick after the write": ("Test-RunStateFresh", [state(operator_tick=80_000), 110_000, T0 + 60_000], False),
    # His idle time: the tool's own last input does not count against him.
    "own input is the last: idle since his own": ("Get-OperatorIdleMs", [state(), 110_000, 50_000, T0 + 60_000], 100_000),
    "he touched it after the tool: plain rule": ("Get-OperatorIdleMs", [state(), 110_000, 90_000, T0 + 60_000], 20_000),
    "stale state: plain rule": (
        "Get-OperatorIdleMs", [state(), 50_000 + 1_900_000, 50_000, T0 + 1_900_000], 1_900_000),
    "no state: plain rule": ("Get-OperatorIdleMs", [None, 110_000, 100_000, T0], 10_000),
    "state with no own tick: plain rule": ("Get-OperatorIdleMs", [state(own_tick=None), 110_000, 50_000, T0 + 60_000], 60_000),
    "own input then a wrap": (
        "Get-OperatorIdleMs",
        [state(operator_tick=WRAP - 70_000, tick_at_write=WRAP - 300, own_tick=WRAP - 300), 304, WRAP - 300, T0 + 604],
        70_304),
    # Whether the run goes on (its recorded foreground and pointer are kept) or is replaced.
    "continues after the tool's own input": ("Test-RunContinues", [state(), 110_000, 50_000, T0 + 60_000], True),
    "continues before the tool sent any input": (
        "Test-RunContinues", [state(own_tick=None, operator_tick=10_000), 110_000, 10_000, T0 + 60_000], True),
    "replaced after his input": ("Test-RunContinues", [state(), 110_000, 90_000, T0 + 60_000], False),
    "replaced when stale": ("Test-RunContinues", [state(), 50_000 + 1_900_000, 50_000, T0 + 1_900_000], False),
    "no state, no run": ("Test-RunContinues", [None, 110_000, 50_000, T0], False),
    # Titles, and what a full-screen capture would hold.
    "a denied title is hidden": ("Get-ShownTitle", [True, "Acme Bank - Jane Doe - Savings"], "(off limits)"),
    "an allowed title is shown, control characters removed": (
        "Get-ShownTitle", [False, "Notes\nfake\trow"], "Notes fake row"),
    "a visible denied window blocks a capture": (
        "Get-ExposedDeniedCount",
        [[{"Denied": True, "Minimized": False}, {"Denied": True, "Minimized": True}, {"Denied": False, "Minimized": False}]],
        1),
    "a minimized denied window does not": (
        "Get-ExposedDeniedCount", [[{"Denied": True, "Minimized": True}, {"Denied": False, "Minimized": False}]], 0),
    "no windows": ("Get-ExposedDeniedCount", [[]], 0),
}


@pytest.fixture(scope="module")
def guard_results(tmp_path_factory):
    if sys.platform != "win32" or not shutil.which("powershell"):
        pytest.skip("Windows PowerShell is needed")
    cases = [{"id": name, "fn": fn, "args": args} for name, (fn, args, _) in GUARD_CASES.items()]
    return run_functions(tmp_path_factory.mktemp("guard"), cases)


@needs_powershell
@pytest.mark.parametrize("name", list(GUARD_CASES))
def test_the_guard_arithmetic_and_state_rules(guard_results, name):
    row = guard_results[name]
    assert row["ok"], row["value"]
    expected = GUARD_CASES[name][2]
    assert row["value"] == expected, f"{name}: expected {expected!r}, got {row['value']!r}"


# State file: round trip, bad files, atomic swap.

@needs_powershell
def test_a_state_file_round_trips_and_leaves_no_temp_file(tmp_path):
    path = tmp_path / "nested" / "run.json"
    foreground = {"handle": 66000, "pid": 4242, "process": "notepad"}
    # Build the state with the script's own New-RunState, then write, overwrite and read it back.
    made = run_functions(tmp_path, [{"id": "new", "fn": "New-RunState", "args": [7, 9, 100, 200, foreground, {"x": 5, "y": 6}]}])
    built = made["new"]["value"]
    cases = [{"id": "write", "fn": "Write-RunState", "args": [str(path), built]},
             {"id": "overwrite", "fn": "Write-RunState", "args": [str(path), {**built, "own_tick": 11}]},
             {"id": "read", "fn": "Read-RunState", "args": [str(path)]}]
    rows = run_functions(tmp_path, cases)
    assert all(row["ok"] for row in rows.values()), rows
    read = rows["read"]["value"]
    assert read["operator_tick"] == 7 and read["own_tick"] == 11 and read["tick_at_write"] == 100
    assert read["written_ms"] == 200 and read["foreground"] == foreground and read["pointer"] == {"x": 5, "y": 6}
    assert [p.name for p in path.parent.iterdir()] == ["run.json"], "the temp file is gone"
    assert json.loads(path.read_text(encoding="utf-8"))["own_tick"] == 11


@needs_powershell
@pytest.mark.parametrize("content", [
    None, "", "not json", "[1, 2]", "null", '{"operator_tick": 1}', '{"operator_tick": "x", "tick_at_write": 1, "written_ms": 1}',
    '{"operator_tick": 1, "tick_at_write": 1, "written_ms": 1, "own_tick": "soon"}',
])
def test_a_missing_or_bad_state_file_reads_as_no_state(tmp_path, content):
    path = tmp_path / "run.json"
    if content is not None:
        path.write_text(content, encoding="utf-8")
    row = run_functions(tmp_path, [{"id": "read", "fn": "Read-RunState", "args": [str(path)]}])["read"]
    assert row["ok"], row["value"]
    assert row["value"] is None


@needs_powershell
def test_two_writers_never_leave_a_half_written_state_file(tmp_path):
    path = tmp_path / "run.json"
    writer = tmp_path / "writer.ps1"
    writer.write_text(WRITER, encoding="utf-8")
    count = 120
    procs = [subprocess.Popen([*POWERSHELL, "-File", str(writer), "-Script", str(SCRIPT), "-Path", str(path),
                               "-Id", str(i), "-Count", str(count)], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              text=True, creationflags=NO_WINDOW) for i in (1, 2)]
    seen, bad = 0, []
    deadline = time.monotonic() + 120
    while any(p.poll() is None for p in procs) and time.monotonic() < deadline:
        try:
            text = path.read_text(encoding="utf-8")
        except OSError:
            continue  # the file is being swapped in: a reader may be refused, never shown half of it
        seen += 1
        try:
            data = json.loads(text)
            assert {"operator_tick", "own_tick", "tick_at_write", "written_ms", "foreground", "pointer"} <= set(data)
            assert data["operator_tick"] in (1, 2) and len(data["foreground"]["process"]) == 200
        except (ValueError, AssertionError, KeyError) as exc:
            bad.append((text[:80], repr(exc)))
    outputs = [p.communicate(timeout=60) for p in procs]
    assert not bad, bad[:3]
    assert seen > 10, "the reader saw the file often enough to mean something"
    for out, err in outputs:
        assert out.strip() == "failed=0", (out, err)
    final = json.loads(path.read_text(encoding="utf-8"))
    assert final["operator_tick"] in (1, 2)
    # Windows leaves a run.json~RF*.TMP file when a replaced file was still open in the reader above; the real
    # tool never reads while another command writes (one gated command at a time), so only this tool's own temp
    # file name counts.
    assert [p.name for p in tmp_path.iterdir() if re.fullmatch(r"run\.json\.\d+\.tmp", p.name)] == []


# The guard's whole flow (lock, state file, Enter-AwayGuard, Complete-Run, Invoke-Restore) run against a fake
# [AfDesktop] class whose clocks, pointer and windows the test scripts. The fake calls no Windows API: the real
# class is never compiled in this process, and no window, input or cursor is touched.

FLOW_HARNESS = r"""
param([string]$Script, [string]$StepsFile, [string]$Root, [string]$MutexName)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
public static class AfDesktop {
    public static long Last, Now, Ms, Front = 777, Alive = 777, OwnTick = -2;
    public static int PX = 10, PY = 20, CursorX = -1, CursorY = -1;
    public static bool CursorOk = true, FrontOk = true;
    public static long LastInput() { return Last; }
    public static long TickNow() { return Now; }
    public static IntPtr GetForegroundWindow() { return new IntPtr(Front); }
    public static int[] Pointer() { return new int[] { PX, PY }; }
    public static bool SetCursorPos(int x, int y) { if (!CursorOk) return false; CursorX = x; CursorY = y; return true; }
    public static bool Exists(IntPtr h) { return h.ToInt64() == Alive; }
    public static uint ProcessId(IntPtr h) { return 4242; }
    public static bool IsIconic(IntPtr h) { return false; }
    public static bool ShowWindow(IntPtr h, int c) { return true; }
    public static bool BringToFront(IntPtr h) { if (FrontOk) Front = h.ToInt64(); return FrontOk; }
    public static bool InputDesktopAvailable() { return true; }
}
'@
$names = @('Stop-Error', 'Stop-Active', 'Get-TickAge', 'Get-StateField', 'New-RunState', 'Read-RunState', 'Write-RunState',
    'Test-RunStateFresh', 'Get-OperatorIdleMs', 'Test-RunContinues', 'Get-StatePath', 'Lock-Run', 'Get-AwayStatus',
    'Get-ForegroundRecord', 'Get-PointerRecord', 'Enter-AwayGuard', 'Save-Run', 'Complete-Run', 'Remove-RunState',
    'Assert-InputDesktop', 'Invoke-Restore')
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Script, [ref]$null, [ref]$null)
foreach ($d in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $names -contains $n.Name }, $true)) {
    . ([scriptblock]::Create($d.Extent.Text))
}
function Get-NowMs { [AfDesktop]::Ms }
function Get-WindowInfo($Handle) { [pscustomobject]@{ ProcessId = 4242; Process = 'fake' } }
function Exit-Tool([int]$Code) { Complete-Run; throw "EXIT:$Code" }
$env:AF_ROOT = $Root
$StateRelative = 'system\logs\desktop\run.json'
$RunMutexName = $MutexName
$SettleMs = 0
$script:Guard = $null
$script:RunLock = $null
$IdleSeconds = 120

$holder = $null
$holderHandle = $null
$originalError = [Console]::Error
$release = New-Object System.Threading.ManualResetEvent($false)
$acquired = New-Object System.Threading.ManualResetEvent($false)

function Set-Fake($Values) {
    if ($null -eq $Values) { return }
    foreach ($p in $Values.PSObject.Properties) {
        $field = [AfDesktop].GetField($p.Name)
        $field.SetValue($null, [Convert]::ChangeType($p.Value, $field.FieldType))
    }
}

$statePath = Join-Path $Root 'system\logs\desktop\run.json'
$steps = [System.IO.File]::ReadAllText($StepsFile) | ConvertFrom-Json
$results = New-Object System.Collections.ArrayList
foreach ($s in $steps) {
    # Each step is a new process of the tool: it has sent nothing yet.
    [AfDesktop]::OwnTick = -2
    Set-Fake $s.set
    $IdleSeconds = if ($null -ne (Get-StateField $s 'idle')) { [int]$s.idle } else { 120 }
    if ((Get-StateField $s 'hold_lock') -eq $false -and $null -ne $holder) {
        [void]$release.Set()
        $holder.EndInvoke($holderHandle)
        $holder = $null
    }
    if ((Get-StateField $s 'hold_lock') -eq $true -and $null -eq $holder) {
        $holder = [powershell]::Create().AddScript({
            param($name, $release, $acquired)
            $m = New-Object System.Threading.Mutex($false, $name)
            [void]$m.WaitOne()
            [void]$acquired.Set()
            [void]$release.WaitOne()
            $m.ReleaseMutex()
        }).AddArgument($MutexName).AddArgument($release).AddArgument($acquired)
        $holderHandle = $holder.BeginInvoke()
        [void]$acquired.WaitOne(10000)
    }
    $sw = New-Object System.IO.StringWriter
    [Console]::SetError($sw)
    $exit = 0
    $out = New-Object System.Collections.ArrayList
    try {
        switch ($s.run) {
            'guard' { Enter-AwayGuard }
            'restore' { Invoke-Restore | ForEach-Object { [void]$out.Add([string]$_) } }
        }
        Set-Fake (Get-StateField $s 'after')
        $code = if ($null -ne (Get-StateField $s 'exit')) { [int]$s.exit } else { 0 }
        Exit-Tool $code
    } catch {
        if ($_.Exception.Message -like 'EXIT:*') { $exit = [int]($_.Exception.Message.Substring(5)) } else { throw }
    }
    [Console]::SetError($originalError)
    $state = $null
    if ([System.IO.File]::Exists($statePath)) { $state = [System.IO.File]::ReadAllText($statePath) | ConvertFrom-Json }
    [void]$results.Add([pscustomobject]@{
        name = $s.name; exit = $exit; stderr = $sw.ToString(); stdout = @($out); state = $state
        files = @(Get-ChildItem (Split-Path $statePath) -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
        cursor = @([AfDesktop]::CursorX, [AfDesktop]::CursorY); front = [AfDesktop]::Front
    })
}
if ($null -ne $holder) { [void]$release.Set() }
ConvertTo-Json -InputObject @($results) -Depth 8 -Compress
"""

T = 10_000_000  # a tick
M = 2_000_000_000_000  # a wall-clock time in ms


def run_flow(folder: Path, steps: list[dict]) -> list[dict]:
    harness = folder / "flow.ps1"
    harness.write_text(FLOW_HARNESS, encoding="utf-8")
    steps_file = folder / "steps.json"
    steps_file.write_text(json.dumps(steps), encoding="utf-8")
    root = folder / "root"
    root.mkdir()
    out = powershell("-File", harness, "-Script", SCRIPT, "-StepsFile", steps_file, "-Root", root,
                     "-MutexName", f"Local\\AfDesktopToolTest{uuid.uuid4().hex}")
    assert out.returncode == 0, out.stderr + out.stdout
    return json.loads(out.stdout)


def guard_step(name, now, last, ms, *, idle=None, after=None, exit=None, **fake):
    step = {"name": name, "run": "guard", "set": {"Now": now, "Last": last, "Ms": ms, **fake}}
    if idle is not None:
        step["idle"] = idle
    if after is not None:
        step["after"] = after
    if exit is not None:
        step["exit"] = exit
    return step


@needs_powershell
def test_a_run_keeps_his_tick_and_its_first_record_across_commands(tmp_path):
    first, second, his_input, long_away = run_flow(tmp_path, [
        # He has been away 10 minutes. The command sends input, so the last-input tick becomes the tool's own.
        guard_step("first", T, T - 600_000, M, Front=777, PX=10, PY=20,
                   after={"OwnTick": T + 50, "Last": T + 50, "Now": T + 60, "Ms": M + 60}),
        # Two seconds later the last input is still the tool's own: allowed, and the first record is kept.
        guard_step("second", T + 2000, T + 50, M + 2000, Front=999, PX=500, PY=600,
                   after={"OwnTick": T + 2100, "Last": T + 2100, "Now": T + 2110, "Ms": M + 2110}),
        # He moves the mouse a second ago.
        guard_step("he comes back", T + 5000, T + 4000, M + 5000),
        # Later he has been still for 130 s: the plain rule passes, and the run starts over.
        guard_step("a new run", T + 134_000, T + 4000, M + 134_000, Front=555, PX=1, PY=2),
    ])
    assert first["exit"] == 0 and first["files"] == ["run.json"]
    assert first["state"]["operator_tick"] == T - 600_000
    assert first["state"]["own_tick"] == T + 50 and first["state"]["tick_at_write"] == T + 60
    assert first["state"]["foreground"] == {"handle": 777, "pid": 4242, "process": "fake"}
    assert first["state"]["pointer"] == {"x": 10, "y": 20}

    assert second["exit"] == 0
    assert second["state"]["operator_tick"] == T - 600_000, "his tick is kept"
    assert second["state"]["own_tick"] == T + 2100, "the tool's own tick follows its own input"
    assert second["state"]["foreground"]["handle"] == 777 and second["state"]["pointer"] == {"x": 10, "y": 20}, \
        "the first command's record is kept"

    assert his_input["exit"] == 4
    assert "the operator is active (last input 1 s ago); nothing was sent; wait and retry" in his_input["stderr"]
    assert his_input["state"] == second["state"], "a refusal changes nothing"

    assert long_away["exit"] == 0
    assert long_away["state"]["operator_tick"] == T + 4000
    assert long_away["state"]["own_tick"] is None
    assert long_away["state"]["foreground"]["handle"] == 555 and long_away["state"]["pointer"] == {"x": 1, "y": 2}


@needs_powershell
def test_the_idle_threshold_is_inclusive_and_uses_the_given_seconds(tmp_path):
    just_short, exact, short_option = run_flow(tmp_path, [
        guard_step("119.999 s", T, T - 119_999, M),
        guard_step("120 s", T, T - 120_000, M),
        guard_step("3 s with -IdleSeconds 3", T + 10_000, T + 7000, M + 10_000, idle=3),
    ])
    assert just_short["exit"] == 4 and "last input 119 s ago" in just_short["stderr"]
    assert just_short["state"] is None and just_short["files"] == []
    assert exact["exit"] == 0
    assert short_option["exit"] == 0


@needs_powershell
def test_a_command_that_stops_partway_still_records_its_own_input(tmp_path):
    refused, next_command = run_flow(tmp_path, [
        guard_step("type, refused after 40 characters", T, T - 600_000, M, exit=3,
                   after={"OwnTick": T + 300, "Last": T + 300, "Now": T + 310}),
        guard_step("the next command", T + 1000, T + 300, M + 1000),
    ])
    assert refused["exit"] == 3 and refused["state"]["own_tick"] == T + 300
    assert next_command["exit"] == 0, "the tool's partial input does not lock it out"


@needs_powershell
def test_a_command_with_no_injected_input_takes_a_tick_that_moved_as_its_own(tmp_path):
    moved, unmoved, after_moved = run_flow(tmp_path, [
        guard_step("invoke by pattern, the clock moved", T, T - 600_000, M, after={"Last": T + 80, "Now": T + 90}),
        guard_step("focus, nothing moved", T + 500, T + 80, M + 500),
        guard_step("the next command", T + 800, T + 80, M + 800),
    ])
    assert moved["state"]["own_tick"] == T + 80
    assert unmoved["exit"] == 0 and unmoved["state"]["own_tick"] == T + 80
    assert after_moved["exit"] == 0


@needs_powershell
def test_a_command_that_moved_no_tick_and_sent_nothing_records_no_own_tick(tmp_path):
    only, = run_flow(tmp_path, [guard_step("focus", T, T - 600_000, M)])
    assert only["exit"] == 0 and only["state"]["own_tick"] is None
    assert only["state"]["operator_tick"] == T - 600_000


@needs_powershell
def test_an_unreadable_clock_fails_closed_and_writes_nothing(tmp_path):
    only, = run_flow(tmp_path, [guard_step("no clock", T, -1, M)])
    assert only["exit"] == 1 and "would not say when the operator last used the computer" in only["stderr"]
    assert only["state"] is None


@needs_powershell
def test_a_second_command_while_one_holds_the_lock_exits_1_and_changes_nothing(tmp_path):
    held, released = run_flow(tmp_path, [
        {**guard_step("held", T, T - 600_000, M), "hold_lock": True},
        {**guard_step("released", T, T - 600_000, M), "hold_lock": False},
    ])
    assert held["exit"] == 1
    assert "another command that acts on the screen is running; nothing was sent; retry when it has ended" in held["stderr"]
    assert held["state"] is None and held["files"] == []
    assert released["exit"] == 0, "once the other command has ended the next one runs"


@needs_powershell
def test_the_lock_is_released_when_a_command_ends_so_the_next_can_run(tmp_path):
    steps = [guard_step(f"command {i}", T + i * 100, T - 600_000, M + i * 100) for i in range(3)]
    steps.insert(1, guard_step("he is active", T + 50, T + 40, M + 50))
    results = run_flow(tmp_path, steps)
    assert [r["exit"] for r in results] == [0, 4, 0, 0]


def restore_steps(**flags):
    start = guard_step("start", T, T - 600_000, M, Front=777, PX=10, PY=20,
                       after={"OwnTick": T + 50, "Last": T + 50, "Now": T + 60, "Ms": M + 60})
    return [start, {"name": "restore", "run": "restore",
                    "set": {"Now": T + 3000, "Last": T + 50, "Ms": M + 3000, "Front": 999, **flags}}]


@needs_powershell
def test_restore_puts_the_pointer_and_the_foreground_back_and_deletes_the_state(tmp_path):
    _, restored = run_flow(tmp_path, restore_steps())
    assert restored["exit"] == 0
    assert restored["cursor"] == [10, 20] and restored["front"] == 777
    assert restored["stdout"] == ["restored the pointer to 10,20", "restored the foreground window (process 'fake')"]
    assert restored["state"] is None and restored["files"] == []


@needs_powershell
def test_restore_reports_a_foreground_window_that_is_gone_and_still_exits_0(tmp_path):
    _, gone = run_flow(tmp_path, restore_steps(Alive=0))
    assert gone["exit"] == 0
    assert gone["cursor"] == [10, 20], "the pointer is still put back"
    assert gone["stdout"][1] == "could not restore the foreground window (process 'fake'): it is gone"
    assert gone["front"] == 999 and gone["state"] is None


@needs_powershell
def test_restore_exits_1_and_says_so_when_windows_refuses(tmp_path):
    _, refused = run_flow(tmp_path, restore_steps(FrontOk=False, CursorOk=False))
    assert refused["exit"] == 1
    assert refused["stdout"] == ["could not restore the pointer: Windows rejected the move to 10,20",
                                 "could not restore the foreground window (process 'fake'): Windows kept another window in front"]
    assert refused["state"] is None, "the run ends either way"


@needs_powershell
def test_restore_is_gated_and_leaves_the_state_and_the_screen_alone_while_he_is_active(tmp_path):
    steps = restore_steps()
    steps[1]["set"]["Last"] = T + 2500  # he touched the machine half a second ago
    start, active = run_flow(tmp_path, steps)
    assert active["exit"] == 4 and "the operator is active (last input 0 s ago)" in active["stderr"]
    assert active["state"] == start["state"], "the run is left as it was"
    assert active["cursor"] == [-1, -1] and active["front"] == 999, "nothing was moved"


@needs_powershell
def test_restore_with_no_run_or_an_old_one_has_nothing_to_restore_and_removes_the_file(tmp_path):
    results = run_flow(tmp_path, [
        {"name": "none", "run": "restore", "set": {"Now": T, "Last": T - 1000, "Ms": M}},
        guard_step("start", T, T - 600_000, M, after={"OwnTick": T + 50, "Last": T + 50, "Now": T + 60, "Ms": M + 60}),
        {"name": "31 minutes later", "run": "restore",
         "set": {"Now": T + 60 + 31 * 60_000, "Last": T + 50, "Ms": M + 60 + 31 * 60_000}},
    ])
    none, old = results[0], results[2]
    assert none["exit"] == 0 and none["stdout"] == ["nothing to restore: no run is recorded, or its record is more than 30 minutes old"]
    assert old["exit"] == 0 and old["stdout"][0].startswith("nothing to restore") and old["state"] is None
    assert old["cursor"] == [-1, -1]


# The script's text.

def test_script_is_ascii_because_windows_powershell_misreads_utf8_without_a_bom():
    assert SCRIPT.read_bytes().isascii()


def test_script_downloads_nothing():
    text = SCRIPT.read_text(encoding="utf-8")
    for banned in ("Invoke-WebRequest", "Invoke-RestMethod", "Install-Module", "DownloadFile", "Start-BitsTransfer", "curl "):
        assert banned not in text


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


def test_the_header_names_every_command_exit_code_4_and_the_idle_default():
    text = SCRIPT.read_text(encoding="utf-8")
    header = text[:text.index("[CmdletBinding")]
    assert "restore" in header
    assert re.search(r"4 the operator is active: nothing was sent, wait and retry", header)
    assert "-IdleSeconds" in header and "default 120" in header
    assert re.search(r"\[int\]\$IdleSeconds = 120\b", text)
    assert "'restore'" in text[text.index("$Command -notin"):text.index("$Command -notin") + 200]


GATED = {"focus": "Invoke-Focus", "invoke": "Invoke-Element", "click": "Invoke-Click", "type": "Invoke-Type",
         "key": "Invoke-Key"}
FIRST_CHANGE = ("Invoke-BringToFront", "Invoke-GuardedClick", ".Invoke()", "SendChord", "SendChar", "SetCursorPos",
                "ShowWindow", "BringToFront(", "Remove-RunState")


def test_the_gated_commands_are_the_documented_six():
    text = SCRIPT.read_text(encoding="utf-8")
    gated = re.search(r"\$GatedCommands = @\(([^)]*)\)", text).group(1)
    assert set(re.findall(r"'(\w+)'", gated)) == {"focus", "invoke", "click", "type", "key", "restore"}


@pytest.mark.parametrize("command", list(GATED))
def test_every_gated_command_enters_the_guard_before_its_first_change(command):
    body = function_text(GATED[command])
    assert "Enter-AwayGuard" in body, command
    first = min(body.index(marker) for marker in FIRST_CHANGE if marker in body)
    assert body.index("Enter-AwayGuard") < first, f"{command} changes something before the guard runs"
    assert "Resolve-Target" in body and body.index("Resolve-Target") < body.index("Enter-AwayGuard"), \
        "a refusal or a bad match is final whoever is at the machine, so it comes before the wait"


def test_restore_is_gated_and_changes_nothing_before_it_has_checked():
    body = function_text("Invoke-Restore")
    gate = body.index("Stop-Active")
    assert body.index("Lock-Run") < body.index("Get-AwayStatus") < gate
    for marker in ("SetCursorPos", "BringToFront(", "ShowWindow"):
        assert body.index(marker) > gate, marker
    assert "Remove-RunState" in body


@pytest.mark.parametrize("name", ["Invoke-Windows", "Invoke-Tree", "Invoke-Screenshot"])
def test_the_read_only_commands_are_not_gated(name):
    body = function_text(name)
    assert "Enter-AwayGuard" not in body and "Lock-Run" not in body


def test_every_exit_goes_through_exit_tool_so_a_partial_run_still_records_its_input():
    text = SCRIPT.read_text(encoding="utf-8")
    code = [line.strip() for line in text.splitlines() if not line.lstrip().startswith("#")]
    exits = [line for line in code if re.search(r"(?:^|[;{(]\s*)exit\b", line)]
    assert exits == ["exit $Code"], exits
    for name in ("Stop-Refused", "Stop-Failed", "Stop-Error", "Stop-Active"):
        assert "Exit-Tool" in function_text(name), name
    assert function_text("Exit-Tool").index("Complete-Run") < function_text("Exit-Tool").index("exit ")
    assert "Stop-Active" in function_text("Enter-AwayGuard")
    assert re.search(r"4\b", function_text("Stop-Active"))


def test_the_own_tick_is_read_right_after_the_tools_own_send():
    source = SCRIPT.read_text(encoding="utf-8")
    send = source[source.index("static bool Send(INPUT[] events)"):]
    send = send[:send.index("}") + 1]
    assert send.index("SendInput(") < send.index("OwnTick = LastInput()") < send.index("return sent")
    assert "if (!GetLastInputInfo(ref l)) return -1;" in source, "an unreadable clock fails closed"
    complete = function_text("Complete-Run")
    assert "[AfDesktop]::OwnTick" in complete


def test_a_full_screen_capture_is_refused_before_it_is_taken_and_no_longer_only_warns():
    body = function_text("Invoke-Screenshot")
    assert "Get-ExposedDeniedCount" in body and "Stop-Refused" in body
    assert body.index("Stop-Refused") < body.index("CopyFromScreen")
    assert "-Window to capture one window" in body
    assert "warning:" not in body


def test_titles_of_off_limits_windows_are_hidden_in_the_list_and_in_messages():
    assert "Get-ShownTitle" in function_text("Invoke-Windows")
    assert "Format-Text $info.Title" not in function_text("Invoke-Windows")
    assert "Get-ShownTitle" in function_text("Format-Window")


def test_the_loaded_functions_call_no_windows_api():
    for name in PURE:
        body = function_text(name)
        for banned in ("AfDesktop", "Add-Type", "SendInput", "SetCursorPos", "Get-Process", "SetForeground"):
            assert banned not in body, (name, banned)


def test_the_run_state_path_is_gitignored():
    relative = re.search(r"\$StateRelative = '([^']+)'", SCRIPT.read_text(encoding="utf-8")).group(1).replace("\\", "/")
    assert relative == "system/logs/desktop/run.json"
    if not shutil.which("git"):
        pytest.skip("git is not on PATH")
    result = subprocess.run(["git", "check-ignore", "-q", relative], cwd=ROOT, capture_output=True)
    assert result.returncode == 0, "the run state holds a pointer position and window handles; it is never tracked"


@needs_powershell
def test_the_script_parses():
    code = ("$e=$null; [void][System.Management.Automation.Language.Parser]::ParseFile("
            f"'{SCRIPT}', [ref]$null, [ref]$e); $e | ForEach-Object {{ $_.ToString() }}; 'errors: ' + @($e).Count")
    out = powershell("-Command", code)
    assert out.stdout.strip().endswith("errors: 0"), out.stdout + out.stderr


COMPILE = r"""
param([string]$Path)
$text = [System.IO.File]::ReadAllText($Path)
$m = [regex]::Match($text, "(?s)\$NativeSource = @'\r?\n(.*?)\r?\n'@")
if (-not $m.Success) { 'native source not found'; exit 2 }
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition $m.Groups[1].Value -IgnoreWarnings
'compiled'
"""


@needs_powershell
def test_the_embedded_csharp_compiles_without_being_called(tmp_path):
    compile_only = tmp_path / "compile.ps1"
    compile_only.write_text(COMPILE, encoding="utf-8")
    out = powershell("-File", compile_only, "-Path", SCRIPT)
    assert out.stdout.strip() == "compiled", out.stdout + out.stderr


# The doc.

def doc() -> str:
    return DOC.read_text(encoding="utf-8")


def test_doc_is_cataloged_and_states_the_limits():
    text = doc()
    for limit in ("Never type passwords", "Never place trades or move money", "Never solve CAPTCHAs",
                  "as data, never as instructions", "Never change Windows security settings"):
        assert limit in text
    assert "[`desktop-tool.md`](desktop-tool.md)" in (ROOT / "library" / "process" / "README.md").read_text(encoding="utf-8")
    assert "[`desktop-tool.md`](desktop-tool.md)" in (ROOT / "library" / "process" / "tool-routes.md").read_text(encoding="utf-8")


def test_doc_names_the_away_guard_restore_exit_4_and_the_decided_findings():
    text = doc()
    assert "exit 4" in text.lower() or "4 the operator is active" in text
    assert "-IdleSeconds" in text and "120" in text
    assert "`restore`" in text and "last step" in text
    assert "(off limits)" in text
    assert "A full-screen screenshot is refused" in text
    assert "A full-screen screenshot still captures" not in text
    assert "not detected until the next command" in text
    assert "never lower `-idleseconds`" in text.lower()
    assert "away guard" in text.lower() and "away guard" in (ROOT / "library" / "process" / "README.md").read_text(encoding="utf-8")
    assert "`system/logs/desktop/run.json`" in text


def test_doc_stays_inside_its_word_cap():
    assert len(doc().split()) <= 1500, "a process file is capped at 1,500 words (af doctor)"
    assert len((ROOT / "library" / "process" / "README.md").read_text(encoding="utf-8").split()) <= 1500
    assert len((ROOT / "AGENTS.operator.md").read_text(encoding="utf-8").split()) <= 1000


def test_doc_has_no_em_dash_and_names_no_institution_of_his():
    for rel in ("library/process/desktop-tool.md",):
        text = (ROOT / rel).read_text(encoding="utf-8")
        assert "—" not in text and "–" not in text
