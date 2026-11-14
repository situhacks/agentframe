"""system/tools/desktop.ps1 and its doc: every check here runs without a desktop and touches no window.

Nothing here runs the script. Two checks start a throwaway PowerShell: one only parses the script, one only
compiles its embedded C# (nothing in it is called), and the guard's arithmetic and state-file functions are
loaded one by one from the script text and run on numbers and temp files. Those functions call no Windows
API, which a test below checks. The live tests, which open windows and send input, are test_desktop_tool.py.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

import pytest

from desktop_fixtures import (FIXTURE, HEARTBEAT_LIMIT_SECONDS, NAMED_EXE, NAMED_EXE_LIFETIME_SECONDS,
                              THIEF_LIFETIME_SECONDS)

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "system" / "tools" / "desktop.ps1"
DOC = ROOT / "library" / "process" / "desktop-tool.md"
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)
POWERSHELL = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass"]
needs_powershell = pytest.mark.skipif(sys.platform != "win32" or not shutil.which("powershell"),
                                      reason="Windows PowerShell is needed")

# The functions the harness may load. None calls Windows; test_the_loaded_functions_call_no_windows_api checks it.
PURE = ["Get-TickAge", "Get-StateField", "New-RunState", "Read-RunState", "Write-RunState", "Test-RunStateFresh",
        "Get-OperatorIdleMs", "Test-RunContinues", "Get-ShownTitle", "Get-ExposedDeniedCount", "Get-RepositoryRoots",
        "Test-OutAllowed", "Get-NativeCacheName"]

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


def state(operator_tick=10_000, run_start_tick=50_000, run_start_ms=T0, own_tick=50_000, **extra):
    return {"operator_tick": operator_tick, "own_tick": own_tick, "run_start_tick": run_start_tick,
            "run_start_ms": run_start_ms, **extra}


def function_text(name: str) -> str:
    text = SCRIPT.read_text(encoding="utf-8")
    start = text.index(f"function {name}")
    nxt = re.search(r"^(function |# --- |\$script:Bound)", text[start + 10:], re.M)
    body = text[start:start + 10 + nxt.start()] if nxt else text[start:]
    return re.sub(r"(\n#[^\n]*)+\n?$", "\n", body)  # the comment above the next function is not this one's


# name -> (function, arguments, expected value)
FIVE_MIN = 300_000
GUARD_CASES = {
    # Unsigned tick arithmetic, across a wrap of the 32-bit clock.
    "age": ("Get-TickAge", [1000, 400], 600),
    "age zero": ("Get-TickAge", [5, 5], 0),
    "age across a wrap": ("Get-TickAge", [100, WRAP - 100], 200),
    # A run begun 60 s ago: ticks and wall clock agree.
    "fresh": ("Test-RunStateFresh", [state(), 110_000, T0 + 60_000], True),
    "fresh at exactly 5 minutes": ("Test-RunStateFresh", [state(), 50_000 + FIVE_MIN, T0 + FIVE_MIN], True),
    "a run 5 minutes and 1 second old is over": ("Test-RunStateFresh", [state(), 50_000 + FIVE_MIN + 1000, T0 + FIVE_MIN + 1000], False),
    "10 minutes is over (the old 30 minute limit is gone)": ("Test-RunStateFresh", [state(), 50_000 + 600_000, T0 + 600_000], False),
    "begun in the future": ("Test-RunStateFresh", [state(), 40_000, T0 - 10_000], False),
    "wall clock inside the limit but the tick clock past it": ("Test-RunStateFresh", [state(), 50_000 + 400_000, T0 + 100_000], False),
    "from before a reboot, ticks restarted small": ("Test-RunStateFresh", [state(), 20_000, T0 + 200_000], False),
    "from before a reboot, ticks restarted larger": ("Test-RunStateFresh", [state(), 80_000, T0 + 200_000], False),
    "begun just before a wrap (a tick greater than now is not stale)": (
        "Test-RunStateFresh",
        [state(operator_tick=WRAP - 70_000, run_start_tick=WRAP - 300, own_tick=WRAP - 300), 304, T0 + 604], True),
    "his tick after the run began": ("Test-RunStateFresh", [state(operator_tick=80_000), 110_000, T0 + 60_000], False),
    # His idle time: the tool's own last input does not count against him.
    "own input is the last: idle since his own": ("Get-OperatorIdleMs", [state(), 110_000, 50_000, T0 + 60_000], 100_000),
    "he touched it after the tool: plain rule": ("Get-OperatorIdleMs", [state(), 110_000, 90_000, T0 + 60_000], 20_000),
    "run over its limit: plain rule": (
        "Get-OperatorIdleMs", [state(), 50_000 + 400_000, 50_000, T0 + 400_000], 400_000),
    "no state: plain rule": ("Get-OperatorIdleMs", [None, 110_000, 100_000, T0], 10_000),
    "state with no own tick: plain rule": ("Get-OperatorIdleMs", [state(own_tick=None), 110_000, 50_000, T0 + 60_000], 60_000),
    "own input then a wrap": (
        "Get-OperatorIdleMs",
        [state(operator_tick=WRAP - 70_000, run_start_tick=WRAP - 300, own_tick=WRAP - 300), 304, WRAP - 300, T0 + 604],
        70_304),
    # Whether the run goes on (its recorded foreground and pointer are kept) or is replaced.
    "continues after the tool's own input": ("Test-RunContinues", [state(), 110_000, 50_000, T0 + 60_000], True),
    "continues before the tool sent any input": (
        "Test-RunContinues", [state(own_tick=None, operator_tick=10_000), 110_000, 10_000, T0 + 60_000], True),
    "replaced after his input": ("Test-RunContinues", [state(), 110_000, 90_000, T0 + 60_000], False),
    "replaced when over its limit": ("Test-RunContinues", [state(), 50_000 + 400_000, 50_000, T0 + 400_000], False),
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
    assert read["operator_tick"] == 7 and read["own_tick"] == 11 and read["run_start_tick"] == 100
    assert read["run_start_ms"] == 200 and read["foreground"] == foreground and read["pointer"] == {"x": 5, "y": 6}
    assert [p.name for p in path.parent.iterdir()] == ["run.json"], "the temp file is gone"
    assert json.loads(path.read_text(encoding="utf-8"))["own_tick"] == 11


@needs_powershell
@pytest.mark.parametrize("content", [
    None, "", "not json", "[1, 2]", "null", '{"operator_tick": 1}', '{"operator_tick": "x", "run_start_tick": 1, "run_start_ms": 1}',
    '{"operator_tick": 1, "run_start_tick": 1, "run_start_ms": 1, "own_tick": "soon"}',
    '{"operator_tick": 1, "tick_at_write": 1, "written_ms": 1}',
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
            assert {"operator_tick", "own_tick", "run_start_tick", "run_start_ms", "foreground", "pointer"} <= set(data)
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


# The guard's whole flow (lock, state file, Enter-AwayGuard, Invoke-Timed, Complete-Run, Invoke-Restore) run
# against a fake [AfDesktop] class whose clocks, pointer and windows the test scripts. The fake calls no
# Windows API: the real class is never compiled in this process, and no window, input or cursor is touched.

FLOW_HARNESS = r"""
param([string]$Script, [string]$StepsFile, [string]$Root, [string]$MutexName)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
public static class AfDesktop {
    public static long Last, Now, Ms, Front = 777, Alive = 777, OwnTick = -2, Expected = -2, IntrudedTick = -1;
    public static int PX = 10, PY = 20, CursorX = -1, CursorY = -1;
    public static bool CursorOk = true, FrontOk = true, Intruded;
    public static void Arm(long tick) { Expected = tick; Intruded = false; }
    public static void Accept(long tick) { OwnTick = tick; Expected = tick; }
    public static bool FakeCall(int ms, long bump) { System.Threading.Thread.Sleep(ms); if (bump >= 0) Last = bump; return true; }
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
$names = @('Stop-Error', 'Stop-Failed', 'Stop-Active', 'Stop-Intruded', 'Stop-IfIntruded', 'Get-TickAge', 'Get-StateField',
    'New-RunState', 'Read-RunState', 'Write-RunState', 'Test-RunStateFresh', 'Get-OperatorIdleMs', 'Test-RunContinues',
    'Get-StateRoot', 'Get-StatePath', 'Assert-Usage', 'Lock-Run', 'Get-AwayStatus', 'Get-ForegroundRecord',
    'Get-PointerRecord', 'Write-Trace', 'Invoke-Timed', 'Enter-AwayGuard', 'Save-Run', 'Complete-Run', 'Remove-RunState',
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
$TimedCallLimitMs = 250
$GatedCommands = @('focus', 'invoke', 'click', 'type', 'key', 'restore')
$Command = 'focus'
$SettleMs = 0
$script:Guard = $null
$script:RunLock = $null
$script:Trace = $false
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
    # Each step is a new process of the tool: it has sent nothing and armed nothing yet.
    [AfDesktop]::OwnTick = -2
    [AfDesktop]::Expected = -2
    [AfDesktop]::Intruded = $false
    Set-Fake $s.set
    $IdleSeconds = if ($null -ne (Get-StateField $s 'idle')) { [int]$s.idle } else { 120 }
    $Command = if ($null -ne (Get-StateField $s 'command')) { [string]$s.command } else { 'focus' }
    $script:Trace = (Get-StateField $s 'trace') -eq $true
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
            'validate' { Assert-Usage }
            'guard' { Enter-AwayGuard }
            'restore' { Invoke-Restore | ForEach-Object { [void]$out.Add([string]$_) } }
        }
        $calls = Get-StateField $s 'calls'
        if ($null -ne $calls) {
            foreach ($c in @($calls)) {
                if ($null -ne (Get-StateField $c 'pre_bump')) { [AfDesktop]::Last = [long]$c.pre_bump }
                $callMs = [int]$c.ms
                $callBump = if ($null -ne (Get-StateField $c 'bump')) { [long]$c.bump } else { -1 }
                [void](Invoke-Timed ([string]$c.label) { [AfDesktop]::FakeCall($callMs, $callBump) })
            }
        }
        $fail = Get-StateField $s 'send_fail'
        if ($null -ne $fail) {
            Set-Fake (Get-StateField $s 'before_fail')
            [AfDesktop]::Intruded = $true
            [AfDesktop]::IntrudedTick = [long]$fail.tick
            Stop-IfIntruded ([string]$fail.note)
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
MIN = 60_000


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


def guard_step(name, now, last, ms, *, idle=None, after=None, exit=None, calls=None, send_fail=None, before_fail=None,
               **fake):
    step = {"name": name, "run": "guard", "set": {"Now": now, "Last": last, "Ms": ms, **fake}}
    for key, value in (("idle", idle), ("after", after), ("exit", exit), ("calls", calls), ("send_fail", send_fail),
                       ("before_fail", before_fail)):
        if value is not None:
            step[key] = value
    return step


def own_after(tick, now=None, ms=None):
    """The machine after the tool sent input: the last input is the tool's own tick."""
    return {"OwnTick": tick, "Last": tick, "Now": tick + 10 if now is None else now, **({} if ms is None else {"Ms": ms})}


@needs_powershell
def test_a_run_keeps_his_tick_and_its_first_record_across_commands(tmp_path):
    first, second, his_input, long_away = run_flow(tmp_path, [
        # He has been away 10 minutes. The command sends input, so the last-input tick becomes the tool's own.
        guard_step("first", T, T - 600_000, M, Front=777, PX=10, PY=20, after=own_after(T + 50, T + 60, M + 60)),
        # Two seconds later the last input is still the tool's own: allowed, and the first record is kept.
        guard_step("second", T + 2000, T + 50, M + 2000, Front=999, PX=500, PY=600,
                   after=own_after(T + 2100, T + 2110, M + 2110)),
        # He moves the mouse a second ago.
        guard_step("he comes back", T + 5000, T + 4000, M + 5000),
        # Later he has been still for 130 s: the plain rule passes, and the run starts over.
        guard_step("a new run", T + 134_000, T + 4000, M + 134_000, Front=555, PX=1, PY=2),
    ])
    assert first["exit"] == 0 and first["files"] == ["run.json"]
    assert first["state"]["operator_tick"] == T - 600_000
    assert first["state"]["own_tick"] == T + 50
    assert first["state"]["run_start_tick"] == T and first["state"]["run_start_ms"] == M
    assert first["state"]["foreground"] == {"handle": 777, "pid": 4242, "process": "fake"}
    assert first["state"]["pointer"] == {"x": 10, "y": 20}

    assert second["exit"] == 0
    assert second["state"]["operator_tick"] == T - 600_000, "his tick is kept"
    assert second["state"]["own_tick"] == T + 2100, "the tool's own tick follows its own input"
    assert second["state"]["run_start_ms"] == M, "rewriting the state does not move the run's start"
    assert second["state"]["foreground"]["handle"] == 777 and second["state"]["pointer"] == {"x": 10, "y": 20}, \
        "the first command's record is kept"

    assert his_input["exit"] == 4
    assert "the operator is active (last input 1 s ago); nothing was sent; wait and retry" in his_input["stderr"]
    assert his_input["state"] == second["state"], "a refusal changes nothing"

    assert long_away["exit"] == 0
    assert long_away["state"]["operator_tick"] == T + 4000
    assert long_away["state"]["own_tick"] is None
    assert long_away["state"]["run_start_ms"] == M + 134_000
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
        guard_step("type, refused after 40 characters", T, T - 600_000, M, exit=3, after=own_after(T + 300, T + 310)),
        guard_step("the next command", T + 1000, T + 300, M + 1000),
    ])
    assert refused["exit"] == 3 and refused["state"]["own_tick"] == T + 300
    assert next_command["exit"] == 0, "the tool's partial input does not lock it out"


