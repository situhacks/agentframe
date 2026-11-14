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
# see Get-OperatorIdleMs. A run lasts at most 5 minutes; after that the plain idle rule applies, so
# end a job with restore inside its run. Before every send the tool reads that clock and stops with
# exit 4 (saying how much was sent) if it is not the tool's own last tick, so input he makes between
# two sends is seen. A clock that cannot be read is treated as his input. A call that sends no input
# counts as moving the clock itself only for the Invoke pattern, and only if it returned within 250 ms
# ($ClockOwnLabels); for a raise or a pointer move a moved clock is his. Not seen: his input in the same
# clock tick as a send (about 16 ms), and his input during a pattern Invoke that returns within 250 ms.
# -Verbose or AF_DESKTOP_TRACE=1 prints, for each timed call, whether the clock moved.
# Procedure and standing limits: library/process/desktop-tool.md.

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
# A call that moves the last-input clock within this many ms is taken as the tool's own (Invoke-Timed), but
# only a call whose label is listed here. Every other timed call (SetForegroundWindow, ShowWindow, SetCursorPos)
# is not known to move the clock, so for it a moved clock is his; -Verbose shows whether any of them ever does.
$TimedCallLimitMs = 250
$ClockOwnLabels = @('InvokePattern')
# Explorer owns the taskbar and the desktop, so a click there would pass the same-process check
# for any File Explorer target and could launch an app.
$ShellSurfaces = @('Shell_TrayWnd', 'Shell_SecondaryTrayWnd', 'Progman', 'WorkerW')

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:Guard = $null
$script:RunLock = $null
$script:Trace = ($VerbosePreference -eq 'Continue') -or ($env:AF_DESKTOP_TRACE -eq '1')

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

    // The tool's own latest tick (read straight after its own SendInput, or accepted after a quick API
    // call), -2 while it has none.
    public static long OwnTick = -2;

    // The last-input tick the tool expects to find before its next send or call: what the guard accepted,
    // then each own tick. -2 until the guard arms it, and nothing is sent while it is unarmed.
    public static long Expected = -2;
    public static bool Intruded;
    public static long IntrudedTick;

    public static void Arm(long tick) { Expected = tick; Intruded = false; }
    public static void Accept(long tick) { OwnTick = tick; Expected = tick; }

    public delegate uint SendFn(INPUT[] events);
    public delegate long ClockFn();

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
        return SendGuarded(events, delegate(INPUT[] e) { return SendInput((uint)e.Length, e, Marshal.SizeOf(typeof(INPUT))); }, LastInput);
    }

    // Before every send the last-input tick must still be the one the tool expects. If it is not, the
    // operator touched the machine since the tool's last event: nothing is sent, Intruded is set, and the
    // expected and own ticks stay as they were, so the next command finds a tick that is not the tool's own
    // and falls to the plain idle rule. After a send the clock is read at once and kept as the own tick. A
    // reading below 0 (Windows would not say), before or after a send, is treated as his input: the callers
    // check Intruded after every send and stop. (The send and the clock are parameters so a test can run this
    // with a scripted clock and no input.)
    public static bool SendGuarded(INPUT[] events, SendFn send, ClockFn clock) {
        if (Expected == -2) { Intruded = true; IntrudedTick = -1; return false; }
        long before = clock();
        if (before < 0 || before != Expected) { Intruded = true; IntrudedTick = before; return false; }
        uint sent = send(events);
        if (sent > 0) {
            long after = clock();
            if (after < 0) { Intruded = true; IntrudedTick = after; }
            else Accept(after);
        }
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

function Stop-Active([int64]$IdleMs, [string]$Note = 'nothing was sent') {
    $seconds = [int64][Math]::Floor($IdleMs / 1000)
    [Console]::Error.WriteLine("error: the operator is active (last input $seconds s ago); $($Note.TrimEnd('.')); wait and retry")
    Exit-Tool 4
}

# He touched the machine after the tool's last event: stop at once. Complete-Run still rewrites the state, but
# with the own tick as the tool last made it (never this tick), so the next command sees a last input that is
# not the tool's own and waits for -IdleSeconds of quiet.
function Stop-Intruded([int64]$Tick, [string]$Note) {
    $age = if ($Tick -lt 0) { 0 } else { Get-TickAge ([AfDesktop]::TickNow()) $Tick }
    if (-not $Note) { $Note = 'nothing was sent' }
    Stop-Active $age $Note
}

function Stop-IfIntruded([string]$Note) {
    if ([AfDesktop]::Intruded) { Stop-Intruded ([AfDesktop]::IntrudedTick) $Note }
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

function New-RunState([int64]$OperatorTick, $OwnTick, [int64]$RunStartTick, [int64]$RunStartMs, $Foreground, $Pointer) {
    [ordered]@{
        operator_tick  = $OperatorTick
        own_tick       = $OwnTick
        run_start_tick = $RunStartTick
        run_start_ms   = $RunStartMs
        foreground     = $Foreground
        pointer        = $Pointer
    }
}

# A missing, unreadable or malformed file is no state at all: it is replaced, never trusted.
function Read-RunState([string]$Path) {
    try {
        if (-not [System.IO.File]::Exists($Path)) { return $null }
        $state = [System.IO.File]::ReadAllText($Path) | ConvertFrom-Json
        foreach ($field in @('operator_tick', 'run_start_tick', 'run_start_ms')) {
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

# True while the run can still be described by the state: it began no more than 5 minutes ago by the wall
# clock and by the tick clock, and the two clocks agree about how long ago. Rewriting the state never moves
# the start, so a run has a time limit. A reboot restarts the ticks, so it breaks the agreement; a wrap does
# not, because both sides are unsigned differences. (A plain "tick greater than now" test would call a run
# begun just before a wrap stale, and miss one from before a reboot.)
function Test-RunStateFresh($State, [int64]$NowTick, [int64]$NowMs, [int64]$MaxAgeMs = 300000, [int64]$SlackMs = 10000) {
    if ($null -eq $State) { return $false }
    $age = $NowMs - [int64](Get-StateField $State 'run_start_ms')
    if ($age -lt 0 -or $age -gt $MaxAgeMs) { return $false }
    $ticks = Get-TickAge $NowTick ([int64](Get-StateField $State 'run_start_tick'))
    if ($ticks -gt $MaxAgeMs) { return $false }
    if ([Math]::Abs($ticks - $age) -gt $SlackMs) { return $false }
    # His last input came before the run began, so it is at least as old as the run.
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
# (or, before it sent any, since his last one). Otherwise the state is replaced and a new run begins.
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

# The roots that hold tracked files: this checkout, AF_ROOT, and the main repository of a worktree (the
# parent of git's common directory, found by reading .git and commondir, never by running git).
function Get-RepositoryRoots([string]$Checkout, [string]$AfRoot) {
    $roots = New-Object System.Collections.ArrayList
    foreach ($candidate in @($Checkout, $AfRoot)) {
        if ($candidate) { [void]$roots.Add([System.IO.Path]::GetFullPath($candidate).TrimEnd('\')) }
    }
    try {
        $dotGit = Join-Path $Checkout '.git'
        if ([System.IO.File]::Exists($dotGit)) {
            $line = ([System.IO.File]::ReadAllText($dotGit) -split "`n" | Where-Object { $_ -like 'gitdir:*' } | Select-Object -First 1)
            $raw = $line.Substring(7).Trim() -replace '/', '\'
            # A relative gitdir is relative to the folder that holds the .git file, never the current directory.
            if (-not [System.IO.Path]::IsPathRooted($raw)) { $raw = Join-Path ([System.IO.Path]::GetFullPath($Checkout)) $raw }
            $gitDir = [System.IO.Path]::GetFullPath($raw)
            $commonFile = Join-Path $gitDir 'commondir'
            $common = if ([System.IO.File]::Exists($commonFile)) {
                [System.IO.Path]::GetFullPath((Join-Path $gitDir (([System.IO.File]::ReadAllText($commonFile)).Trim() -replace '/', '\')))
            } else { $gitDir }
            [void]$roots.Add([System.IO.Path]::GetDirectoryName($common.TrimEnd('\')))
        }
    } catch { }
    @($roots | Select-Object -Unique)
}

# True when a screenshot may be written there: outside every root, or inside the ignored workspace/ or
# system/sessions/ of one of them.
function Test-OutAllowed([string]$Full, $Roots) {
    # The \\?\ prefix is the same path (strip it before comparing); any other \\ path is a network share or
    # a device, which this check cannot place, so it is refused outright.
    if ($Full.StartsWith('\\?\UNC\', [StringComparison]::OrdinalIgnoreCase)) { return $false }
    if ($Full.StartsWith('\\?\')) { $Full = $Full.Substring(4) }
    if ($Full.StartsWith('\\')) { return $false }
    $inside = $false
    foreach ($root in $Roots) {
        $prefix = ([string]$root).TrimEnd('\') + '\'
        if (-not $Full.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { continue }
        $inside = $true
        foreach ($ignored in @('workspace\', 'system\sessions\')) {
            if ($Full.StartsWith($prefix + $ignored, [StringComparison]::OrdinalIgnoreCase)) { return $true }
        }
    }
    -not $inside
}

# The cache file for the compiled native code is named by a hash of its source, so a file built from
# other source is never the one loaded.
function Get-NativeCacheName([string]$Source) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $bytes = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($Source)) } finally { $sha.Dispose() }
    'AfDesktop-' + (($bytes[0..7] | ForEach-Object { $_.ToString('x2') }) -join '') + '.dll'
}

# Loads the compiled native class from the cache when the file for exactly this source is there and loads;
# otherwise compiles it, writes the cache through a temp name when it can, and loads that. Any trouble with
# the cache falls back to compiling in memory. Returns where it came from: cache, built, or memory.
function Import-Native([string]$Source, [string]$Folder) {
    $name = Get-NativeCacheName $Source
    $file = $null
    $damaged = $false
    try {
        $file = Join-Path $Folder $name
        if ([System.IO.File]::Exists($file)) {
            Add-Type -Path $file
            return 'cache'
        }
    } catch {
        # A damaged or unloadable file: replace it below. A path that failed to load cannot be loaded again in
        # this process, so this run uses an in-memory build and the next one uses the new file.
        $damaged = $true
        try { [System.IO.File]::Delete($file) } catch { }
    }
    try {
        [void][System.IO.Directory]::CreateDirectory($Folder)
        # Temp files of its own left by a killed compile, once they are an hour old (a newer one may be a
        # compile in progress).
        foreach ($stray in [System.IO.Directory]::GetFiles($Folder, 'AfDesktop-*.tmp')) {
            if ([System.IO.File]::GetLastWriteTimeUtc($stray) -lt [DateTime]::UtcNow.AddHours(-1)) { try { [System.IO.File]::Delete($stray) } catch { } }
        }
        $temp = "$file.$PID.tmp"
        Add-Type -TypeDefinition $Source -IgnoreWarnings -OutputAssembly $temp
        try { [System.IO.File]::Move($temp, $file) }
        catch { if (-not [System.IO.File]::Exists($file)) { throw } else { [System.IO.File]::Delete($temp) } }
        if ($damaged) { Add-Type -TypeDefinition $Source -IgnoreWarnings } else { Add-Type -Path $file }
        foreach ($old in [System.IO.Directory]::GetFiles($Folder, 'AfDesktop-*.dll')) {
            if ([System.IO.Path]::GetFileName($old) -ne $name) { try { [System.IO.File]::Delete($old) } catch { } }
        }
        return 'built'
    } catch {
        try { [System.IO.File]::Delete($temp) } catch { }
        if ('AfDesktop' -as [type]) { return 'memory' }
        Add-Type -TypeDefinition $Source -IgnoreWarnings
        return 'memory'
    }
}

# --- Away guard: state, locking, and timed calls ---

function Get-StateRoot {
    if ($env:AF_ROOT) { $env:AF_ROOT } else { Join-Path $PSScriptRoot '..\..' }
}

function Get-StatePath {
    [System.IO.Path]::GetFullPath((Join-Path (Get-StateRoot) $StateRelative))
}

function Get-NowMs { [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }

# Every gated command takes the same -IdleSeconds check, before anything else is done.
function Assert-Usage {
    if ($GatedCommands -contains $Command -and $IdleSeconds -lt 0) { Stop-Failed '-IdleSeconds must be 0 or more' }
}

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

function Write-Trace([string]$Text) {
    if ($script:Trace) { [Console]::Error.WriteLine("trace: $Text") }
}

# One API call that might bump the system's last-input clock (a raise, a restore, the Invoke pattern, the
# pointer). The clock is read straight before and straight after that call alone. A change counts as the
# tool's own only when the call returned within the limit; a slower call (an Invoke held up by a modal
# dialog) or a clock that already differed before the call is his, so nothing is accepted and the next
# command waits for quiet. With -Verbose or AF_DESKTOP_TRACE=1 each call prints whether the clock moved.
function Invoke-Timed([string]$Label, [scriptblock]$Call) {
    if ([AfDesktop]::Expected -eq -2) { return (& $Call) }
    $before = [AfDesktop]::LastInput()
    if ($before -lt 0 -or $before -ne [AfDesktop]::Expected) { Stop-Intruded $before 'nothing more was sent' }
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $result = & $Call
    $ms = $watch.ElapsedMilliseconds
    $after = [AfDesktop]::LastInput()
    $moved = $after -ne $before
    $accepted = $false
    if ($moved -and $after -ge 0 -and $ms -le $TimedCallLimitMs -and $ClockOwnLabels -contains $Label) {
        [AfDesktop]::Accept($after)
        $accepted = $true
    }
    Write-Trace "$Label clock_moved=$moved ms=$ms accepted=$accepted"
    # A clock that cannot be read after the call is his input like any other.
    if ($after -lt 0) { Stop-Intruded $after 'nothing more was sent' }
    $result
}

# Runs once per gated command, before its first input: takes the lock, and refuses (exit 4) unless he has
# been away for -IdleSeconds. A run's first command records the foreground window and the pointer and its
# start time; a command inside the run keeps both.
function Enter-AwayGuard {
    Lock-Run
    $status = Get-AwayStatus
    if ($status.IdleMs -lt ([int64]$IdleSeconds * 1000)) { Stop-Active $status.IdleMs }
    if ($status.Continues) {
        $kept = $status.State
        $guard = @{
            OperatorTick = [int64](Get-StateField $kept 'operator_tick')
            OwnTick      = Get-StateField $kept 'own_tick'
            RunStartTick = [int64](Get-StateField $kept 'run_start_tick')
            RunStartMs   = [int64](Get-StateField $kept 'run_start_ms')
            Foreground   = Get-StateField $kept 'foreground'
            Pointer      = Get-StateField $kept 'pointer'
        }
    } else {
        $guard = @{
            OperatorTick = $status.Last
            OwnTick      = $null
            RunStartTick = $status.NowTick
            RunStartMs   = $status.NowMs
            Foreground   = Get-ForegroundRecord
            Pointer      = Get-PointerRecord
        }
    }
    try { Save-Run $guard $guard.OwnTick }
    catch { Stop-Error "could not write the run state ($($_.Exception.Message)); nothing was sent" }
    # From here every send and every timed call checks the clock against this tick.
    [AfDesktop]::Arm($status.Last)
    $script:Guard = $guard
}

function Save-Run($Guard, $OwnTick) {
    $state = New-RunState $Guard.OperatorTick $OwnTick $Guard.RunStartTick $Guard.RunStartMs $Guard.Foreground $Guard.Pointer
    Write-RunState (Get-StatePath) $state
}

# Ends a gated command: records the tool's own latest tick (set after its own sends and quick calls only),
# keeps his tick and the run's start, and lets the next command run. A command that stopped because he came
# back leaves the own tick where it was, so the next command finds a last input that is not the tool's.
function Complete-Run {
    $guard = $script:Guard
    $script:Guard = $null
    try {
        if ($null -ne $guard) {
            $own = $guard.OwnTick
            if ([AfDesktop]::OwnTick -ne -2) { $own = [AfDesktop]::OwnTick }
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
    if ([AfDesktop]::IsIconic($Target.Handle)) { [void](Invoke-Timed 'ShowWindow' { [AfDesktop]::ShowWindow($Target.Handle, 9) }) }
    [void](Invoke-Timed 'SetForegroundWindow' { [AfDesktop]::BringToFront($Target.Handle) })
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
    if (-not (Invoke-Timed 'SetCursorPos' { [AfDesktop]::SetCursorPos($ScreenX, $ScreenY) })) { Stop-Error "could not move the cursor to $ScreenX,$ScreenY" }
    Assert-ReadyForInput $Target ''
    if (-not [AfDesktop]::SendLeftClick()) {
        Stop-IfIntruded 'no click was sent'
        Stop-Error 'Windows rejected the click'
    }
    Stop-IfIntruded 'the click was sent'

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
    # A screenshot holds whatever is on screen, and the repository is public. This checkout, AF_ROOT and
    # the main repository of a worktree each have tracked folders; only their workspace/ and system/sessions/
    # are ignored.
    try { $full = [System.IO.Path]::GetFullPath($Path) } catch { Stop-Failed "-Out $Path is not a path this tool can place" }
    $roots = Get-RepositoryRoots ([System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))) $env:AF_ROOT
    if (Test-OutAllowed $full $roots) { return }
    Stop-Failed "-Out $full is inside a repository, which is public; write to the scratchpad or under workspace/"
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
            [void](Invoke-Timed 'InvokePattern' { $pattern.Invoke() })
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
        if (-not $ok) {
            Stop-IfIntruded $progress
            Stop-Error "Windows rejected the input. $progress"
        }
        $sent++
        Stop-IfIntruded "$sent of $($chars.Count) characters were sent."
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
        if (-not [AfDesktop]::SendChord($chord.Codes)) {
            Stop-IfIntruded $progress
            Stop-Error "Windows rejected '$($chord.Text)'. $progress"
        }
        $sent++
        Stop-IfIntruded "$sent of $($chords.Count) chords were sent."
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
        'nothing to restore: no run is recorded, or its record is more than 5 minutes old'
        return
    }
    if ($status.IdleMs -lt ([int64]$IdleSeconds * 1000)) { Stop-Active $status.IdleMs }
    Assert-InputDesktop
    [AfDesktop]::Arm($status.Last)
    $rejected = $false
    $pointer = Get-StateField $state 'pointer'
    if ($null -eq $pointer) {
        'could not restore the pointer: its position was not recorded'
    } elseif (Invoke-Timed 'SetCursorPos' { [AfDesktop]::SetCursorPos([int]$pointer.x, [int]$pointer.y) }) {
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
            if ([AfDesktop]::IsIconic($handle)) { [void](Invoke-Timed 'ShowWindow' { [AfDesktop]::ShowWindow($handle, 9) }) }
            [void](Invoke-Timed 'SetForegroundWindow' { [AfDesktop]::BringToFront($handle) })
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

Assert-Usage

try { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) } catch { }
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
# The compiled native class is cached next to the run state, named by a hash of its source (Import-Native).
$null = Import-Native $NativeSource (Split-Path (Get-StatePath))
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
