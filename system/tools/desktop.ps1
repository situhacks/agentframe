# Operate native Windows apps from a shell, using only what ships with Windows.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File system/tools/desktop.ps1 <command> [options]
#
#   windows
#   focus      -Window <title match>
#   screenshot [-Window <title match>] -Out <path.png> [-MaxWidth 1568]
#   tree       -Window <title match> [-Depth 4]
#   invoke     -Window <title match> -Name <element name>
#   click      -Window <title match> -X <screen x> -Y <screen y>
#   type       -Window <title match> -Text <text>
#   key        -Window <title match> -Keys "ctrl+s" (space-separated chords run in order)
#   restore    (puts back the pointer and the foreground window a run found, then ends the run)
#
# -Window is a case-insensitive literal match on the title. An exact title wins over a partial one;
# several matches are an error, never guessed. Exit codes: 0 done, 1 failure (Windows rejected the
# action, or the desktop cannot take input), 2 bad usage, nothing found, or an ambiguous match,
# 3 refused (final), 4 the operator is active: nothing was sent, wait and retry.
#
# Away guard: focus, invoke, click, type, key and restore run only when the operator has been away
# for -IdleSeconds (default 120; a lower value is for the tests, never for a real job). windows,
# tree and screenshot do not change what he sees and are not gated. The tool's own input resets the
# system's last-input clock, so a run keeps a state file (system/logs/desktop/run.json, git-ignored);
# see Get-OperatorIdleMs. Input he makes while one command is sending is not detected until the
# next command. Procedure and standing limits: library/process/desktop-tool.md.

[CmdletBinding(PositionalBinding = $false)]
param(
    [Parameter(Position = 0)][string]$Command,
    [string]$Window,
    [string]$Out,
    [int]$MaxWidth = 1568,
    [int]$Depth = 4,
    [string]$Name,
    [int]$X,
    [int]$Y,
    [string]$Text,
    [string]$Keys,
    [int]$IdleSeconds = 120
)

# >>> DENYLIST: windows this tool refuses to act on. Add a rule here and nowhere else.
# Each rule is a .NET regex, case-sensitive unless it starts with (?i). A false positive only costs
# a refusal, so a rule may be broad. This file is public: it holds category words and well-known
# names only. A rule that names the operator's own institutions goes in the local list instead.
$DenyNames = @(                         # checked against the process names and the title
    # Category words
    '(?i)bank'
    '(?i)broker'
    '(?i)trad(e|ing)'
    '(?i)invest'
    '(?i)wealth'
    '(?i)financ'
    '(?i)credit (union|card)'
    '(?i)crypto'
    '(?i)password'
    # Well-known password managers
    '(?i)keepass|bitwarden|lastpass|dashlane|nordpass|proton ?pass|roboform|enpass|keeper'
    # Well-known banks, card issuers, payment apps, and brokerages
    '(?<![A-Za-z0-9])(TD|ING|UBS)(?![A-Za-z0-9])' # upper case only, so "td" and "stdout" stay usable
    '(?i)(?<![a-z0-9])(rbc|bmo|cibc|hsbc|citi|n26)(?![a-z0-9])'
    '(?i)(?<![a-z])(chase|wells fargo|capital one|barclays|santander|lloyds|natwest|bnp paribas|desjardins)'
    '(?i)(?<![a-z])(american express|paypal|venmo|revolut|monzo)'
    '(?i)(?<![a-z])(schwab|fidelity|vanguard|robinhood|merrill|webull|etoro|coinbase|binance)'
)
# Optional and private, never tracked: one regex a line, checked like the rules above; # starts a comment.
$LocalDenyPath = 'workspace\life\systems\desktop-denylist.txt'
$DenyProcesses = @(                     # checked against the process names only
    '(?i)^CredentialUIBroker$'
    '(?i)^consent$'
    '(?i)^LogonUI$'
    '(?i)^SecHealthUI$'
    '(?i)^SecurityHealth'
    '(?i)^SystemSettings$'
    '(?i)^control$'
    '(?i)^SystemProperties'
    '(?i)^rundll32$'                    # hosts Control Panel applets
    '(?i)^mmc$'                         # hosts the firewall, security policy, and services consoles
    '(?i)^regedit$'
)
$DenyTitles = @(                        # checked against the title only
    '(?i)windows security'
    '(?i)user account control'
    '(?i)^settings$'
    '(?i)control panel'
)
# <<< DENYLIST

$TreeNodeLimit = 2000
$TypeDelayMs = 5
$SettleMs = 150
# Commands that change what he sees or send input; each runs Enter-AwayGuard (restore has its own gate).
$GatedCommands = @('focus', 'invoke', 'click', 'type', 'key', 'restore')
# Under the checkout, or under AF_ROOT when set so a worktree and the main checkout share one run. Git-ignored.
$StateRelative = 'system\logs\desktop\run.json'
$RunMutexName = 'Local\AgentFrameDesktopTool'
# Explorer owns the taskbar and the desktop, so a click there would pass the same-process check
# for any File Explorer target and could launch an app.
$ShellSurfaces = @('Shell_TrayWnd', 'Shell_SecondaryTrayWnd', 'Progman', 'WorkerW')

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:Guard = $null
$script:RunLock = $null