# A call that sends no input is timed on its own (Invoke-Timed): the clock is read straight before and after it.

@needs_powershell
def test_a_quick_call_that_moved_the_clock_is_the_tools_own(tmp_path):
    raised, next_command, trace = run_flow(tmp_path, [
        guard_step("raise moved the clock, 5 ms", T, T - 600_000, M, calls=[{"label": "SetForegroundWindow", "ms": 5, "bump": T + 80}]),
        guard_step("the next command", T + 800, T + 80, M + 800),
        guard_step("traced", T + 1600, T + 80, M + 1600, calls=[{"label": "SetForegroundWindow", "ms": 0}]),
    ])
    assert raised["exit"] == 0 and raised["state"]["own_tick"] == T + 80
    assert raised["stderr"] == "", "nothing extra is printed without -Verbose"
    assert next_command["exit"] == 0
    assert trace["stderr"] == ""


@needs_powershell
def test_a_slow_call_that_moved_the_clock_is_his_and_the_next_command_waits(tmp_path):
    slow, next_command, quiet = run_flow(tmp_path, [
        # An Invoke held up by a modal dialog for 400 ms while he clicked it away: the clock moved, but not as the tool's own.
        guard_step("invoke held up", T, T - 600_000, M, calls=[{"label": "InvokePattern", "ms": 400, "bump": T + 5000}]),
        guard_step("the next command, 1 s later", T + 6000, T + 5000, M + 6000),
        guard_step("after 130 quiet seconds", T + 135_000, T + 5000, M + 135_000),
    ])
    assert slow["exit"] == 0 and slow["state"]["own_tick"] is None, "his change is not taken as the tool's own"
    assert next_command["exit"] == 4 and "last input 1 s ago" in next_command["stderr"]
    assert quiet["exit"] == 0 and quiet["state"]["operator_tick"] == T + 5000


@needs_powershell
def test_a_clock_that_moved_outside_any_timed_call_is_never_taken_as_the_tools_own(tmp_path):
    moved, next_command = run_flow(tmp_path, [
        # No injected input and no timed call, yet the clock moved during the command.
        guard_step("focus, the clock moved somewhere else", T, T - 600_000, M, after={"Last": T + 80, "Now": T + 90}),
        guard_step("the next command", T + 500, T + 80, M + 500),
    ])
    assert moved["exit"] == 0 and moved["state"]["own_tick"] is None
    assert next_command["exit"] == 4 and "last input 0 s ago" in next_command["stderr"]


@needs_powershell
def test_a_command_that_moved_no_tick_and_sent_nothing_records_no_own_tick(tmp_path):
    only, = run_flow(tmp_path, [guard_step("focus", T, T - 600_000, M)])
    assert only["exit"] == 0 and only["state"]["own_tick"] is None
    assert only["state"]["operator_tick"] == T - 600_000


@needs_powershell
def test_his_input_before_a_timed_call_stops_the_command_and_the_next_one_waits(tmp_path):
    stopped, next_command, later = run_flow(tmp_path, [
        guard_step("two calls, he clicks between them", T + 1000, T - 600_000, M, calls=[
            {"label": "ShowWindow", "ms": 0, "bump": T + 40}, {"label": "SetForegroundWindow", "ms": 0, "pre_bump": T + 900}]),
        guard_step("the next command", T + 1500, T + 900, M + 1500),
        guard_step("130 quiet seconds on", T + 131_000, T + 900, M + 131_000),
    ])
    assert stopped["exit"] == 4
    assert "the operator is active (last input 0 s ago); nothing more was sent; wait and retry" in stopped["stderr"]
    assert stopped["state"]["own_tick"] == T + 40, "what the tool did before he came is kept as its own"
    assert next_command["exit"] == 4
    assert later["exit"] == 0 and later["state"]["operator_tick"] == T + 900