$NativeSource = @'
#pragma warning disable 618
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class AfDesktop {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Explicit)] public struct INPUTUNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public INPUTUNION u; }

    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc cb, IntPtr lParam);
    [DllImport("user32.dll")] static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextLengthW(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern uint MapVirtualKeyW(uint code, uint mapType);
    [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [StructLayout(LayoutKind.Sequential)] struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }
    [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LASTINPUTINFO info);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr context);

    public static void MakeDpiAware() {
        try { if (SetProcessDpiAwarenessContext(new IntPtr(-4))) return; }
        catch (EntryPointNotFoundException) { }
        SetProcessDPIAware();
    }

    // The tick (GetTickCount, 32 bits, wraps every 49.7 days) of the last input by anyone, or -1 when
    // Windows will not say. Compared for equality, and subtracted with unsigned arithmetic (Get-TickAge).
    public static long LastInput() {
        LASTINPUTINFO l = new LASTINPUTINFO();
        l.cbSize = (uint)Marshal.SizeOf(typeof(LASTINPUTINFO));
        if (!GetLastInputInfo(ref l)) return -1;
        return (long)l.dwTime;
    }

    // The same clock's current tick, as the low 32 bits of Environment.TickCount.
    public static long TickNow() {
        return (long)unchecked((uint)Environment.TickCount);
    }

    // The last-input tick read straight after this process's latest SendInput; -2 while it has sent none.
    public static long OwnTick = -2;

    public static int[] Pointer() {
        POINT p;
        if (!GetCursorPos(out p)) return new int[0];
        return new int[] { p.X, p.Y };
    }

    public static bool Exists(IntPtr h) {
        return IsWindow(h) && IsWindowVisible(h);
    }

    public static IntPtr[] TopLevel() {
        List<IntPtr> found = new List<IntPtr>();
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            if (IsWindowVisible(h) && GetWindowTextLengthW(h) > 0) found.Add(h);
            return true;
        }, IntPtr.Zero);
        return found.ToArray();
    }

    public static string Title(IntPtr h) {
        int n = GetWindowTextLengthW(h);
        if (n <= 0) return "";
        StringBuilder b = new StringBuilder(n + 1);
        GetWindowTextW(h, b, b.Capacity);
        return b.ToString();
    }

    public static string ClassName(IntPtr h) {
        StringBuilder b = new StringBuilder(256);
        GetClassNameW(h, b, b.Capacity);
        return b.ToString();
    }

    public static uint ProcessId(IntPtr h) {
        uint pid;
        GetWindowThreadProcessId(h, out pid);
        return pid;
    }

    // A Store app's top-level frame belongs to ApplicationFrameHost; the app itself owns a child window.
    public static uint HostedProcessId(IntPtr h) {
        uint own = ProcessId(h);
        uint found = 0;
        EnumChildWindows(h, delegate(IntPtr c, IntPtr l) {
            uint p = ProcessId(c);
            if (p != 0 && p != own) { found = p; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    // Fails while the screen is locked, the user is signed out, or the remote session is disconnected.
    public static bool InputDesktopAvailable() {
        IntPtr d = OpenInputDesktop(0, false, 0x0100);
        if (d == IntPtr.Zero) return false;
        CloseDesktop(d);
        return true;
    }

    public static int[] Rect(IntPtr h) {
        RECT r;
        if (!GetWindowRect(h, out r)) return new int[0];
        return new int[] { r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top };
    }

    public static IntPtr RootAt(int x, int y) {
        POINT p;
        p.X = x;
        p.Y = y;
        IntPtr w = WindowFromPoint(p);
        return w == IntPtr.Zero ? w : GetAncestor(w, 2);
    }

    public static bool BringToFront(IntPtr h) {
        if (GetForegroundWindow() == h) return true;
        if (SetForegroundWindow(h) && GetForegroundWindow() == h) return true;
        // Windows limits foreground changes from a background process; sharing input with the
        // current foreground thread lifts the limit without sending that window any input.
        IntPtr fg = GetForegroundWindow();
        uint pid;
        uint fgThread = fg == IntPtr.Zero ? 0 : GetWindowThreadProcessId(fg, out pid);
        uint self = (uint)AppDomain.GetCurrentThreadId();
        bool attached = fgThread != 0 && fgThread != self && AttachThreadInput(self, fgThread, true);
        try {
            BringWindowToTop(h);
            SetForegroundWindow(h);
        } finally {
            if (attached) AttachThreadInput(self, fgThread, false);
        }
        return GetForegroundWindow() == h;
    }

    static INPUT KeyEvent(ushort vk, bool up) {
        INPUT i = new INPUT();
        i.type = 1;
        i.u.ki.wVk = vk;
        i.u.ki.wScan = (ushort)MapVirtualKeyW(vk, 0);
        uint flags = up ? 2u : 0u;
        if ((vk >= 0x21 && vk <= 0x28) || vk == 0x2D || vk == 0x2E) flags |= 1u;
        i.u.ki.dwFlags = flags;
        return i;
    }

    static bool Send(INPUT[] events) {
        uint sent = SendInput((uint)events.Length, events, Marshal.SizeOf(typeof(INPUT)));
        // Read the clock before anything else can run, so only this tool's own input is counted as its own.
        if (sent > 0) OwnTick = LastInput();
        return sent == (uint)events.Length;
    }

    public static bool SendChord(ushort[] vks) {
        INPUT[] events = new INPUT[vks.Length * 2];
        for (int n = 0; n < vks.Length; n++) {
            events[n] = KeyEvent(vks[n], false);
            events[events.Length - 1 - n] = KeyEvent(vks[n], true);
        }
        if (Send(events)) return true;
        // A partial send can leave a modifier held down for every later input; release them all.
        INPUT[] release = new INPUT[vks.Length];
        for (int n = 0; n < vks.Length; n++) release[n] = KeyEvent(vks[n], true);
        Send(release);
        return false;
    }

    public static bool SendChar(char c) {
        INPUT[] events = new INPUT[2];
        for (int n = 0; n < 2; n++) {
            events[n].type = 1;
            events[n].u.ki.wScan = c;
            events[n].u.ki.dwFlags = n == 0 ? 4u : 6u;
        }
        return Send(events);
    }

    public static bool SendLeftClick() {
        INPUT[] events = new INPUT[2];
        events[0].type = 0;
        events[0].u.mi.dwFlags = 2u;
        events[1].type = 0;
        events[1].u.mi.dwFlags = 4u;
        return Send(events);
    }
}
'@

# Every exit goes through here, so a command that stops partway still records its own last input.
function Exit-Tool([int]$Code) {
    Complete-Run
    exit $Code
}

function Stop-Refused([string]$Message) {
    [Console]::Error.WriteLine("refused: $Message")
    Exit-Tool 3
}

function Stop-Failed([string]$Message) {
    [Console]::Error.WriteLine("error: $Message")
    Exit-Tool 2
}

function Stop-Error([string]$Message) {
    [Console]::Error.WriteLine("error: $Message")
    Exit-Tool 1
}

function Stop-Active([int64]$IdleMs) {
    $seconds = [int64][Math]::Floor($IdleMs / 1000)
    [Console]::Error.WriteLine("error: the operator is active (last input $seconds s ago); nothing was sent; wait and retry")
    Exit-Tool 4
}

# --- Away guard: pure functions (no Windows calls; the tests load these by name) ---

# Milliseconds from Then to Now on the 32-bit tick clock. Unsigned, so it is right across a wrap.
function Get-TickAge([int64]$Now, [int64]$Then) {
    $span = ($Now - $Then) % 4294967296
    if ($span -lt 0) { $span += 4294967296 }
    $span
}

function Get-StateField($State, [string]$Name) {
    $property = $State.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    $property.Value
}

function New-RunState([int64]$OperatorTick, $OwnTick, [int64]$NowTick, [int64]$NowMs, $Foreground, $Pointer) {
    [ordered]@{
        operator_tick = $OperatorTick
        own_tick      = $OwnTick
        tick_at_write = $NowTick
        written_ms    = $NowMs
        foreground    = $Foreground
        pointer       = $Pointer
    }
}

# A missing, unreadable or malformed file is no state at all: it is replaced, never trusted.
function Read-RunState([string]$Path) {
    try {
        if (-not [System.IO.File]::Exists($Path)) { return $null }
        $state = [System.IO.File]::ReadAllText($Path) | ConvertFrom-Json
        foreach ($field in @('operator_tick', 'tick_at_write', 'written_ms')) {
            $value = Get-StateField $state $field
            if ($null -eq $value) { return $null }
            [void][int64]$value
        }
        $own = Get-StateField $state 'own_tick'
        if ($null -ne $own) { [void][int64]$own }
        return $state
    } catch {
        return $null
    }
}

# Writes through a temp file in the same folder and swaps it in, so a reader sees the old file or the
# new one and never half of one. A swap that fails because a reader holds the file is retried.
function Write-RunState([string]$Path, $State) {
    $folder = [System.IO.Path]::GetDirectoryName($Path)
    [void][System.IO.Directory]::CreateDirectory($folder)
    $temp = "$Path.$PID.tmp"
    $json = $State | ConvertTo-Json -Compress -Depth 4
    [System.IO.File]::WriteAllText($temp, $json, (New-Object System.Text.UTF8Encoding($false)))
    $last = $null
    for ($try = 0; $try -lt 25; $try++) {
        try {
            if ([System.IO.File]::Exists($Path)) { [System.IO.File]::Replace($temp, $Path, [NullString]::Value) }
            else { [System.IO.File]::Move($temp, $Path) }
            return
        } catch {
            $last = $_
            Start-Sleep -Milliseconds 20
        }
    }
    try { [System.IO.File]::Delete($temp) } catch { }
    throw $last
}

# True while the state can describe this run: written no more than 30 minutes ago by the wall clock, and
# the tick clock agrees with the wall clock about how long ago. A reboot restarts the ticks, so it breaks
# that agreement; a wrap does not, because both sides are unsigned differences. (A plain "tick greater
# than now" test would call a state written just before a wrap stale, and miss one from before a reboot.)
function Test-RunStateFresh($State, [int64]$NowTick, [int64]$NowMs, [int64]$MaxAgeMs = 1800000, [int64]$SlackMs = 10000) {
    if ($null -eq $State) { return $false }
    $age = $NowMs - [int64](Get-StateField $State 'written_ms')
    if ($age -lt 0 -or $age -gt $MaxAgeMs) { return $false }
    $ticks = Get-TickAge $NowTick ([int64](Get-StateField $State 'tick_at_write'))
    if ([Math]::Abs($ticks - $age) -gt $SlackMs) { return $false }
    # His last input came before the state was written, so it is at least as old as the state.
    $operatorAge = Get-TickAge $NowTick ([int64](Get-StateField $State 'operator_tick'))
    if ($operatorAge + $SlackMs -lt $ticks) { return $false }
    $true
}

# His idle time. If the last input on the machine is the one this tool recorded as its own, nobody has
# touched it since the tool did, so he is idle since his own last input before the run. Any other last
# input (or no usable state) is judged plainly: idle since that input.
function Get-OperatorIdleMs($State, [int64]$NowTick, [int64]$LastInputTick, [int64]$NowMs) {
    if ($null -ne $State -and (Test-RunStateFresh $State $NowTick $NowMs)) {
        $own = Get-StateField $State 'own_tick'
        if ($null -ne $own -and [int64]$own -eq $LastInputTick) {
            return (Get-TickAge $NowTick ([int64](Get-StateField $State 'operator_tick')))
        }
    }
    Get-TickAge $NowTick $LastInputTick
}

# The run goes on when the state is fresh and nothing has been input since the tool's own last input
# (or, before it sent any, since his last one). Otherwise the state is replaced.
function Test-RunContinues($State, [int64]$NowTick, [int64]$LastInputTick, [int64]$NowMs) {
    if ($null -eq $State -or -not (Test-RunStateFresh $State $NowTick $NowMs)) { return $false }
    $own = Get-StateField $State 'own_tick'
    if ($null -ne $own -and [int64]$own -eq $LastInputTick) { return $true }
    $LastInputTick -eq [int64](Get-StateField $State 'operator_tick')
}

# The title as it may be shown: a denied window's title can hold an account or vault name.
function Get-ShownTitle([bool]$Denied, [string]$Title) {
    if ($Denied) { return '(off limits)' }
    ([string]$Title) -replace '[\x00-\x1F\x7F]+', ' '
}

# Rows are { Denied, Minimized }. A minimized window is not on screen, so a capture does not hold it.
function Get-ExposedDeniedCount($Rows) {
    @($Rows | Where-Object { $_.Denied -and -not $_.Minimized }).Count
}

# --- Away guard: state and locking ---

function Get-StatePath {
    $root = if ($env:AF_ROOT) { $env:AF_ROOT } else { Join-Path $PSScriptRoot '..\..' }
    [System.IO.Path]::GetFullPath((Join-Path $root $StateRelative))
}

function Get-NowMs { [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }

# One gated command at a time. Two at once would each pass the guard and then send into the same screen.
function Lock-Run {
    $mutex = New-Object System.Threading.Mutex($false, $RunMutexName)
    $got = $false
    try { $got = $mutex.WaitOne(0) } catch [System.Threading.AbandonedMutexException] { $got = $true }
    if (-not $got) {
        $mutex.Dispose()
        Stop-Error 'another command that acts on the screen is running; nothing was sent; retry when it has ended'
    }
    $script:RunLock = $mutex
}

# What the tool knows right now, with no side effect: the state if it is usable, his idle time, whether
# the run continues.
function Get-AwayStatus {
    $last = [AfDesktop]::LastInput()
    if ($last -lt 0) { Stop-Error 'Windows would not say when the operator last used the computer; nothing was sent' }
    $nowTick = [AfDesktop]::TickNow()
    $nowMs = Get-NowMs
    $read = Read-RunState (Get-StatePath)
    $fresh = if ($null -ne $read -and (Test-RunStateFresh $read $nowTick $nowMs)) { $read } else { $null }
    [pscustomobject]@{
        State     = $fresh
        Stored    = $null -ne $read
        Last      = $last
        NowTick   = $nowTick
        NowMs     = $nowMs
        IdleMs    = Get-OperatorIdleMs $fresh $nowTick $last $nowMs
        Continues = Test-RunContinues $fresh $nowTick $last $nowMs
    }
}

function Get-ForegroundRecord {
    $handle = [AfDesktop]::GetForegroundWindow()
    if ($handle -eq [IntPtr]::Zero) { return $null }
    $info = Get-WindowInfo $handle
    [ordered]@{ handle = $handle.ToInt64(); pid = [int64]$info.ProcessId; process = [string]$info.Process }
}

function Get-PointerRecord {
    $at = [AfDesktop]::Pointer()
    if ($at.Count -ne 2) { return $null }
    [ordered]@{ x = [int]$at[0]; y = [int]$at[1] }
}

# Runs once per gated command, before its first input: takes the lock, and refuses (exit 4) unless he has
# been away for -IdleSeconds. A run's first command records the foreground window and the pointer.
function Enter-AwayGuard {
    if ($IdleSeconds -lt 0) { Stop-Failed '-IdleSeconds must be 0 or more' }
    Lock-Run
    $status = Get-AwayStatus
    if ($status.IdleMs -lt ([int64]$IdleSeconds * 1000)) { Stop-Active $status.IdleMs }
    if ($status.Continues) {
        $kept = $status.State
        $guard = @{
            OperatorTick = [int64](Get-StateField $kept 'operator_tick')
            OwnTick      = Get-StateField $kept 'own_tick'
            Foreground   = Get-StateField $kept 'foreground'
            Pointer      = Get-StateField $kept 'pointer'
        }
    } else {
        $guard = @{
            OperatorTick = $status.Last
            OwnTick      = $null
            Foreground   = Get-ForegroundRecord
            Pointer      = Get-PointerRecord
        }
    }
    $guard.TickBefore = $status.Last
    try { Save-Run $guard $guard.OwnTick }
    catch { Stop-Error "could not write the run state ($($_.Exception.Message)); nothing was sent" }
    $script:Guard = $guard
}

function Save-Run($Guard, $OwnTick) {
    $state = New-RunState $Guard.OperatorTick $OwnTick ([AfDesktop]::TickNow()) (Get-NowMs) $Guard.Foreground $Guard.Pointer
    Write-RunState (Get-StatePath) $state
}

# Ends a gated command: records the last-input tick of the tool's own latest input, keeps his tick, and
# lets the next command run. With no injected input (focus, a pattern invoke), a tick that moved during
# the command is taken as the tool's own; the same is true of any input he made in that short time.
function Complete-Run {
    $guard = $script:Guard
    $script:Guard = $null
    try {
        if ($null -ne $guard) {
            $own = $guard.OwnTick
            if ([AfDesktop]::OwnTick -ne -2) {
                $own = [AfDesktop]::OwnTick
            } else {
                $after = [AfDesktop]::LastInput()
                if ($after -ge 0 -and $after -ne $guard.TickBefore) { $own = $after }
            }
            try { Save-Run $guard $own }
            catch { [Console]::Error.WriteLine("warning: could not write the run state ($($_.Exception.Message)); the next command will wait for the operator to be away") }
        }
    } finally {
        if ($null -ne $script:RunLock) {
            try { $script:RunLock.ReleaseMutex() } catch { }
            $script:RunLock.Dispose()
            $script:RunLock = $null
        }
    }
}

function Remove-RunState {
    try { [System.IO.File]::Delete((Get-StatePath)) } catch { }
}

function Read-LocalDenyNames {
    # Reads the list under this checkout and under AF_ROOT; each adds rules, neither can remove one.
    $roots = @([System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..')))
    if ($env:AF_ROOT) { $roots += [System.IO.Path]::GetFullPath($env:AF_ROOT) }
    $rules = @()
    foreach ($root in ($roots | Select-Object -Unique)) {
        $file = Join-Path $root $LocalDenyPath
        if (-not [System.IO.File]::Exists($file)) { continue }
        $number = 0
        foreach ($line in [System.IO.File]::ReadAllLines($file)) {
            $number++
            $rule = $line.Trim()
            if (-not $rule -or $rule.StartsWith('#')) { continue }
            try { [void](New-Object System.Text.RegularExpressions.Regex($rule)) }
            catch { Stop-Error "line $number of $file is not a valid .NET regex; fix or remove it. Nothing was done." }
            $rules += $rule
        }
    }
    $rules
}

function Format-Text($Value) {
    # On-screen text can hold control characters that would forge lines or columns in the output.
    ([string]$Value) -replace '[\x00-\x1F\x7F]+', ' '
}

function Need([string]$Param, [string]$Why) {
    if (-not $script:Bound.ContainsKey($Param) -or [string]::IsNullOrEmpty([string]$script:Bound[$Param])) {
        Stop-Failed "-$Param is required: $Why"
    }
}

function Get-WindowInfo([IntPtr]$Handle) {
    $owner = [AfDesktop]::ProcessId($Handle)
    $process = ''
    try { $process = (Get-Process -Id $owner -ErrorAction Stop).ProcessName } catch { $process = '' }
    $hosted = ''
    if ($process -eq 'ApplicationFrameHost') {
        $guest = [AfDesktop]::HostedProcessId($Handle)
        if ($guest -ne 0) {
            try { $hosted = (Get-Process -Id $guest -ErrorAction Stop).ProcessName } catch { $hosted = '' }
        }
    }
    [pscustomobject]@{
        Handle    = $Handle
        Hex       = '0x{0:X}' -f $Handle.ToInt64()
        ProcessId = $owner
        Process   = $process
        Hosted    = $hosted
        Title     = [AfDesktop]::Title($Handle)
    }
}

function Format-Window($Info) {
    $guest = if ($Info.Hosted) { " hosting '$($Info.Hosted)'" } else { '' }
    # A refusal names the window it refused, and an off-limits title can hold an account or vault name.
    $shown = Get-ShownTitle ([bool](Get-DenyReason $Info)) $Info.Title
    "process '$($Info.Process)'$guest, title '$shown'"
}

function Test-ControlPanelFolder($Info) {
    # A Control Panel page opens in Explorer under its own title, so only the address bar shows it.
    if ($Info.Process -ne 'explorer' -or [AfDesktop]::ClassName($Info.Handle) -ne 'CabinetWClass') { return $false }
    try {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($Info.Handle)
        $isToolbar = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::ToolBar)
        foreach ($bar in $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $isToolbar)) {
            if ([regex]::IsMatch([string]$bar.Current.Name, '(?i)control panel')) { return $true }
        }
        return $false
    } catch {
        return $true
    }
}

function Get-DenyReason($Info) {
    if (-not $Info.Process) { return 'its process could not be identified' }
    $processes = @($Info.Process, $Info.Hosted) | Where-Object { $_ }
    foreach ($rule in $DenyNames) {
        if ([regex]::IsMatch($Info.Title, $rule)) { return "denylist rule $rule" }
        foreach ($process in $processes) {
            if ([regex]::IsMatch($process, $rule)) { return "denylist rule $rule" }
        }
    }
    foreach ($rule in $script:LocalDenyNames) {
        # The rule names his own institution, so the message does not repeat it.
        if ([regex]::IsMatch($Info.Title, $rule)) { return 'a rule in the local denylist' }
        foreach ($process in $processes) {
            if ([regex]::IsMatch($process, $rule)) { return 'a rule in the local denylist' }
        }
    }
    foreach ($rule in $DenyProcesses) {
        foreach ($process in $processes) {
            if ([regex]::IsMatch($process, $rule)) { return "denylist rule $rule" }
        }
    }
    foreach ($rule in $DenyTitles) {
        if ([regex]::IsMatch($Info.Title, $rule)) { return "denylist rule $rule" }
    }
    if (Test-ControlPanelFolder $Info) { return 'it is a Control Panel page' }
    return $null
}

function Get-TopWindows {
    foreach ($handle in [AfDesktop]::TopLevel()) { Get-WindowInfo $handle }
}

function Resolve-Target([string]$Match) {
    if ([string]::IsNullOrWhiteSpace($Match)) { Stop-Failed '-Window is required: name the window to act on' }
    $found = @(Get-TopWindows | Where-Object { $_.Title.IndexOf($Match, [StringComparison]::OrdinalIgnoreCase) -ge 0 })
    if ($found.Count -eq 0) { Stop-Failed "no visible window title contains '$Match'; run the windows command to list them" }
    $exact = @($found | Where-Object { [string]::Equals($_.Title, $Match, [StringComparison]::OrdinalIgnoreCase) })
    if ($exact.Count -gt 0) { $found = $exact }
    foreach ($candidate in $found) {
        $reason = Get-DenyReason $candidate
        if ($reason) { Stop-Refused "$(Format-Window $candidate) is off limits: $reason" }
    }
    if ($found.Count -gt 1) {
        $names = ($found | ForEach-Object { Format-Window $_ }) -join '; '
        Stop-Failed "'$Match' matches $($found.Count) windows, so none was touched; use a longer match. Found: $names"
    }
    $found[0]
}

function Assert-ReadyForInput($Target, [string]$Progress) {
    $note = if ($Progress) { " $Progress" } else { ' No input was sent.' }
    $foreground = [AfDesktop]::GetForegroundWindow()
    if ($foreground.ToInt64() -ne $Target.Handle.ToInt64()) {
        $seen = if ($foreground -eq [IntPtr]::Zero) { 'no foreground window' } else { Format-Window (Get-WindowInfo $foreground) }
        Stop-Refused "the foreground window is not the requested one. Requested: $(Format-Window $Target). Found: $seen.$note"
    }
    $now = Get-WindowInfo $Target.Handle
    $reason = Get-DenyReason $now
    if ($reason) { Stop-Refused "$(Format-Window $now) is off limits: $reason.$note" }
}

function Assert-InputDesktop {
    if (-not [AfDesktop]::InputDesktopAvailable()) {
        Stop-Error 'the desktop cannot take input: the screen is locked, the user is signed out, or the remote session is disconnected. No input was sent.'
    }
}

function Invoke-BringToFront($Target) {
    Assert-InputDesktop
    $wasForeground = [AfDesktop]::GetForegroundWindow().ToInt64() -eq $Target.Handle.ToInt64()
    if ([AfDesktop]::IsIconic($Target.Handle)) { [void][AfDesktop]::ShowWindow($Target.Handle, 9) }
    [void][AfDesktop]::BringToFront($Target.Handle)
    # A window that has just been raised can still lose the foreground; check after it settles.
    if (-not $wasForeground) { Start-Sleep -Milliseconds $SettleMs }
    Assert-ReadyForInput $Target ''
}

function Invoke-GuardedClick($Target, [int]$ScreenX, [int]$ScreenY) {
    Assert-ReadyForInput $Target ''
    # The foreground check alone would let a click land on another window beside the target.
    $under = [AfDesktop]::RootAt($ScreenX, $ScreenY)
    if ($under -eq [IntPtr]::Zero) { Stop-Refused "no window at $ScreenX,$ScreenY. No input was sent." }
    if ($under.ToInt64() -ne $Target.Handle.ToInt64()) {
        # The target's own menus and dropdowns are separate windows in the same process.
        $over = Get-WindowInfo $under
        $reason = Get-DenyReason $over
        $shell = [AfDesktop]::ClassName($under) -in $ShellSurfaces
        if ($reason -or $shell -or $over.ProcessId -ne $Target.ProcessId) {
            Stop-Refused "$ScreenX,$ScreenY is over another window. Requested: $(Format-Window $Target). Found: $(Format-Window $over). No input was sent."
        }
    }
    if (-not [AfDesktop]::SetCursorPos($ScreenX, $ScreenY)) { Stop-Error "could not move the cursor to $ScreenX,$ScreenY" }
    Assert-ReadyForInput $Target ''
    if (-not [AfDesktop]::SendLeftClick()) { Stop-Error 'Windows rejected the click' }
}

function Assert-NotPasswordField([string]$Progress) {
    $note = if ($Progress) { $Progress } else { 'No input was sent.' }
    # Fails closed: input that cannot be checked is not sent.
    try {
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        $isPassword = $null -ne $focused -and
            [bool]$focused.GetCurrentPropertyValue([System.Windows.Automation.AutomationElement]::IsPasswordProperty)
    } catch {
        Stop-Refused "could not read the focused field, so the password-field check could not run. $note"
    }
    if ($isPassword) { Stop-Refused "the focused field is a password field, and this tool never types passwords. $note" }
}

function Get-Chords([string]$Sequence) {
    $named = @{
        ctrl = 0x11; control = 0x11; shift = 0x10; alt = 0x12
        enter = 0x0D; 'return' = 0x0D; tab = 0x09; esc = 0x1B; escape = 0x1B; space = 0x20; backspace = 0x08
        delete = 0x2E; del = 0x2E; insert = 0x2D; home = 0x24; 'end' = 0x23; pageup = 0x21; pagedown = 0x22
        up = 0x26; down = 0x28; left = 0x25; right = 0x27
        plus = 0xBB; minus = 0xBD; comma = 0xBC; period = 0xBE; slash = 0xBF; semicolon = 0xBA; quote = 0xDE
    }
    foreach ($chord in ($Sequence.Trim() -split '\s+')) {
        $parts = @($chord.ToLowerInvariant() -split '\+')
        $codes = New-Object System.Collections.Generic.List[uint16]
        foreach ($part in $parts) {
            if ($part -in @('win', 'lwin', 'rwin', 'windows')) {
                Stop-Refused "'$chord' uses the Windows key, which acts on the shell rather than the requested window. No input was sent."
            }
            if ($named.ContainsKey($part)) { $codes.Add([uint16]$named[$part]) }
            elseif ($part -match '^f([1-9]|1[0-9]|2[0-4])$') { $codes.Add([uint16](0x6F + [int]$Matches[1])) }
            elseif ($part -cmatch '^[a-z0-9]$') { $codes.Add([uint16][char]$part.ToUpperInvariant()) }
            else { Stop-Failed "unknown key '$part' in '$chord'. No input was sent." }
        }
        $key = $parts[$parts.Count - 1]
        if ($key -in @('ctrl', 'control', 'shift', 'alt')) { Stop-Failed "'$chord' has no key after its modifier. No input was sent." }
        $modifiers = @($parts | Select-Object -First ($parts.Count - 1))
        if (($key -eq 'tab' -and $modifiers -contains 'alt') -or
            ($key -in @('esc', 'escape') -and ($modifiers -contains 'alt' -or $modifiers -contains 'ctrl' -or $modifiers -contains 'control'))) {
            Stop-Refused "'$chord' switches away from the requested window. No input was sent."
        }
        [pscustomobject]@{ Text = $chord; Codes = [uint16[]]$codes.ToArray() }
    }
}

function Assert-IgnoredOut([string]$Path) {
    # A screenshot holds whatever is on screen, and the repository is public.
    $root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..')).TrimEnd('\') + '\'
    $full = [System.IO.Path]::GetFullPath($Path)
    if (-not $full.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)) { return }
    foreach ($ignored in @('workspace\', 'system\sessions\')) {
        if ($full.StartsWith($root + $ignored, [StringComparison]::OrdinalIgnoreCase)) { return }
    }
    Stop-Failed "-Out $full is inside the repository, which is public; write to the scratchpad or under workspace/"
}

function Save-Scaled($Source, [int]$OriginX, [int]$OriginY) {
    $width = [Math]::Min($Source.Width, $MaxWidth)
    $height = [int][Math]::Max(1, [Math]::Round($Source.Height * ($width / [double]$Source.Width)))
    $scale = $width / [double]$Source.Width
    $path = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
    [void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($path))
    $scaled = New-Object System.Drawing.Bitmap($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($scaled)
    try {
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.DrawImage($Source, 0, 0, $width, $height)
        $scaled.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $scaled.Dispose()
    }
    $shown = $scale.ToString('0.######', [System.Globalization.CultureInfo]::InvariantCulture)
    "saved=$path source=$($Source.Width)x$($Source.Height) image=${width}x$height origin=$OriginX,$OriginY scale=$shown"
    "map: screenX = $OriginX + imageX / $shown; screenY = $OriginY + imageY / $shown"
}

function Invoke-Screenshot {
    Need 'Out' 'give the .png path to write'
    if ([System.IO.Path]::GetExtension($Out) -ne '.png') { Stop-Failed '-Out must end in .png' }
    if ($MaxWidth -lt 1) { Stop-Failed '-MaxWidth must be at least 1' }
    Assert-IgnoredOut $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
    if ($script:Bound.ContainsKey('Window')) {
        $target = Resolve-Target $Window
        if ([AfDesktop]::IsIconic($target.Handle)) { Stop-Failed "$(Format-Window $target) is minimized; run the focus command first" }
        $rect = [AfDesktop]::Rect($target.Handle)
        if ($rect.Count -ne 4 -or $rect[2] -lt 1 -or $rect[3] -lt 1) { Stop-Error "could not read the bounds of $(Format-Window $target)" }
        $image = New-Object System.Drawing.Bitmap($rect[2], $rect[3], [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
        $graphics = [System.Drawing.Graphics]::FromImage($image)
        try {
            # PrintWindow draws only this window, so nothing stacked on top of it is captured.
            $hdc = $graphics.GetHdc()
            $printed = [AfDesktop]::PrintWindow($target.Handle, $hdc, 2)
            $graphics.ReleaseHdc($hdc)
            if (-not $printed) { Stop-Error "Windows could not capture $(Format-Window $target)" }
            Save-Scaled $image $rect[0] $rect[1]
        } finally {
            $graphics.Dispose()
            $image.Dispose()
        }
        return
    }
    Assert-InputDesktop
    # A capture of every screen holds whatever is on it, so it is refused while an off-limits window is shown.
    $rows = @(Get-TopWindows | ForEach-Object {
        [pscustomobject]@{ Denied = [bool](Get-DenyReason $_); Minimized = [AfDesktop]::IsIconic($_.Handle) }
    })
    $exposed = Get-ExposedDeniedCount $rows
    if ($exposed -gt 0) {
        Stop-Refused "$exposed off-limits window(s) are visible, so a full-screen capture would include them. Nothing was captured; use -Window to capture one window."
    }
    $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $image = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $graphics = [System.Drawing.Graphics]::FromImage($image)
    try {
        $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bounds.Size)
        Save-Scaled $image $bounds.Left $bounds.Top
    } finally {
        $graphics.Dispose()
        $image.Dispose()
    }
}

function Format-Field($Value) {
    $clean = (Format-Text $Value) -replace '"', '\"'
    if ($clean.Length -gt 200) { $clean = $clean.Substring(0, 200) + '...' }
    $clean
}

function Get-ElementBounds($Element) {
    $r = $Element.Current.BoundingRectangle
    if ($r.IsEmpty -or [double]::IsInfinity($r.X) -or [double]::IsInfinity($r.Width)) { return $null }
    [pscustomobject]@{ X = [int]$r.X; Y = [int]$r.Y; Width = [int]$r.Width; Height = [int]$r.Height }
}

function Format-Element($Element) {
    $info = $Element.Current
    $type = $info.ControlType.ProgrammaticName -replace '^ControlType\.', ''
    $box = Get-ElementBounds $Element
    $where = if ($box) { "$($box.X),$($box.Y),$($box.Width),$($box.Height)" } else { 'none' }
    "$type name=`"$(Format-Field $info.Name)`" id=`"$(Format-Field $info.AutomationId)`" bounds=$where enabled=$($info.IsEnabled)"
}

function Write-TreeNode($Element, [int]$Level) {
    if ($script:TreeCount -ge $TreeNodeLimit) {
        $script:TreeCut = $true
        return
    }
    $script:TreeCount++
    try {
        ('  ' * $Level) + (Format-Element $Element)
    } catch [System.Windows.Automation.ElementNotAvailableException] {
        return
    }
    if ($Level -ge $Depth) { return }
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $child = $walker.GetFirstChild($Element)
    while ($null -ne $child) {
        Write-TreeNode $child ($Level + 1)
        $child = $walker.GetNextSibling($child)
    }
}

function Invoke-Tree {
    Need 'Window' 'name the window to read'
    if ($Depth -lt 0) { Stop-Failed '-Depth must be 0 or more' }
    $target = Resolve-Target $Window
    $script:TreeCount = 0
    $script:TreeCut = $false
    Write-TreeNode ([System.Windows.Automation.AutomationElement]::FromHandle($target.Handle)) 0
    if ($script:TreeCut) { "truncated at $TreeNodeLimit elements; lower -Depth" }
}

function Invoke-Element {
    Need 'Window' 'name the window that holds the element'
    Need 'Name' 'give the element name as the tree command prints it'
    $target = Resolve-Target $Window
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($target.Handle)
    $named = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Name,
        [System.Windows.Automation.PropertyConditionFlags]::IgnoreCase)
    $found = @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $named) | ForEach-Object { $_ })
    if ($found.Count -eq 0) { Stop-Failed "no element named '$Name' in $(Format-Window $target); the tree command lists them" }
    $enabled = @($found | Where-Object { $_.Current.IsEnabled })
    if ($enabled.Count -eq 0) { Stop-Failed "'$Name' is disabled: $(Format-Element $found[0])" }
    if ($enabled.Count -gt 1) {
        $list = ($enabled | ForEach-Object { Format-Element $_ }) -join '; '
        Stop-Failed "'$Name' matches $($enabled.Count) elements, so none was touched; use the click command with coordinates from one of: $list"
    }
    $element = $enabled[0]
    $reason = Get-DenyReason (Get-WindowInfo $target.Handle)
    if ($reason) { Stop-Refused "$(Format-Window $target) is off limits: $reason" }
    # The pattern may send no input at all, yet it presses a button he can see: it is gated like the rest.
    Enter-AwayGuard
    $pattern = $null
    if ($element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        try {
            $pattern.Invoke()
            "invoked via=invoke $(Format-Element $element)"
            return
        } catch {
            [Console]::Error.WriteLine("warning: the Invoke pattern failed ($($_.Exception.Message)); clicking the element centre instead")
        }
    }
    Invoke-BringToFront $target
    $box = Get-ElementBounds $element
    if (-not $box -or $box.Width -lt 1 -or $box.Height -lt 1) { Stop-Failed "'$Name' has no bounds on screen to click" }
    $centreX = $box.X + [int][Math]::Floor($box.Width / 2)
    $centreY = $box.Y + [int][Math]::Floor($box.Height / 2)
    Invoke-GuardedClick $target $centreX $centreY
    "invoked via=click at=$centreX,$centreY $(Format-Element $element)"
}

function Invoke-Click {
    Need 'Window' 'raw input acts only on a named window'
    Need 'X' 'give the screen x coordinate'
    Need 'Y' 'give the screen y coordinate'
    $target = Resolve-Target $Window
    Enter-AwayGuard
    Invoke-BringToFront $target
    Invoke-GuardedClick $target $X $Y
    "clicked at=$X,$Y $(Format-Window $target)"
}

function Invoke-Type {
    Need 'Window' 'raw input acts only on a named window'
    Need 'Text' 'give the text to type'
    $target = Resolve-Target $Window
    Enter-AwayGuard
    Invoke-BringToFront $target
    $chars =($Text -replace "`r`n", "`n").ToCharArray()
    $sent = 0
    foreach ($char in $chars) {
        $progress = "$sent of $($chars.Count) characters were sent."
        # Apps move the focus on their own (auto-advance, autocomplete), so every character checks.
        Assert-ReadyForInput $target $progress
        Assert-NotPasswordField $progress
        $delay = $TypeDelayMs
        $ok = switch ($char) {
            "`n" { $delay = $SettleMs; [AfDesktop]::SendChord([uint16[]]@(0x0D)) }
            "`t" { $delay = $SettleMs; [AfDesktop]::SendChord([uint16[]]@(0x09)) }
            default { [AfDesktop]::SendChar($char) }
        }
        if (-not $ok) { Stop-Error "Windows rejected the input. $progress" }
        $sent++
        # Enter and Tab can retitle the window or move the focus; let that land before the next check.
        Start-Sleep -Milliseconds $delay
    }
    "typed $sent characters into $(Format-Window $target)"
}

function Invoke-Key {
    Need 'Window' 'raw input acts only on a named window'
    Need 'Keys' 'give a chord such as ctrl+s'
    $chords = @(Get-Chords $Keys)
    $target = Resolve-Target $Window
    Enter-AwayGuard
    Invoke-BringToFront $target
    $sent = 0
    foreach ($chord in $chords) {
        $progress = if ($sent -gt 0) { "$sent of $($chords.Count) chords were sent." } else { '' }
        Assert-ReadyForInput $target $progress
        # Single letters, a paste, or Enter into a password field enter a password as surely as type does.
        Assert-NotPasswordField $progress
        if (-not [AfDesktop]::SendChord($chord.Codes)) { Stop-Error "Windows rejected '$($chord.Text)'. $progress" }
        $sent++
        # A chord can switch tabs or retitle the window; let that land before the next check.
        Start-Sleep -Milliseconds $SettleMs
    }
    "sent $Keys to $(Format-Window $target)"
}

function Invoke-Focus {
    Need 'Window' 'name the window to bring to the front'
    $target = Resolve-Target $Window
    Enter-AwayGuard
    Invoke-BringToFront $target
    "focused $($target.Hex) $(Format-Window $target)"
}

# Puts back what the run found (the pointer and the foreground window) and ends the run.
function Invoke-Restore {
    Lock-Run
    $status = Get-AwayStatus
    $state = $status.State
    if ($null -eq $state) {
        Remove-RunState
        'nothing to restore: no run is recorded, or its record is more than 30 minutes old'
        return
    }
    if ($status.IdleMs -lt ([int64]$IdleSeconds * 1000)) { Stop-Active $status.IdleMs }
    Assert-InputDesktop
    $rejected = $false
    $pointer = Get-StateField $state 'pointer'
    if ($null -eq $pointer) {
        'could not restore the pointer: its position was not recorded'
    } elseif ([AfDesktop]::SetCursorPos([int]$pointer.x, [int]$pointer.y)) {
        "restored the pointer to $($pointer.x),$($pointer.y)"
    } else {
        $rejected = $true
        "could not restore the pointer: Windows rejected the move to $($pointer.x),$($pointer.y)"
    }
    $recorded = Get-StateField $state 'foreground'
    if ($null -eq $recorded) {
        'could not restore the foreground window: none was recorded'
    } else {
        $handle = [IntPtr][int64]$recorded.handle
        $name = "process '$($recorded.process)'"
        if (-not [AfDesktop]::Exists($handle) -or [AfDesktop]::ProcessId($handle) -ne [uint32]$recorded.pid) {
            "could not restore the foreground window ($name): it is gone"
        } else {
            # Bringing his own window back is not input to it, so a denied one is put back too, untitled.
            if ([AfDesktop]::IsIconic($handle)) { [void][AfDesktop]::ShowWindow($handle, 9) }
            [void][AfDesktop]::BringToFront($handle)
            Start-Sleep -Milliseconds $SettleMs
            if ([AfDesktop]::GetForegroundWindow().ToInt64() -eq $handle.ToInt64()) {
                "restored the foreground window ($name)"
            } else {
                $rejected = $true
                "could not restore the foreground window ($name): Windows kept another window in front"
            }
        }
    }
    Remove-RunState
    if ($rejected) { Exit-Tool 1 }
}

function Invoke-Windows {
    foreach ($info in Get-TopWindows) {
        $denied = [bool](Get-DenyReason $info)
        $flag = if ($denied) { 'denied' } else { 'ok' }
        "$($info.Hex)`t$($info.Process)`t$flag`t$(Get-ShownTitle $denied $info.Title)"
    }
}

$script:Bound = $PSBoundParameters
$usage = 'usage: desktop.ps1 windows | focus | screenshot | tree | invoke | click | type | key | restore; see the header of this script'
if (-not $Command) { Stop-Failed $usage }
if ($Command -notin @('windows', 'focus', 'screenshot', 'tree', 'invoke', 'click', 'type', 'key', 'restore')) { Stop-Failed "unknown command '$Command'. $usage" }

try { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) } catch { }
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition $NativeSource -IgnoreWarnings
[AfDesktop]::MakeDpiAware()
$script:LocalDenyNames = @(Read-LocalDenyNames)

try {
    switch ($Command) {
        'windows' { Invoke-Windows }
        'focus' { Invoke-Focus }
        'screenshot' { Invoke-Screenshot }
        'tree' { Invoke-Tree }
        'invoke' { Invoke-Element }
        'click' { Invoke-Click }
        'type' { Invoke-Type }
        'key' { Invoke-Key }
        'restore' { Invoke-Restore }
    }
} catch {
    # An unexpected failure still records the tool's own last input before it ends.
    [Console]::Error.WriteLine("error: $($_.Exception.Message)")
    Exit-Tool 1
}
Exit-Tool 0