@needs_powershell
def test_his_input_in_the_middle_of_a_send_stops_with_the_count_and_the_next_command_waits(tmp_path):
    stopped, next_command, later = run_flow(tmp_path, [
        guard_step("type: he touched the machine after character 12", T + 3000, T - 600_000, M,
                   before_fail={"OwnTick": T + 1000},
                   send_fail={"tick": T + 2000, "note": "12 of 40 characters were sent."}),
        guard_step("the next command", T + 3000, T + 2000, M + 3000),
        guard_step("a later one", T + 125_000, T + 2000, M + 125_000),
    ])
    assert stopped["exit"] == 4
    assert "the operator is active (last input 1 s ago); 12 of 40 characters were sent; wait and retry" in stopped["stderr"]
    assert stopped["state"]["own_tick"] == T + 1000, "the own tick is the tool's last send, not his input"
    assert next_command["exit"] == 4, "his tick is newer than the tool's own, so the plain rule applies"
    assert later["exit"] == 0


@needs_powershell
def test_the_reviewers_chain_no_command_passes_until_120_quiet_seconds_after_any_input(tmp_path):
    r = run_flow(tmp_path, [
        guard_step("1 invoke blocks on a modal; he closes it at +5 s", T, T - 600_000, M,
                   calls=[{"label": "InvokePattern", "ms": 400, "bump": T + 5000}], after={"Now": T + 5100, "Ms": M + 5100}),
        guard_step("2 next command 1 s after his click", T + 6000, T + 5000, M + 6000),
        guard_step("3 119 s after his click", T + 5000 + 119_000, T + 5000, M + 5000 + 119_000),
        guard_step("4 121 s after his click", T + 5000 + 121_000, T + 5000, M + 5000 + 121_000,
                   after=own_after(T + 5000 + 121_100, T + 5000 + 121_200, M + 5000 + 121_200)),
        guard_step("5 his key 1 s after the tool's last send", T + 5000 + 123_000, T + 5000 + 122_000, M + 5000 + 123_000),
    ])
    assert [x["exit"] for x in r] == [0, 4, 4, 0, 4]
    assert r[3]["state"]["operator_tick"] == T + 5000, "the run began with his click as his last input"


@needs_powershell
def test_a_run_has_a_five_minute_limit_that_rewriting_the_state_does_not_extend(tmp_path):
    r = run_flow(tmp_path, [
        guard_step("run begins", T, T - 600_000, M, after=own_after(T + 100, T + 200, M + 200)),
        guard_step("4 min 59 s in: still inside the run", T + 299_000, T + 100, M + 299_000,
                   after=own_after(T + 299_100, T + 299_200, M + 299_200)),
        guard_step("5 min 1 s in, rewritten 2 s ago: not fresh, so the tool's own input counts against it",
                   T + 301_000, T + 299_100, M + 301_000),
        guard_step("then 2 quiet minutes", T + 299_100 + 121_000, T + 299_100, M + 299_100 + 121_000),
    ])
    assert [x["exit"] for x in r] == [0, 0, 4, 0]
    assert r[1]["state"]["run_start_ms"] == M, "the second command rewrote the state without moving the start"
    assert "last input 1 s ago" in r[2]["stderr"]
    assert r[3]["state"]["run_start_ms"] == M + 299_100 + 121_000, "a new run"


@needs_powershell
def test_a_trace_line_per_call_shows_whether_the_clock_moved_and_how_long_it_took(tmp_path):
    traced, plain = run_flow(tmp_path, [
        {**guard_step("traced", T, T - 600_000, M, calls=[{"label": "SetForegroundWindow", "ms": 20, "bump": T + 80},
                                                          {"label": "InvokePattern", "ms": 0}]), "trace": True},
        {**guard_step("untraced", T + 1000, T + 80, M + 1000, calls=[{"label": "InvokePattern", "ms": 0}]), "trace": False},
    ])
    lines = traced["stderr"].splitlines()
    assert re.fullmatch(r"trace: SetForegroundWindow clock_moved=True ms=\d+ accepted=True", lines[0]), lines
    assert re.fullmatch(r"trace: InvokePattern clock_moved=False ms=\d+ accepted=False", lines[1]), lines
    assert plain["stderr"] == ""


@needs_powershell
@pytest.mark.parametrize("command", ["focus", "invoke", "click", "type", "key", "restore"])
def test_a_negative_idle_seconds_is_bad_usage_for_every_gated_command(tmp_path, command):
    bad, fine, other = run_flow(tmp_path, [
        {"name": "negative", "run": "validate", "command": command, "idle": -1, "set": {"Last": T, "Now": T, "Ms": M}},
        {"name": "zero", "run": "validate", "command": command, "idle": 0, "set": {}},
        {"name": "an ungated command", "run": "validate", "command": "tree", "idle": -1, "set": {}},
    ])
    assert bad["exit"] == 2 and "error: -IdleSeconds must be 0 or more" in bad["stderr"]
    assert bad["files"] == [] and bad["state"] is None
    assert fine["exit"] == 0 and other["exit"] == 0


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
    start = guard_step("start", T, T - 600_000, M, Front=777, PX=10, PY=20, after=own_after(T + 50, T + 60, M + 60))
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
        guard_step("start", T, T - 600_000, M, after=own_after(T + 50, T + 60, M + 60)),
        {"name": "5 minutes and 1 second later", "run": "restore",
         "set": {"Now": T + 301_000, "Last": T + 50, "Ms": M + 301_000}},
    ])
    none, old = results[0], results[2]
    assert none["exit"] == 0 and none["stdout"] == ["nothing to restore: no run is recorded, or its record is more than 5 minutes old"]
    assert old["exit"] == 0 and old["stdout"][0].startswith("nothing to restore") and old["state"] is None
    assert old["cursor"] == [-1, -1]


# The real C# Send, run with a scripted clock and a fake sender: nothing is sent and no clock is read.

SEND_HARNESS = r"""
param([string]$Path, [string]$CasesFile)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$text = [System.IO.File]::ReadAllText($Path)
$m = [regex]::Match($text, "(?s)\`$NativeSource = @'\r?\n(.*?)\r?\n'@")
Add-Type -TypeDefinition $m.Groups[1].Value -IgnoreWarnings
$script:calls = 0
$script:ret = 0
$script:clocks = @()
$script:ci = 0
$send = [AfDesktop+SendFn]{ param($events) $script:calls++; [uint32]$script:ret }
$clock = [AfDesktop+ClockFn]{ $v = $script:clocks[$script:ci]; $script:ci++; [long]$v }
$cases = [System.IO.File]::ReadAllText($CasesFile) | ConvertFrom-Json
$results = foreach ($c in $cases) {
    [AfDesktop]::OwnTick = -2
    [AfDesktop]::Expected = [long]$c.expected
    [AfDesktop]::Intruded = $false
    [AfDesktop]::IntrudedTick = 0
    $rows = foreach ($s in @($c.sends)) {
        $script:calls = 0
        $script:ret = [int]$s.returns
        $script:clocks = @($s.clocks)
        $script:ci = 0
        $events = [Array]::CreateInstance([AfDesktop+INPUT], [int]$s.events)
        $r = [AfDesktop]::SendGuarded($events, $send, $clock)
        [pscustomobject]@{ result = $r; called = $script:calls; clock_reads = $script:ci }
    }
    [pscustomobject]@{ id = $c.id; sends = @($rows); own = [AfDesktop]::OwnTick; expected = [AfDesktop]::Expected
        intruded = [AfDesktop]::Intruded; intruded_tick = [AfDesktop]::IntrudedTick }
}
ConvertTo-Json -InputObject @($results) -Depth 6 -Compress
"""

SEND_CASES = [
    {"id": "unarmed: nothing is sent", "expected": -2, "sends": [{"events": 2, "returns": 2, "clocks": [100]}]},
    {"id": "the clock is still the expected tick: sent, and the new tick is the own tick", "expected": 100,
     "sends": [{"events": 2, "returns": 2, "clocks": [100, 108]}]},
    {"id": "he touched the machine since: nothing is sent", "expected": 100,
     "sends": [{"events": 2, "returns": 2, "clocks": [105]}]},
    {"id": "his input between two sends of one command stops the second", "expected": 100, "sends": [
        {"events": 2, "returns": 2, "clocks": [100, 110]}, {"events": 2, "returns": 2, "clocks": [110, 120]},
        {"events": 2, "returns": 2, "clocks": [125]}]},
    {"id": "a partial send is a failure, not an intrusion, and its tick is kept", "expected": 100,
     "sends": [{"events": 2, "returns": 1, "clocks": [100, 112]}]},
    {"id": "an unreadable clock before a send stops it", "expected": 100, "sends": [{"events": 1, "returns": 1, "clocks": [-1]}]},
    {"id": "nothing accepted when the sender sent none", "expected": 100,
     "sends": [{"events": 1, "returns": 0, "clocks": [100, 130]}]},
]


@pytest.fixture(scope="module")
def send_results(tmp_path_factory):
    if sys.platform != "win32" or not shutil.which("powershell"):
        pytest.skip("Windows PowerShell is needed")
    folder = tmp_path_factory.mktemp("send")
    harness = folder / "send.ps1"
    harness.write_text(SEND_HARNESS, encoding="utf-8")
    cases = folder / "cases.json"
    cases.write_text(json.dumps(SEND_CASES), encoding="utf-8")
    out = powershell("-File", harness, "-Path", SCRIPT, "-CasesFile", cases)
    assert out.returncode == 0, out.stderr + out.stdout
    return {row["id"]: row for row in json.loads(out.stdout)}


@needs_powershell
def test_send_with_the_guard_unarmed_sends_nothing(send_results):
    row = send_results["unarmed: nothing is sent"]
    assert row["sends"][0]["result"] is False and row["sends"][0]["called"] == 0 and row["intruded"] is True


@needs_powershell
def test_send_reads_the_clock_before_and_after_and_keeps_the_new_tick_as_its_own(send_results):
    row = send_results["the clock is still the expected tick: sent, and the new tick is the own tick"]
    assert row["sends"][0] == {"result": True, "called": 1, "clock_reads": 2}
    assert (row["own"], row["expected"], row["intruded"]) == (108, 108, False)


@needs_powershell
def test_send_stops_when_the_clock_is_not_the_expected_tick(send_results):
    row = send_results["he touched the machine since: nothing is sent"]
    assert row["sends"][0] == {"result": False, "called": 0, "clock_reads": 1}
    assert row["intruded"] is True and row["intruded_tick"] == 105
    assert (row["own"], row["expected"]) == (-2, 100), "the own and expected ticks stay as they were"


@needs_powershell
def test_send_stops_a_command_at_the_second_send_when_he_acts_between_two(send_results):
    row = send_results["his input between two sends of one command stops the second"]
    assert [s["result"] for s in row["sends"]] == [True, True, False]
    assert [s["called"] for s in row["sends"]] == [1, 1, 0]
    assert row["intruded"] is True and row["intruded_tick"] == 125
    assert (row["own"], row["expected"]) == (120, 120), "the last tick the tool made itself is kept"


@needs_powershell
def test_send_treats_a_partial_send_as_a_failure_not_an_intrusion(send_results):
    row = send_results["a partial send is a failure, not an intrusion, and its tick is kept"]
    assert row["sends"][0]["result"] is False and row["intruded"] is False and row["own"] == 112


@needs_powershell
def test_send_fails_closed_on_an_unreadable_clock_and_accepts_nothing_when_nothing_was_sent(send_results):
    unreadable = send_results["an unreadable clock before a send stops it"]
    assert unreadable["sends"][0]["called"] == 0 and unreadable["intruded"] is True
    none_sent = send_results["nothing accepted when the sender sent none"]
    assert none_sent["own"] == -2 and none_sent["expected"] == 100 and none_sent["intruded"] is False


# The screenshot path rule, with temp folders laid out as a main checkout and a worktree.

def make_repositories(base: Path):
    main = base / "main"
    (main / ".git" / "worktrees" / "wt").mkdir(parents=True)
    (main / ".git" / "worktrees" / "wt" / "commondir").write_text("../..\n", encoding="utf-8")
    worktree = main / ".claude" / "worktrees" / "wt"
    worktree.mkdir(parents=True)
    (worktree / ".git").write_text(f"gitdir: {(main / '.git' / 'worktrees' / 'wt').as_posix()}\n", encoding="utf-8")
    af_root = base / "vault"
    af_root.mkdir()
    return main, worktree, af_root


@needs_powershell
def test_a_screenshot_path_is_refused_inside_any_tracked_folder_of_the_checkout_af_root_or_main_repository(tmp_path):
    main, worktree, af_root = make_repositories(tmp_path)
    outside = tmp_path / "scratch" / "x.png"
    allowed = [outside, worktree / "workspace" / "x.png", worktree / "system" / "sessions" / "a" / "x.png",
               main / "workspace" / "x.png", main / "system" / "sessions" / "x.png", af_root / "workspace" / "x.png",
               af_root / "system" / "sessions" / "x.png"]
    refused = [worktree / "library" / "x.png", main / "library" / "process" / "x.png", main / "x.png",
               worktree / "system" / "tools" / "x.png", af_root / "library" / "x.png", af_root / "system" / "logs" / "x.png",
               main / ".claude" / "worktrees" / "other" / "x.png", main / "workspacex" / "x.png"]
    cases = [{"id": "roots", "fn": "Get-RepositoryRoots", "args": [str(worktree), str(af_root)]}]
    rows = run_functions(tmp_path, cases, PURE)
    roots = [Path(r).resolve() for r in rows["roots"]["value"]]
    assert sorted(roots) == sorted({worktree.resolve(), af_root.resolve(), main.resolve()}), roots
    cases = [{"id": str(p), "fn": "Test-OutAllowed", "args": [str(p), [str(r) for r in roots]]} for p in allowed + refused]
    rows = run_functions(tmp_path, cases, PURE)
    for p in allowed:
        assert rows[str(p)]["value"] is True, f"{p} should be allowed"
    for p in refused:
        assert rows[str(p)]["value"] is False, f"{p} should be refused"


OUT_HARNESS = r"""
param([string]$Script, [string]$ToolsDir, [string]$CasesFile)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Script, [ref]$null, [ref]$null)
foreach ($d in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and @('Assert-IgnoredOut', 'Get-RepositoryRoots', 'Test-OutAllowed') -contains $n.Name }, $true)) {
    # The script lives under system\tools of the checkout being tested, not of this repository.
    . ([scriptblock]::Create($d.Extent.Text.Replace('$PSScriptRoot', "'$ToolsDir'")))
}
function Stop-Failed($Message) { throw 'REFUSED' }
$results = foreach ($p in ([System.IO.File]::ReadAllText($CasesFile) | ConvertFrom-Json)) {
    try { Assert-IgnoredOut $p; [pscustomobject]@{ path = $p; allowed = $true } } catch { [pscustomobject]@{ path = $p; allowed = $false } }
}
ConvertTo-Json -InputObject @($results) -Compress
"""


@needs_powershell
def test_the_screenshot_path_rule_as_the_script_applies_it_from_a_worktree_with_af_root_set(tmp_path):
    main, worktree, af_root = make_repositories(tmp_path)
    allowed = [tmp_path / "scratch" / "x.png", worktree / "workspace" / "x.png", main / "workspace" / "x.png",
               af_root / "system" / "sessions" / "x.png"]
    refused = [worktree / "library" / "x.png", main / "library" / "process" / "x.png", af_root / "library" / "x.png"]
    harness = tmp_path / "out.ps1"
    harness.write_text(OUT_HARNESS, encoding="utf-8")
    cases = tmp_path / "cases.json"
    cases.write_text(json.dumps([str(p) for p in allowed + refused]), encoding="utf-8")
    env = {**os.environ, "AF_ROOT": str(af_root)}
    out = subprocess.run([*POWERSHELL, "-File", str(harness), "-Script", str(SCRIPT), "-ToolsDir", str(worktree / "system" / "tools"),
                          "-CasesFile", str(cases)], capture_output=True, text=True, encoding="utf-8", env=env,
                         creationflags=NO_WINDOW)
    assert out.returncode == 0, out.stderr + out.stdout
    answers = {Path(row["path"]): row["allowed"] for row in json.loads(out.stdout)}
    assert [answers[p] for p in allowed] == [True] * len(allowed)
    assert [answers[p] for p in refused] == [False] * len(refused), "a path in a tracked folder of the main checkout, a worktree or AF_ROOT is refused"


@needs_powershell
def test_the_repository_roots_of_a_plain_checkout_and_without_af_root(tmp_path):
    (tmp_path / "plain" / ".git").mkdir(parents=True)
    rows = run_functions(tmp_path, [{"id": "r", "fn": "Get-RepositoryRoots", "args": [str(tmp_path / "plain"), ""]}], PURE)
    value = rows["r"]["value"]  # one root comes back as a bare string, as PowerShell unrolls a one-element array
    assert [Path(r).resolve() for r in ([value] if isinstance(value, str) else value)] == [(tmp_path / "plain").resolve()]


# The compiled-assembly cache: its name is a hash of the source, and any trouble falls back to compiling.

IMPORT_HARNESS = r"""
param([string]$Script, [string]$Folder)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$text = [System.IO.File]::ReadAllText($Script)
$m = [regex]::Match($text, "(?s)\`$NativeSource = @'\r?\n(.*?)\r?\n'@")
$NativeSource = $m.Groups[1].Value
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Script, [ref]$null, [ref]$null)
foreach ($d in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and @('Get-NativeCacheName', 'Import-Native') -contains $n.Name }, $true)) {
    . ([scriptblock]::Create($d.Extent.Text))
}
$from = Import-Native $NativeSource $Folder
$current = ([AfDesktop].GetMethod('SendGuarded') -ne $null) -and ([AfDesktop]::TickNow() -gt 0)
ConvertTo-Json -Compress @{ from = $from; usable = $current; name = (Get-NativeCacheName $NativeSource) }
"""


def run_import(folder: Path, cache_dir: Path) -> dict:
    harness = folder / "import.ps1"
    harness.write_text(IMPORT_HARNESS, encoding="utf-8")
    out = powershell("-File", harness, "-Script", SCRIPT, "-Folder", cache_dir)
    assert out.returncode == 0, out.stderr + out.stdout
    return json.loads(out.stdout)


@needs_powershell
def test_the_native_cache_is_named_by_the_source_hash_and_used_on_the_next_run(tmp_path):
    cache = tmp_path / "cache" / "desktop"
    first = run_import(tmp_path, cache)
    assert first["from"] == "built" and first["usable"] is True
    files = [p.name for p in cache.iterdir()]
    assert files == [first["name"]] and re.fullmatch(r"AfDesktop-[0-9a-f]{16}\.dll", first["name"])
    second = run_import(tmp_path, cache)
    assert second["from"] == "cache" and second["usable"] is True
    assert [p.name for p in cache.iterdir()] == files


@needs_powershell
def test_a_cache_file_for_other_source_is_never_loaded_and_is_pruned(tmp_path):
    cache = tmp_path / "cache"
    cache.mkdir()
    decoy = cache / "AfDesktop-0000000000000000.dll"
    decoy.write_bytes(b"not an assembly")
    result = run_import(tmp_path, cache)
    assert result["from"] == "built" and result["usable"] is True
    assert not decoy.exists() and [p.name for p in cache.iterdir()] == [result["name"]]


@needs_powershell
def test_a_damaged_cache_file_with_the_right_name_falls_back_to_a_build_and_is_replaced(tmp_path):
    cache = tmp_path / "cache"
    name = run_import(tmp_path, cache)["name"]
    (cache / name).write_bytes(b"MZ but not really")
    result = run_import(tmp_path, cache)
    assert result["from"] == "built" and result["usable"] is True
    assert run_import(tmp_path, cache)["from"] == "cache"


@needs_powershell
def test_a_cache_folder_that_cannot_be_written_falls_back_to_compiling_in_memory(tmp_path):
    blocker = tmp_path / "file-not-folder"
    blocker.write_text("x", encoding="utf-8")
    result = run_import(tmp_path, blocker / "desktop")
    assert result["from"] == "memory" and result["usable"] is True


@needs_powershell
def test_the_cache_name_changes_with_the_source(tmp_path):
    cases = [{"id": "a", "fn": "Get-NativeCacheName", "args": ["class A {}"]},
             {"id": "b", "fn": "Get-NativeCacheName", "args": ["class B {}"]},
             {"id": "a2", "fn": "Get-NativeCacheName", "args": ["class A {}"]}]
    rows = run_functions(tmp_path, cases, PURE)
    assert rows["a"]["value"] == rows["a2"]["value"] != rows["b"]["value"]
    assert re.fullmatch(r"AfDesktop-[0-9a-f]{16}\.dll", rows["a"]["value"])


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


def test_every_send_checks_the_clock_first_and_keeps_the_tick_it_reads_right_after():
    source = SCRIPT.read_text(encoding="utf-8")
    send = source[source.index("public static bool SendGuarded("):]
    send = send[:send.index("\n    }") + 6]
    assert send.index("before != Expected") < send.index("send(events)") < send.index("Accept(clock())")
    assert "if (!GetLastInputInfo(ref l)) return -1;" in source, "an unreadable clock fails closed"
    plain = source[source.index("static bool Send(INPUT[] events)"):]
    assert "SendGuarded(events," in plain[:plain.index("}")], "every SendInput goes through the guarded send"
    assert source.count("SendInput((uint)") == 1


def test_a_clock_change_is_the_tools_own_only_through_a_timed_call_or_its_own_send():
    complete = function_text("Complete-Run")
    assert "LastInput" not in complete, "a change anywhere else in the command is never taken as the tool's own"
    assert "[AfDesktop]::OwnTick" in complete
    timed = function_text("Invoke-Timed")
    assert "$TimedCallLimitMs" in timed and "Accept(" in timed and "Stop-Intruded" in timed
    assert timed.index("LastInput()") < timed.rindex("& $Call") < timed.rindex("LastInput()")
    assert re.search(r"\$TimedCallLimitMs = 250\b", SCRIPT.read_text(encoding="utf-8"))


def test_every_call_that_may_move_the_clock_is_timed_and_every_send_checks_for_his_input():
    text = SCRIPT.read_text(encoding="utf-8")
    code = text[text.index("function Exit-Tool"):]
    for number, line in enumerate(code.splitlines(), 1):
        if re.search(r"\[AfDesktop\]::(BringToFront|ShowWindow|SetCursorPos)\(|\$pattern\.Invoke\(\)", line):
            assert "Invoke-Timed" in line, f"an untimed call that may move the clock: {line.strip()}"
    for name in ("Invoke-GuardedClick", "Invoke-Type", "Invoke-Key"):
        assert "Stop-IfIntruded" in function_text(name), name
    assert "Arm(" in function_text("Invoke-Restore") and "Invoke-Timed" in function_text("Invoke-Restore")
    assert "Arm(" in function_text("Enter-AwayGuard")
    assert "-IdleSeconds must be 0 or more" not in function_text("Enter-AwayGuard"), "one shared check, Assert-Usage"


def test_the_idle_seconds_check_is_shared_and_runs_before_anything_is_compiled_or_touched():
    text = SCRIPT.read_text(encoding="utf-8")
    main = text[text.index("$script:Bound = $PSBoundParameters"):]
    assert main.index("Assert-Usage") < main.index("Add-Type") < main.index("Import-Native")
    body = function_text("Assert-Usage")
    assert "$GatedCommands -contains $Command" in body and "$IdleSeconds -lt 0" in body and "Stop-Failed" in body


def test_the_stale_wording_is_gone_from_the_script_the_doc_and_the_changelog():
    for path in (SCRIPT, DOC, ROOT / "CHANGELOG.md"):
        assert "not detected until the next command" not in path.read_text(encoding="utf-8"), path


def test_the_native_code_is_loaded_through_the_cache_and_named_by_a_hash():
    text = SCRIPT.read_text(encoding="utf-8")
    main = text[text.index("$script:Bound = $PSBoundParameters"):]
    assert "Import-Native $NativeSource" in main
    assert "Add-Type -TypeDefinition $NativeSource" not in main
    body = function_text("Import-Native")
    assert "Get-NativeCacheName" in body and "-OutputAssembly" in body and "Add-Type -Path" in body
    assert body.index("Add-Type -Path") < body.index("-OutputAssembly"), "a file for this source is tried first"


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
        for banned in ("[AfDesktop]", "Add-Type", "SendInput", "SetCursorPos", "Get-Process", "SetForeground"):
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
$m = [regex]::Match($text, "(?s)\`$NativeSource = @'\r?\n(.*?)\r?\n'@")
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
    assert "5 minutes" in text and "250 ms" in text and "AF_DESKTOP_TRACE" in text and "Before every send" in text
    assert "inside its run" in text
    assert "never lower `-idleseconds`" in text.lower()
    assert "away guard" in text.lower() and "away guard" in (ROOT / "library" / "process" / "README.md").read_text(encoding="utf-8")
    assert "`system/logs/desktop/run.json`" in text


def test_doc_stays_inside_its_word_cap():
    assert len(doc().split()) <= 1500, "a process file is capped at 1,500 words (af doctor)"
    assert len((ROOT / "library" / "process" / "README.md").read_text(encoding="utf-8").split()) <= 1500
    assert len((ROOT / "AGENTS.operator.md").read_text(encoding="utf-8").split()) <= 1000


def test_doc_has_no_em_dash_or_en_dash():
    # It cannot check for his institutions: naming them in a tracked test would be the leak it guards against.
    text = DOC.read_text(encoding="utf-8")
    assert "\u2014" not in text and "\u2013" not in text


# The live tests' helper windows (test_desktop_tool.py cannot be imported here: it skips without AF_DESKTOP_TESTS).

@needs_powershell
def test_the_live_fixture_script_parses(tmp_path):
    fixture = tmp_path / "fixture.ps1"
    fixture.write_text(FIXTURE, encoding="utf-8")
    code = ("$e=$null; [void][System.Management.Automation.Language.Parser]::ParseFile("
            f"'{fixture}', [ref]$null, [ref]$e); $e | ForEach-Object {{ $_.ToString() }}; 'errors: ' + @($e).Count")
    out = powershell("-Command", code)
    assert out.stdout.strip().endswith("errors: 0"), out.stdout + out.stderr


def test_every_helper_window_closes_itself_soon_after_the_test_run_stops():
    assert HEARTBEAT_LIMIT_SECONDS <= 30 and THIEF_LIFETIME_SECONDS <= 15 and NAMED_EXE_LIFETIME_SECONDS <= 30
    assert f"-gt {HEARTBEAT_LIMIT_SECONDS} -or" in FIXTURE, "the form closes when the heartbeat file goes quiet"
    assert "$failsafe.Interval = 240000" not in FIXTURE
    assert f"$script:ThiefEnd.Interval = {THIEF_LIFETIME_SECONDS * 1000}" in FIXTURE
    assert "$script:Thief.TopMost = $false" in FIXTURE, "the thief does not stay topmost"
    assert "Stop-Thief" in FIXTURE[FIXTURE.index("$failsafe.Add_Tick"):], "closing the form also ends the thief"
    assert f"failsafe.Interval = {NAMED_EXE_LIFETIME_SECONDS * 1000}" in NAMED_EXE
    live = (ROOT / "system" / "tests" / "test_desktop_tool.py").read_text(encoding="utf-8")
    assert '"-HeartbeatFile", str(HEARTBEAT)' in live and "HEARTBEAT.write_text" in live
