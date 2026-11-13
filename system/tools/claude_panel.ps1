# The Claude panel driver, version 2. It manages chats in the Claude Code panel of the one VS Code window
# whose title holds the vault folder name, and nothing else. It sends no key events and no mouse events:
# it reads and acts through Windows UI Automation patterns (read, Invoke, ExpandCollapse, SelectionItem,
# Window) and through the AgentFrame bridge extension (system/vscode-bridge), which opens, reveals and
# closes chat tabs and prefills a chat's input from a file inbox. Text always arrives in a UTF-8 file,
# never as an argument, and is never logged.
#
#   list                                       JSON array: session_id, name, status, active
#   send  -Session <id> -TextFile <path>       send the file's text to that chat (a prompt or a slash command)
#   new   -TextFile <path> -Model <name> [-GroupId <id>]
#                                              start a chat on that model with the file as its first message;
#                                              prints its session id on the last line
#   model -Session <id> [-Model <name>]        switch an idle chat's model; without -Model, list the options
#   close-untitled                             close empty untitled 'Claude Code' tabs (no transcript, no row, no draft)
#   selftest                                   the whole path on a Haiku 5.5 chat it creates
#
# How a send works. Closing a chat's tab ends its process; opening it again with a prompt prefills the
# input (the page does it; the bridge never presses Send). So: reveal the chat, close its tab only after
# one unbroken check, open it with the text, read the input back through UI Automation, and only then
# invoke 'Send message'. Text that starts with a slash is sent as it is. A chat with no open tab skips the
# close. The driver opens a tab only when that adds a tab (a tab someone else opened is never used).
#
# Guards:
#   1. the operator has been idle for -IdleSeconds before any step that changes his screen, and again
#      immediately before 'Send message'; otherwise wait, then exit 3
#   2. the chat is idle: its row says idle and every live session record for it says 'status: idle'
#      (no status is not idle); a live record from outside the VS Code panel (a terminal) is exit 5
#   3. the last thing before any tab close is one unbroken check with no wait after it: the window is not
#      minimized, the target row is the only active row and idle, exactly one Claude tab carries the label
#      and it is the active tab, the records are idle, the input is empty (a draft is exit 4) or, for the
#      driver's own prefilled tab, holds exactly the driver's text; the operator is idle. If a wait was
#      needed, the whole check runs again
#   4. before Send: the input equals the text, the target row is the only active row, Send is enabled;
#      the very elements checked are read again and that same Send button is invoked, with no search in
#      between; otherwise nothing is sent (exit 5)
#   5. one run at a time: every run that changes anything holds system/logs/panel-bridge/driver.lock
#      (pid, session id, action, start); a second run waits within its budget, then exits 3
# The run tracks what it changed (tab closed, tab opened, text prefilled, Send pressed; a bridge step that
# timed out counts as attempted) and every exit says it, then states the tab's final state from a fresh
# read (the bridge's tab list, the chat's process, and its input when that tab is in front). On a failure
# before Send it puts the chat back where that is safe: its own prefilled tab is closed only through the
# unbroken check, as its own (the chat's process is the one its open started), and a tab that was open at
# the start is opened again whenever it is closed. Once the text is sent and confirmed the exit is 0; a
# later cleanup failure is a note, and it closes only a tab it opened in this run that was closed before.
# The window may be minimized: the accessibility tree is stale then, so the driver restores it with the
# Window pattern first and minimizes it again at the end. Every model pick saves the global model key and
# the settings file hash first, writes the key back right after the pick, and verifies (exit 6 if the
# file does not match).
# Deadline: a run ends within -WaitMinutes plus $WorkSeconds; every step is bounded by it, and the chat is
# put back before it: $RestoreSeconds to wait for him, then $SettleSeconds of its own for the chat's
# process to end and its tab to come back. af's timeout for a run is that plus a margin (panel.rs:
# DRIVER_WORK_SECS). A list reads only and has a short deadline of its own ($ListWorkSeconds).
# Exit codes: 0 done and confirmed; 1 failed or could not confirm; 2 bridge, window, chat or control not
# found; 3 operator active, chat busy or another run holding the lock; 4 the input holds text that is
# not the driver's; 5 a safety check failed; 6 the model key or settings file does not match after a
# pick. 3, 4 and 5 mean nothing was sent.
# Every run appends one line to system/logs/claude-panel.log: time, action, session id, exit code, seconds.

param(
    [Parameter(Position = 0)][string]$Action = '',
    [string]$Session = '',
    [string]$TextFile = '',
    [string]$Model = '',
    [string]$GroupId = '',
    [int]$IdleSeconds = 120,
    [double]$WaitMinutes = 20
)

$ErrorActionPreference = 'Stop'
try { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) } catch { }

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, WindowsBase
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public class PanelExit : Exception {
    public int Code;
    public string Reason;
    public PanelExit(int code, string reason) : base(reason) { Code = code; Reason = reason; }
}

public static class PanelNative {
    [StructLayout(LayoutKind.Sequential)]
    struct LII { public uint cbSize; public uint dwTime; }
    [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LII p);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("kernel32.dll")] static extern bool GetProcessTimes(IntPtr h, out long c, out long e, out long k, out long u);
    // This process's start as a FILETIME (what af and Claude's session records write), so a lock is
    // never mistaken for this run's once the pid is reused.
    public static long OwnStart() {
        long c, e, k, u;
        if (!GetProcessTimes(System.Diagnostics.Process.GetCurrentProcess().Handle, out c, out e, out k, out u)) return 0;
        return c;
    }
    // Milliseconds since the last input by anyone (the operator: this driver sends none).
    public static double IdleMs() {
        LII l = new LII();
        l.cbSize = (uint)Marshal.SizeOf(typeof(LII));
        GetLastInputInfo(ref l);
        return unchecked((uint)Environment.TickCount - l.dwTime);
    }
    public static string Title(IntPtr h) { StringBuilder sb = new StringBuilder(512); GetWindowTextW(h, sb, 512); return sb.ToString(); }
}
'@

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$CT = [System.Windows.Automation.ControlType]
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
# The vault root is the main checkout, also when this script runs from a git worktree under .claude\worktrees.
$VaultRoot = [regex]::Replace($Root, '\\\.claude\\worktrees\\[^\\]+$', '')
$LogDir = Join-Path $VaultRoot 'system\logs'
$LogFile = Join-Path $LogDir 'claude-panel.log'
$BridgeBase = Join-Path $VaultRoot 'system\logs\panel-bridge'
$LockFile = Join-Path $BridgeBase 'driver.lock'
$RowPrefix = 'sessions-list-row-'
$UntitledLabel = 'Claude Code'
$PanelEntrypoint = 'claude-vscode'
$MaxChars = 200000
# The time a run may take beyond -WaitMinutes: close, reopen, read-back, Send, the confirmation (a
# /compact may take 10 minutes) and putting the chat back. panel.rs DRIVER_WORK_SECS is the same number.
$WorkSeconds = 1200
# The part of it kept for putting the chat and the window back after a failure: waiting for him, then
# a budget of its own for the chat's process to end and its tab to come back.
$RestoreSeconds = 120
$SettleSeconds = 90
# A list reads only: its work time (panel.rs DRIVER_LIST_WORK_SECS is the same number).
$ListWorkSeconds = 45
$Utf8 = New-Object System.Text.UTF8Encoding($false)

$script:Win = $null
$script:Hwnd = [IntPtr]::Zero
$script:WaitUntil = (Get-Date).AddMinutes($WaitMinutes)
$script:HardUntil = $script:WaitUntil.AddSeconds($WorkSeconds)
$script:RestoreSecs = $RestoreSeconds
$script:SettleSecs = $SettleSeconds
$script:StepUntil = $script:HardUntil.AddSeconds(-($RestoreSeconds + $SettleSeconds))
$script:Restoring = $false
$script:RestoreUntil = $script:HardUntil
$script:ScreenBegun = $false
$script:WasMinimized = $false
$script:PrevActive = @()
$script:Notes = @()
$script:TxCache = @{}
$script:Lock = $null
$script:St = $null
$script:Clock = [Diagnostics.Stopwatch]::StartNew()

function Stop-Panel([int]$Code, [string]$Reason) {
    throw (New-Object PanelExit($Code, $Reason))
}

function Add-Note([string]$Note) { $script:Notes += $Note }

# The PanelExit inside an error record, or $null for any other error.
function Get-PanelExit($Err) {
    $e = $Err.Exception
    while ($null -ne $e) {
        if ($e -is [PanelExit]) { return $e }
        $e = $e.InnerException
    }
    $null
}

# ---------------------------------------------------------------- deadlines

function Set-Deadlines([string]$ActionName = '') {
    $work = $WorkSeconds
    $script:RestoreSecs = $RestoreSeconds
    $script:SettleSecs = $SettleSeconds
    if ($ActionName -eq 'list') { $work = $ListWorkSeconds; $script:RestoreSecs = 15; $script:SettleSecs = 0 }
    $now = Get-Date
    $script:WaitUntil = $now.AddMinutes($WaitMinutes)
    $script:HardUntil = $script:WaitUntil.AddSeconds($work)
    $script:StepUntil = $script:HardUntil.AddSeconds(-($script:RestoreSecs + $script:SettleSecs))
}

# How long a wait may last: the wait budget while nothing is changed yet, the step deadline once
# something is, and the hard deadline while the chat is being put back.
function Get-WaitUntil {
    if ($script:Restoring) { return $script:RestoreUntil }
    if ($null -ne $script:St -and (Test-Touched)) { return $script:StepUntil }
    $script:WaitUntil
}

# From now on the run is putting things back: it waits for him at most $RestoreSeconds, and stops waiting
# for him early enough to leave the settle budget before the hard deadline.
function Start-Restoring {
    $script:Restoring = $true
    $limit = $script:HardUntil.AddSeconds(-$script:SettleSecs)
    $script:RestoreUntil = (Get-Date).AddSeconds($script:RestoreSecs)
    if ($script:RestoreUntil -gt $limit) { $script:RestoreUntil = $limit }
}

# A step's own limit in milliseconds, cut to what is left before the step deadline (while putting things
# back, before the hard deadline: those steps do not wait for him, so his time does not spend them).
function Get-StepMs([int]$Ms) {
    $end = $script:StepUntil
    if ($script:Restoring) { $end = $script:HardUntil }
    $left = ($end - (Get-Date)).TotalMilliseconds
    if ($left -lt 0) { $left = 0 }
    [int][Math]::Min([double]$Ms, $left)
}

# ---------------------------------------------------------------- what the run changed

function New-RunState([string]$Id, [string]$Text) {
    $script:St = [pscustomobject]@{
        Id = $Id; RowId = $Id; Text = $Text; OwnLabel = ''; OwnPid = 0
        Label = ''; LabelBase = 0
        HadTab = $false; TabOpen = $false
        Closed = $false; CloseAsked = $false; OpenCalled = $false; Opened = $false; Prefilled = $false; Sent = $false
        Cleaned = $false; Reopened = $false
    }
}

function Test-Touched { $s = $script:St; [bool]($s.Closed -or $s.CloseAsked -or $s.OpenCalled) }

# What the run changed in the panel (from what it did), then the state of the chat's tab now (from a fresh
# read, never from what it did).
function Get-StateText {
    $s = $script:St
    if ($null -eq $s) { return '' }
    if (-not (Test-Touched)) { return "The panel was not changed: the chat's tab is as it was." }
    $done = @()
    if ($s.Closed) { $done += "closed the chat's tab" }
    elseif ($s.CloseAsked) { $done += "asked the bridge to close the chat's tab (it did not confirm)" }
    if ($s.OpenCalled) {
        if ($s.Prefilled) { $done += 'opened it with the text in the input' }
        elseif ($s.Opened) { $done += 'opened its tab' }
        else { $done += 'asked the bridge to open it (no new tab was seen)' }
    }
    if ($s.Sent) { $done += 'pressed Send' }
    if ($s.Cleaned) { $done += 'closed that tab again to take the unsent text away' }
    if ($s.Reopened) { $done += "opened the chat's tab again" }
    'The driver ' + ($done -join ', then ') + '. ' + (Read-ChatState)
}

# The chat's tab now, read fresh from the bridge's tab list (never from the chat's process, which lives on
# for a while after its tab closes), and when it is open and in front, what its input holds. When the tab
# is closed but the process has not ended, it says so.
function Read-ChatState {
    $s = $script:St
    $tab = Get-TabState
    if ($tab -eq 'unknown') { return "The state of the chat's tab is unknown: the bridge's tab list could not be read." }
    if ($tab -eq 'closed') {
        $t = "The chat's tab is now closed."
        if (Test-ProcessLingers) { $t = "The chat's tab is now closed; its process has not ended yet." }
        if ($s.HadTab) { $t += ' It was open before the run.' }
        return $t
    }
    $in = Read-ChatInput
    $kind = ''
    if ($null -ne $in) { $kind = $in.Kind }
    $t = "The chat's tab is now open"
    if ($kind -eq 'text') { $t = "The chat's tab is OPEN and its input holds the driver's unsent text: clear the input or close the tab before the next send." }
    elseif ($kind -eq 'empty') { $t += ' and its input is empty.' }
    elseif ($kind -eq 'draft') { $t += ' and its input holds text that is not the driver''s.' }
    elseif ($kind -eq 'unrecognised') { $t += " and its input shows $($in.Length) characters the driver does not recognise while Send is disabled (a placeholder the driver does not know, or his text)." }
    else { $t += " (its input was not read: the chat's tab is not the one in front)." }
    if (-not $s.HadTab) { $t += ' It was closed before the run.' }
    $t
}

# The chat's tab by the label rule: 'open' when more Claude tabs carry its label than the other chats'
# tabs that carried it at the start, 'closed' when not, 'unknown' when the tab list cannot be read or the
# run never learned the label.
function Get-TabState {
    $s = $script:St
    $label = $s.Label
    if ($label -eq '') { $label = $s.OwnLabel }
    if ($label -eq '') { return 'unknown' }
    $n = $null
    try { $n = @(Get-ClaudeTabs | Where-Object { $_.label -ceq $label }).Count } catch { return 'unknown' }
    if ($n - $s.LabelBase -ge 1) { return 'open' }
    'closed'
}

# The chat's process still runs (its tab may be closed already; it outlives the tab for a while).
function Test-ProcessLingers {
    $s = $script:St
    if ($s.Id -eq '') { return $false }
    try { return (@(Get-ChatRecords $s.Id).Count -gt 0) } catch { return $false }
}

# Records the chat's tab label and how many tabs of other chats carry it now (labels are cut at 25
# characters, so two chats can share one).
function Set-TabLabel([string]$Label) {
    $s = $script:St
    $s.Label = $Label
    $n = @(Get-ClaudeTabs | Where-Object { $_.label -ceq $Label }).Count
    if ($s.HadTab) { $n-- }
    if ($n -lt 0) { $n = 0 }
    $s.LabelBase = $n
}

# The command that opens the chat's tab later, for a note when the driver could not.
function Get-RevealCommand([string]$Id) {
    "python system/tools/panel_bridge.py reveal --json '{""sessionId"":""$Id""}'"
}

# What the chat's input holds, when its tab is the one in front: its read (Kind 'empty', 'text' (the
# driver's), 'draft' or 'unrecognised', and Length), or $null when it cannot be read.
function Read-ChatInput {
    $s = $script:St
    if ($null -eq $script:Win) { return $null }
    try {
        $act = @(Get-ActiveIds)
        if ($s.RowId -ne '') {
            if (-not ($act.Count -eq 1 -and $act[0] -eq $s.RowId)) { return $null }
        } elseif ($act.Count -ne 0) { return $null }
        $pair = Get-InputPair
        if ($null -eq $pair) { return $null }
        $r = Get-InputRead $pair $s.Text
        if ($r.Kind -eq 'unreadable') { return $null }
        return $r
    } catch { return $null }
}

# Runs the part of an action that changes the chat. On a failure the chat is put back where that is safe
# (never after Send was pressed), and the reason gains what changed and the tab's final state.
function Invoke-Guarded([scriptblock]$Body) {
    try {
        return (& $Body)
    } catch {
        $pe = Get-PanelExit $_
        $code = 1
        $reason = ''
        if ($null -ne $pe) { $code = $pe.Code; $reason = $pe.Reason }
        else { $reason = 'unexpected: ' + $_.Exception.Message + ' (line ' + $_.InvocationInfo.ScriptLineNumber + ')' }
        if (-not $script:St.Sent) { Restore-Chat }
        if ($script:St.Sent -and $code -ge 3 -and $code -le 5) { $code = 1 }
        $state = Get-StateText
        if ($code -ge 3 -and $code -le 5) { $reason = $reason.TrimEnd('.') + '. Nothing was sent. ' + $state }
        elseif ($script:St.Sent) { $reason = $reason.TrimEnd('.') + '. Send was pressed, so the text may have reached the chat. ' + $state }
        else { $reason = $reason.TrimEnd('.') + '. Nothing was sent. ' + $state }
        Stop-Panel $code $reason
    }
}

# ---------------------------------------------------------------- the bridge (a file inbox)

# One bridge action; returns the parsed result, or $null when nothing answered within the timeout.
function Invoke-Bridge([string]$Name, $BArgs, [int]$TimeoutSec = 25) {
    $req = Join-Path $BridgeBase 'requests'
    $res = Join-Path $BridgeBase 'results'
    New-Item -ItemType Directory -Force -Path $req, $res | Out-Null
    $id = (Get-Date).ToString('yyyyMMddTHHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
    $payload = @{}
    if ($null -ne $BArgs) { $payload = $BArgs }
    $body = [ordered]@{ id = $id; action = $Name; args = $payload; ts = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
    $json = ConvertTo-Json -InputObject $body -Compress -Depth 6
    $tmp = Join-Path $req ($id + '.tmp')
    $file = Join-Path $req ($id + '.json')
    [System.IO.File]::WriteAllText($tmp, $json, $Utf8)
    Move-Item -LiteralPath $tmp -Destination $file
    $resultFile = Join-Path $res ($id + '.json')
    $until = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $until) {
        if (Test-Path -LiteralPath $resultFile) {
            $text = $null
            try { $text = [System.IO.File]::ReadAllText($resultFile, $Utf8) } catch { Start-Sleep -Milliseconds 50; continue }
            $parsed = $null
            try { $parsed = $text | ConvertFrom-Json } catch { Start-Sleep -Milliseconds 50; continue }
            Remove-Item -LiteralPath $resultFile -Force -ErrorAction SilentlyContinue
            return $parsed
        }
        Start-Sleep -Milliseconds 50
    }
    Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
    $null
}

# A bridge action that must succeed; its result object.
function Invoke-BridgeOk([string]$Name, $BArgs) {
    $r = Invoke-Bridge $Name $BArgs 25
    if ($null -eq $r) { Stop-Panel 2 "the bridge did not answer '$Name' within 25 seconds" }
    if (-not $r.ok) { Stop-Panel 1 "the bridge refused '$Name': $($r.error)" }
    $r.result
}

function Test-Bridge {
    $r = Invoke-Bridge 'ping' $null 10
    if ($null -eq $r -or -not $r.ok) {
        Stop-Panel 2 ('the AgentFrame bridge extension does not answer ping within 10 seconds. Install it once: ' +
            'python system\vscode-bridge\build.py, then code --install-extension system\logs\panel-bridge\af-bridge-<version>.vsix ' +
            '(do not reload the window; a first install needs no restart). It runs only in the VS Code window whose folder is the vault.')
    }
}

function Get-BridgeTabs {
    $r = Invoke-BridgeOk 'tabs' $null
    @($r.tabs)
}

function Get-ClaudeTabs { @(Get-BridgeTabs | Where-Object { $_.claude }) }

# Closes the first Claude tab with exactly this label ($Count tabs carried it at the check). The installed
# bridge 0.1.0 closes every match unless told otherwise, so all = $false is always passed; 0.2.0 checks
# again that the tab is the active one right before it closes (0.1.0 ignores requireActive). The attempt
# is recorded first: when the bridge does not answer in time it may still carry it out, so the tab list
# is read again to learn what happened.
function Close-TabByLabel([string]$Label, [int]$Count) {
    $s = $script:St
    if ($null -ne $s -and -not $script:Restoring -and -not $s.Opened) { $s.CloseAsked = $true }
    $r = Invoke-Bridge 'close-tab' @{ label = $Label; all = $false; requireActive = $true } 25
    if ($null -eq $r) {
        $now = $null
        try { $now = @(Get-ClaudeTabs | Where-Object { $_.label -ceq $Label }).Count } catch { $now = $null }
        if ($null -eq $now) { Stop-Panel 2 "the bridge did not answer close-tab within 25 seconds and the tab list could not be read, so whether the chat's tab closed is unknown" }
        if ($now -lt $Count) { Add-Note 'the bridge did not answer close-tab in time, but the tab list shows the tab closed'; return }
        Stop-Panel 2 'the bridge did not answer close-tab within 25 seconds; the tab list still shows the tab'
    }
    if (-not $r.ok) {
        if ([string]$r.error -like '*not the active tab*') { Stop-Panel 5 'the bridge did not close the tab: it was no longer the active tab when the close ran' }
        Stop-Panel 1 "the bridge refused 'close-tab': $($r.error)"
    }
    if ($r.result.closed -ne 1) { Stop-Panel 5 "close-tab closed $($r.result.closed) tabs, not 1" }
}

# VS Code cuts a tab label at 25 characters with an ellipsis.
function Get-TabLabel([string]$Title) {
    if ($Title.Length -gt 25) { return ($Title.Substring(0, 24) + [string][char]0x2026) }
    $Title
}

# ---------------------------------------------------------------- the lock

# Who holds the lock, from its file: pid, action, session, start. Read only.
function Read-LockHolder {
    try {
        $fs = [System.IO.File]::Open($LockFile, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]'ReadWrite, Delete')
        try {
            $sr = New-Object System.IO.StreamReader($fs, $Utf8)
            $j = $sr.ReadToEnd() | ConvertFrom-Json
        } finally { $fs.Dispose() }
        return ('pid {0}, {1}, session {2}, since {3}' -f $j.pid, $j.action, $(if ([string]$j.session_id -ne '') { $j.session_id } else { '-' }), $j.started)
    } catch { return 'its holder could not be read' }
}

# One run at a time. The lock is the open file itself (shared for reading only), so it dies with its
# process: a lock file left by a run that ended is stale and is simply taken over. A run that finds it
# held waits within its wait budget, then exits 3.
function Enter-Lock([string]$ActionName, [string]$Id) {
    if ($null -ne $script:Lock) { return }
    New-Item -ItemType Directory -Force -Path $BridgeBase | Out-Null
    while ($true) {
        $fs = $null
        try {
            $fs = [System.IO.File]::Open($LockFile, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::Read)
        } catch {
            $e = $_.Exception
            while ($e -is [System.Management.Automation.MethodInvocationException] -and $null -ne $e.InnerException) { $e = $e.InnerException }
            if (-not ($e -is [System.IO.IOException] -or $e -is [System.UnauthorizedAccessException])) { throw }
            $fs = $null
        }
        if ($null -ne $fs) {
            $body = ConvertTo-Json -Compress -InputObject ([ordered]@{ pid = $PID; pid_start = [PanelNative]::OwnStart(); session_id = $Id; action = $ActionName; started = (Get-Date).ToString('yyyy-MM-ddTHH:mm:ssK') })
            $bytes = $Utf8.GetBytes($body)
            $fs.SetLength(0)
            $fs.Write($bytes, 0, $bytes.Length)
            $fs.Flush()
            $script:Lock = $fs
            return
        }
        if ((Get-Date) -ge $script:WaitUntil) { Stop-Panel 3 ('another driver run held the panel for the whole wait (' + (Read-LockHolder) + ')') }
        Start-Sleep -Seconds 2
    }
}

function Exit-Lock {
    if ($null -eq $script:Lock) { return }
    try { $script:Lock.Dispose() } catch { }
    $script:Lock = $null
    # Fails, harmlessly, when the next run already holds it.
    try { [System.IO.File]::Delete($LockFile) } catch { }
}

# ---------------------------------------------------------------- the idle guard and the session records

function Wait-Idle([string]$What) {
    while ($true) {
        if ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000)) { return }
        if ((Get-Date) -ge (Get-WaitUntil)) { Stop-Panel 3 "the operator kept using the computer until the deadline ($What)" }
        Start-Sleep -Seconds 2
    }
}

function Get-ConfigDir {
    if ($env:CLAUDE_CONFIG_DIR) { return $env:CLAUDE_CONFIG_DIR }
    Join-Path $env:USERPROFILE '.claude'
}

function Test-PidAlive($ProcId) {
    $n = 0
    if (-not [int]::TryParse([string]$ProcId, [ref]$n)) { return $true }
    try { $p = [System.Diagnostics.Process]::GetProcessById($n); return (-not $p.HasExited) } catch { return $false }
}

# The live session records under the config dir's sessions\: a record whose pid is dead is left over and
# ignored (one with no pid is kept: a process cannot be ruled out). Each carries its file in _file.
function Get-Records {
    $dir = Join-Path (Get-ConfigDir) 'sessions'
    $out = @()
    if (-not (Test-Path -LiteralPath $dir)) { return $out }
    foreach ($f in Get-ChildItem -LiteralPath $dir -Filter '*.json' -File -ErrorAction SilentlyContinue) {
        $r = $null
        try { $r = [System.IO.File]::ReadAllText($f.FullName, $Utf8) | ConvertFrom-Json } catch { continue }
        if ($null -eq $r) { continue }
        if ($null -ne $r.pid -and -not (Test-PidAlive $r.pid)) { continue }
        $r | Add-Member -NotePropertyName '_file' -NotePropertyValue $f.FullName -Force
        $out += $r
    }
    $out
}

function Get-ChatRecords([string]$Id) { @(Get-Records | Where-Object { [string]$_.sessionId -eq $Id }) }

# Every live record for the chat: Count; Busy, '' when all say 'status: idle', else why not (a missing
# status is not idle); Terminal, '' unless a record comes from outside the VS Code panel.
function Get-RecordState([string]$Id) {
    $recs = @(Get-ChatRecords $Id)
    $busy = ''
    $term = ''
    foreach ($r in $recs) {
        $ep = [string]$r.entrypoint
        if ($ep -ne $PanelEntrypoint) {
            if ($ep -eq '') { $ep = 'none' }
            $term = "a process outside the VS Code panel holds the chat (entrypoint $ep, pid $($r.pid)): a terminal chat is never driven from the panel, because opening it there would start a second process on its transcript"
        }
        $st = [string]$r.status
        if ($st -ne 'idle') {
            if ($st -eq '') { $st = 'no status' }
            $busy = "its session record (pid $($r.pid)) says $st"
        }
    }
    [pscustomobject]@{ Count = $recs.Count; Busy = $busy; Terminal = $term }
}

# Seconds since the record's process started (its startedAt in ms, else the file's creation time).
function Get-RecordAgeSeconds($Rec) {
    $ms = 0.0
    if ($null -ne $Rec.startedAt -and [double]::TryParse([string]$Rec.startedAt, [ref]$ms) -and $ms -gt 1e11) {
        return ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() - $ms) / 1000.0
    }
    try { return ((Get-Date).ToUniversalTime() - (Get-Item -LiteralPath $Rec._file).CreationTimeUtc).TotalSeconds } catch { return 0.0 }
}

# Guards 1 and 2 before the screen is touched: operator idle and the chat's records idle, up to -WaitMinutes.
function Wait-Guards([string]$Id) {
    while ($true) {
        $why = ''
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { $why = 'the operator is using the computer' }
        elseif ($Id -ne '') {
            $rs = Get-RecordState $Id
            if ($rs.Terminal -ne '') { Stop-Panel 5 $rs.Terminal }
            $why = $rs.Busy
        }
        if ($why -eq '') { return }
        if ((Get-Date) -ge $script:WaitUntil) { Stop-Panel 3 "gave up after $WaitMinutes minutes: $why" }
        Start-Sleep -Seconds 2
    }
}

# ---------------------------------------------------------------- the window

function Get-VaultName { Split-Path -Leaf $VaultRoot }

function Test-VaultTitle([string]$Title, [string]$Vault) {
    if ([string]::IsNullOrEmpty($Vault) -or [string]::IsNullOrEmpty($Title)) { return $false }
    if ($Title.IndexOf($Vault, [StringComparison]::OrdinalIgnoreCase) -lt 0) { return $false }
    return ($Title.IndexOf('Visual Studio Code', [StringComparison]::OrdinalIgnoreCase) -ge 0)
}

function Find-Window {
    $vault = Get-VaultName
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Chrome_WidgetWin_1')
    $found = @()
    foreach ($w in $AE::RootElement.FindAll($TS::Children, $cond)) {
        if (-not (Test-VaultTitle $w.Current.Name $vault)) { continue }
        $proc = Get-Process -Id $w.Current.ProcessId -ErrorAction SilentlyContinue
        if ($null -eq $proc -or $proc.ProcessName -ne 'Code') { continue }
        $found += $w
    }
    if ($found.Count -eq 0) { Stop-Panel 2 "no VS Code window with '$vault' in its title" }
    if ($found.Count -gt 1) { Stop-Panel 2 "more than one VS Code window has '$vault' in its title; close the extra one" }
    $found[0]
}

function Set-WindowState([string]$State) {
    $wp = $script:Win.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
    $wp.SetWindowVisualState([System.Windows.Automation.WindowVisualState]::$State)
}

function Find-Named($win, [string]$Name, $Type) {
    $c1 = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $Name)
    $c2 = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $Type)
    $and = New-Object System.Windows.Automation.AndCondition($c1, $c2)
    @($win.FindAll($TS::Descendants, $and) | ForEach-Object { $_ })
}

# Chromium builds the accessibility tree only after a first query, so the panel is read until it answers.
function Wait-Tree {
    for ($i = 0; $i -lt 6; $i++) {
        if (@(Find-Named $script:Win 'New session' $CT::Button).Count -ge 1) { return }
        Start-Sleep -Milliseconds 1500
    }
    Stop-Panel 2 'the Claude panel session list is not showing in the VS Code window'
}

function Open-Window {
    $script:Win = Find-Window
    $script:Hwnd = [IntPtr][int64]$script:Win.Current.NativeWindowHandle
}

# Before the first read or step on the screen: the window is made visible (a minimized window's tree is
# stale), and his active chat is noted so it can be restored. Runs once per run.
function Ensure-Screen {
    if ($script:ScreenBegun) { return }
    Open-Window
    if ([PanelNative]::IsIconic($script:Hwnd)) {
        Wait-Idle 'restore the minimized VS Code window'
        Set-WindowState 'Normal'
        $script:WasMinimized = $true
        Start-Sleep -Milliseconds 2500
    }
    Wait-Tree
    $script:ScreenBegun = $true
    $script:PrevActive = @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
}

# Puts his active chat back, and the window as it was. Never fights him: if he is at the machine it waits
# a little (within the run's deadline) and then says it did not restore.
function Restore-Screen {
    if (-not $script:ScreenBegun) { return }
    $script:ScreenBegun = $false
    try {
        $limit = (Get-Date).AddSeconds(60)
        if ($limit -gt $script:RestoreUntil) { $limit = $script:RestoreUntil }
        while ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000) -and (Get-Date) -lt $limit) { Start-Sleep -Seconds 2 }
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { Add-Note 'his view was not restored: the operator is at the machine'; return }
        if ($script:PrevActive.Count -eq 1) {
            $want = $script:PrevActive[0]
            $now = @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
            if (-not ($now.Count -eq 1 -and $now[0] -eq $want)) {
                $row = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $want })
                if ($row.Count -eq 1) {
                    $row[0].Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    for ($i = 0; $i -lt 12; $i++) {
                        Start-Sleep -Milliseconds 400
                        $now = @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
                        if ($now.Count -eq 1 -and $now[0] -eq $want) { break }
                    }
                    if (-not ($now.Count -eq 1 -and $now[0] -eq $want)) { Add-Note 'his previous chat is not active again' }
                } else { Add-Note 'his previous chat has no row any more' }
            }
        } else { Add-Note 'he had no chat row active, so none was restored' }
    } catch {
        Add-Note ('restore error: ' + $_.Exception.Message)
    } finally {
        if ($script:WasMinimized) {
            try { Set-WindowState 'Minimized' } catch { Add-Note 'could not minimize the window again' }
            $script:WasMinimized = $false
        }
    }
}

# ---------------------------------------------------------------- reading the panel

function Get-Chats($win) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Button)
    $tw = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $out = @()
    foreach ($b in $win.FindAll($TS::Descendants, $cond)) {
        $id = ''
        try { $id = [string]$b.Current.AutomationId } catch { continue }
        if ($id -eq '' -or -not $id.StartsWith($RowPrefix, [StringComparison]::Ordinal)) { continue }
        $status = 'none'
        $ch = $tw.GetFirstChild($b)
        while ($null -ne $ch) {
            $cls = $ch.Current.ClassName
            if ($cls -like '*statusDot*') {
                if ($cls -like '*Running*') { $status = 'running' } elseif ($cls -like '*Idle*') { $status = 'idle' }
                break
            }
            $ch = $tw.GetNextSibling($ch)
        }
        # The row's name is the chat title and an age (now, 12h, 4d).
        $name = [regex]::Replace($b.Current.Name, '\s+(now|\d+\s?[smhdwy])$', '')
        $out += [pscustomobject]@{
            Id = $id.Substring($RowPrefix.Length); Name = $name; Status = $status
            Active = [bool]($b.Current.ClassName -match '(^|\s)active_'); Element = $b
        }
    }
    $out
}

function Get-Chat([string]$Id) {
    $rows = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id })
    if ($rows.Count -ne 1) { Stop-Panel 2 "no chat row for session $Id in the panel" }
    $rows[0]
}

function Get-ActiveIds { @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id }) }

# A row element read again by itself, with no search.
function Test-RowActive($Row) { [bool]($Row.Element.Current.ClassName -match '(^|\s)active_') }

# The one visible message input, and the Send button in the same chat document (the nearest Document
# above the input that holds exactly one visible Send). $null when that is not what the screen shows.
function Get-InputPair {
    $boxes = @(Find-Named $script:Win 'Message input' $CT::Edit | Where-Object { -not $_.Current.IsOffscreen })
    if ($boxes.Count -ne 1) { return $null }
    $tw = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $p = $tw.GetParent($boxes[0])
    while ($null -ne $p) {
        if ($p.Current.ControlType -eq $CT::Document) {
            $sends = @(Find-Named $p 'Send message' $CT::Button | Where-Object { -not $_.Current.IsOffscreen })
            if ($sends.Count -eq 1) { return [pscustomobject]@{ Input = $boxes[0]; Send = $sends[0]; Doc = $p } }
            if ($sends.Count -gt 1) { return $null }
        }
        $p = $tw.GetParent($p)
    }
    $null
}

function Get-InputValue($El) {
    $o = $null
    if (-not $El.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$o)) { return $null }
    [string]$o.Current.Value
}

function ConvertTo-PlainValue([string]$Value) {
    if ($null -eq $Value) { return '' }
    $v = $Value.Replace([string][char]0xA0, ' ').Replace([string][char]0x200B, '').Replace([string][char]0xFEFF, '')
    $v.Replace("`r`n", "`n").Replace("`r", "`n").TrimEnd([char]10)
}

# What an empty input reads as: the page shows a placeholder through CSS (:empty:before with
# data-placeholder), and UI Automation reads that placeholder as the value. These are the fixed ones the
# Claude Code page uses (2.1.322); a prompt suggestion shown in its place is not fixed, so it reads as text
# the driver cannot tell from a draft, and nothing is closed over it.
function Get-Placeholders {
    $e = [string][char]0x2026
    $out = @(('Ask Claude to edit' + $e), ('Queue another message' + $e))
    foreach ($k in @('ctrl esc', ([string][char]0x2318 + ' Esc'))) {
        $out += ($k + ' to focus or unfocus Claude')
        $out += ($k + ' to attach selected text')
    }
    $out
}

function Test-EmptyValue([string]$Value) {
    $plain = (ConvertTo-PlainValue $Value).Trim()
    if ($plain -eq '') { return $true }
    foreach ($ph in Get-Placeholders) { if ($plain -ceq $ph) { return $true } }
    $false
}

# What the input holds, from its value read now: Kind 'text' (exactly $Text, when given), 'empty' (Send is
# disabled and the value is blank or a known placeholder), 'draft' (Send is enabled: his draft, an
# attachment), 'unrecognised' (Send is disabled but the value is no placeholder the driver knows: a
# suggestion, a new placeholder, or his text), or 'unreadable'; and Length, the value's length. The value
# itself is never returned, so it cannot reach a message or the log.
function Get-InputRead($Pair, [string]$Text) {
    $v = $null
    $enabled = $false
    try { $v = Get-InputValue $Pair.Input; $enabled = [bool]$Pair.Send.Current.IsEnabled } catch { return [pscustomobject]@{ Kind = 'unreadable'; Length = 0 } }
    if ($null -eq $v) { return [pscustomobject]@{ Kind = 'unreadable'; Length = 0 } }
    $len = (ConvertTo-PlainValue $v).Trim().Length
    $kind = 'draft'
    if ($Text -ne '' -and (Test-SameText $v $Text)) { $kind = 'text' }
    elseif (-not $enabled) {
        if (Test-EmptyValue $v) { $kind = 'empty' } else { $kind = 'unrecognised' }
    }
    [pscustomobject]@{ Kind = $kind; Length = $len }
}

function Get-InputKind($Pair, [string]$Text) { (Get-InputRead $Pair $Text).Kind }

# Why an input that is neither empty nor the driver's text stops a close (exit 4).
function Get-InputWhy($Read, [string]$Mode) {
    if ($Read.Kind -eq 'unrecognised') {
        return ("the chat's input shows $($Read.Length) characters of text the driver does not recognise while Send is disabled; " +
            'if he has no draft there, the placeholder list in the driver (Get-Placeholders) needs an update')
    }
    if ($Mode -eq 'empty') { return 'the chat has a draft typed in its input box (its input is not empty)' }
    "the input holds text that is not exactly the driver's"
}

# The input equals the text exactly. A text with line breaks may be read back with other breaks, so for those
# alone the comparison ignores white space; every other character must match.
function Test-SameText([string]$Value, [string]$Text) {
    $plain = ConvertTo-PlainValue $Value
    if ($plain -ceq $Text) { return $true }
    if ($Text.Contains("`n")) {
        $a = [regex]::Replace($plain, '\s+', '')
        $b = [regex]::Replace($Text, '\s+', '')
        if ($a -ceq $b -and $a.Length -gt 0) { return $true }
    }
    $false
}

# Whether the panel shows what a send needs, and the very elements it checked: Why ($null when ready),
# Pair (input, Send, their document) and Row (the target row; $null for a new chat, which has none).
function Test-Ready([string]$RowId, [string]$Text) {
    $rows = @(Get-Chats $script:Win)
    $act = @($rows | Where-Object { $_.Active })
    $row = $null
    if ($RowId -ne '') {
        if (-not ($act.Count -eq 1 -and $act[0].Id -eq $RowId)) { return [pscustomobject]@{ Why = "the target row is not the only active row ($($act.Count) active)"; Pair = $null; Row = $null } }
        $row = $act[0]
    } elseif ($act.Count -ne 0) { return [pscustomobject]@{ Why = "a chat row is active ($($act.Count)), so this is not the new chat's tab"; Pair = $null; Row = $null } }
    $pair = Get-InputPair
    if ($null -eq $pair) { return [pscustomobject]@{ Why = 'one message input with its Send button is not on screen'; Pair = $null; Row = $null } }
    if (-not (Test-SameText (Get-InputValue $pair.Input) $Text)) { return [pscustomobject]@{ Why = 'the input does not hold the text'; Pair = $null; Row = $null } }
    if (-not $pair.Send.Current.IsEnabled) { return [pscustomobject]@{ Why = 'Send is not enabled'; Pair = $null; Row = $null } }
    [pscustomobject]@{ Why = $null; Pair = $pair; Row = $row }
}

function Wait-Ready([string]$RowId, [string]$Text, [int]$Ms) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limit = Get-StepMs $Ms
    $why = 'not read'
    while ($sw.ElapsedMilliseconds -lt $limit) {
        $r = Test-Ready $RowId $Text
        if ($null -eq $r.Why) { return $r }
        $why = $r.Why
        Start-Sleep -Milliseconds 250
    }
    Stop-Panel 5 "read-back failed ($why)"
}

function Get-Combo {
    $c = @(Find-Named $script:Win 'Switch model' $CT::ComboBox | Where-Object { -not $_.Current.IsOffscreen })
    if ($c.Count -ne 1) { Stop-Panel 2 'the model picker is not showing' }
    $c[0]
}

function Get-Pill {
    try { [string](Get-Combo).GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value } catch { '' }
}

# ---------------------------------------------------------------- transcripts

function Find-Transcript([string]$Id) {
    if ($script:TxCache.ContainsKey($Id)) { return $script:TxCache[$Id] }
    $dir = Join-Path (Get-ConfigDir) 'projects'
    $f = @(Get-ChildItem -LiteralPath $dir -Recurse -Filter "$Id.jsonl" -ErrorAction SilentlyContinue | Select-Object -First 1)
    if ($f.Count -eq 0) { return '' }
    $script:TxCache[$Id] = $f[0].FullName
    $f[0].FullName
}

# Where the transcript ends now (0 when there is none yet).
function Get-TxMark([string]$Id) {
    $p = Find-Transcript $Id
    $len = 0
    if ($p -ne '') { $len = (Get-Item -LiteralPath $p).Length }
    [pscustomobject]@{ Len = [long]$len }
}

# What was appended after the mark: user lines, compact_boundary lines, the last custom title.
function Read-TxSince([string]$Id, $Mark) {
    $facts = [pscustomobject]@{ User = 0; Boundary = 0; Title = '' }
    $p = Find-Transcript $Id
    if ($p -eq '') { return $facts }
    $text = ''
    try {
        $fs = New-Object System.IO.FileStream($p, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        try {
            [void]$fs.Seek([Math]::Min($Mark.Len, $fs.Length), [System.IO.SeekOrigin]::Begin)
            $buf = New-Object byte[] ($fs.Length - $fs.Position)
            [void]$fs.Read($buf, 0, $buf.Length)
            $text = $Utf8.GetString($buf)
        } finally { $fs.Dispose() }
    } catch { return $facts }
    foreach ($line in $text -split "`n") {
        if ($line.Contains('"compact_boundary"')) { $facts.Boundary++ }
        if ($line.Contains('"custom-title"')) {
            try { $j = $line | ConvertFrom-Json; if ($j.type -eq 'custom-title') { $facts.Title = [string]$j.customTitle } } catch { }
        }
        if ($line.Contains('"type":"user"') -and -not $line.Contains('"isMeta":true')) { $facts.User++ }
    }
    $facts
}

# ---------------------------------------------------------------- tabs

# Bring the chat's tab to the front (the bridge's reveal; for a chat with no tab this opens one) and wait
# a little for its row to be the only active one. The check that follows says whether it worked.
function Show-Chat([string]$Id) {
    [void](Invoke-BridgeOk 'reveal' @{ sessionId = $Id })
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limit = Get-StepMs 8000
    while ($sw.ElapsedMilliseconds -lt $limit) {
        $act = @(Get-ActiveIds)
        if ($act.Count -eq 1 -and $act[0] -eq $Id) { Start-Sleep -Milliseconds 700; return }
        Start-Sleep -Milliseconds 250
    }
}

# Untitled tabs: the chats with no transcript and no row must all be idle, panel chats, and older than 60
# seconds (a younger one may be about to get its first message). '' when they are, else why not.
function Get-UntitledBlock {
    $rows = @(Get-Chats $script:Win | ForEach-Object { $_.Id })
    $cand = @(Get-Records | Where-Object { $rows -notcontains [string]$_.sessionId -and (Find-Transcript ([string]$_.sessionId)) -eq '' })
    if ($cand.Count -lt 1) { return 'no live chat without a transcript and a row is left' }
    foreach ($c in $cand) {
        if ([string]$c.entrypoint -ne $PanelEntrypoint) { return 'an empty chat process is not a panel chat' }
        if ([string]$c.status -ne 'idle') { return 'an empty chat process is not idle' }
        if ((Get-RecordAgeSeconds $c) -lt 60) { return 'an empty chat started less than 60 seconds ago' }
    }
    ''
}

# The unbroken check before a tab close: reads only, no wait. $RowId is the chat whose row must be the
# only active one ('' for an untitled tab: no row active); $RecId the chat whose records must be idle
# ('' none known, '*' the untitled rule); $Mode 'empty' (the input must hold nothing: a draft is exit 4)
# or 'text' (the input must hold exactly the driver's text); $ExpectLabel the tab's label, or '' for the
# label of the active Claude tab. With $FirstOfMany, several tabs may carry the label when the active one
# is the first of them (close-tab closes the first match). With $OwnPid, the tab must be the driver's own:
# the chat's one process is the one the driver's open started (a tab he reopened has another). Returns
# Code 0, the label to close by and how many tabs carry it, or a Code, a Why and whether waiting may help.
function Test-SafeToClose([string]$RowId, [string]$RecId, [string]$Mode, [string]$ExpectLabel, [bool]$FirstOfMany = $false, [int]$OwnPid = 0) {
    if ([PanelNative]::IsIconic($script:Hwnd)) { return [pscustomobject]@{ Code = 3; Why = 'the VS Code window is minimized'; Retry = $true; Label = '' } }
    $claude = @(Get-ClaudeTabs)
    $label = $ExpectLabel
    if ($label -eq '') {
        $act = @($claude | Where-Object { $_.isActive })
        if ($act.Count -ne 1) { return [pscustomobject]@{ Code = 5; Why = "$($act.Count) Claude tabs are active, so the chat's tab cannot be told"; Retry = $true; Label = '' } }
        $label = [string]$act[0].label
    }
    $same = @($claude | Where-Object { $_.label -ceq $label })
    if ($same.Count -lt 1) { return [pscustomobject]@{ Code = 5; Why = "no Claude tab carries the chat's label"; Retry = $true; Label = '' } }
    if ($same.Count -gt 1 -and -not $FirstOfMany) { return [pscustomobject]@{ Code = 5; Why = "$($same.Count) Claude tabs carry the chat's label (labels are cut at 25 characters)"; Retry = $false; Label = '' } }
    if (-not $same[0].isActive) { return [pscustomobject]@{ Code = 5; Why = "the chat's tab is not the active tab"; Retry = $true; Label = '' } }
    $rows = @(Get-Chats $script:Win)
    $act = @($rows | Where-Object { $_.Active })
    if ($RowId -ne '') {
        if (-not ($act.Count -eq 1 -and $act[0].Id -eq $RowId)) { return [pscustomobject]@{ Code = 5; Why = "the chat's row is not the only active row"; Retry = $true; Label = '' } }
        if ($act[0].Status -ne 'idle') { return [pscustomobject]@{ Code = 3; Why = "the chat's row says $($act[0].Status)"; Retry = $true; Label = '' } }
    } elseif ($act.Count -ne 0) { return [pscustomobject]@{ Code = 5; Why = 'a chat row is active, so the active tab is not an untitled chat'; Retry = $true; Label = '' } }
    if ($RecId -eq '*') {
        $block = Get-UntitledBlock
        if ($block -ne '') { return [pscustomobject]@{ Code = 5; Why = $block; Retry = $false; Label = '' } }
    } elseif ($RecId -ne '') {
        $rs = Get-RecordState $RecId
        if ($rs.Terminal -ne '') { return [pscustomobject]@{ Code = 5; Why = $rs.Terminal; Retry = $false; Label = '' } }
        if ($rs.Busy -ne '') { return [pscustomobject]@{ Code = 3; Why = $rs.Busy; Retry = $true; Label = '' } }
        if ($OwnPid -gt 0) {
            $own = @(Get-ChatRecords $RecId)
            if (-not ($own.Count -eq 1 -and [string]$own[0].pid -eq [string]$OwnPid)) {
                return [pscustomobject]@{ Code = 5; Why = "the chat's tab is not the one the driver opened (its process is not the one the driver's open started)"; Retry = $false; Label = '' }
            }
        }
    }
    $pair = Get-InputPair
    if ($null -eq $pair) { return [pscustomobject]@{ Code = 5; Why = "the chat's input is not on screen"; Retry = $true; Label = '' } }
    $want = ''
    if ($Mode -eq 'text') { $want = [string]$script:St.Text }
    $read = Get-InputRead $pair $want
    $kind = $read.Kind
    if ($kind -eq 'unreadable') { return [pscustomobject]@{ Code = 5; Why = "the chat's input could not be read"; Retry = $true; Label = '' } }
    if ($Mode -eq 'empty') {
        if ($kind -ne 'empty') { return [pscustomobject]@{ Code = 4; Why = (Get-InputWhy $read $Mode); Retry = $false; Label = '' } }
    } else {
        # The driver's own tab: its text exactly, or an empty input, is safe to close.
        if ($kind -ne 'text' -and $kind -ne 'empty') { return [pscustomobject]@{ Code = 4; Why = (Get-InputWhy $read $Mode); Retry = $false; Label = '' } }
    }
    if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { return [pscustomobject]@{ Code = 3; Why = 'the operator is using the computer'; Retry = $true; Label = '' } }
    [pscustomobject]@{ Code = 0; Why = $null; Retry = $false; Label = $label; Count = $same.Count }
}

# Closes a chat tab after the unbroken check, then waits for the chat's process to end (a budget of its own,
# not what is left of the wait for him). Whenever the check fails in a way waiting may cure, the whole check
# (and the reveal, when $RevealId is given) runs again.
function Close-Checked([string]$RowId, [string]$RecId, [string]$Mode, [string]$ExpectLabel, [string]$RevealId = '', [bool]$FirstOfMany = $false, [int]$OwnPid = 0) {
    $first = $true
    while ($true) {
        Wait-Idle 'close a chat tab'
        if ($RevealId -ne '') { Show-Chat $RevealId }
        elseif (-not $first) { Start-Sleep -Milliseconds 600 }
        $first = $false
        $c = Test-SafeToClose $RowId $RecId $Mode $ExpectLabel $FirstOfMany $OwnPid
        if ($c.Code -eq 0) {
            # Nothing waits between the check above and this close.
            Close-TabByLabel $c.Label $c.Count
            $script:St.TabOpen = $false
            if (-not $script:Restoring -and -not $script:St.Opened) { $script:St.Closed = $true }
            break
        }
        if (-not $c.Retry -or (Get-Date) -ge (Get-WaitUntil)) { Stop-Panel $c.Code ('the tab was not closed: ' + $c.Why) }
    }
    if ($RecId -ne '' -and $RecId -ne '*') {
        if (-not (Wait-ProcessEnd $RecId)) { Stop-Panel 1 "the tab was closed but the chat's process did not end in time" }
    }
}

# After a tab close: the chat's process ends within $SettleSeconds of its own (never past the hard deadline).
function Wait-ProcessEnd([string]$Id) {
    $end = (Get-Date).AddSeconds($script:SettleSecs)
    if ($end -gt $script:HardUntil) { $end = $script:HardUntil }
    while ((Get-Date) -lt $end) {
        if (@(Get-ChatRecords $Id).Count -eq 0) { return $true }
        Start-Sleep -Milliseconds 250
    }
    (@(Get-ChatRecords $Id).Count -eq 0)
}

# Opens the chat (no tab, no process) with the text prefilled. The tab set is read right before and after:
# when the open adds no tab, the chat's tab was opened by someone else meanwhile, so the prompt was
# dropped and that tab is not the driver's (it is left alone). The open is recorded before it is asked
# for, and the process it starts is noted, so a later close can prove the tab is still the driver's.
function Open-WithText([string]$Id, [string]$Text) {
    $s = $script:St
    Wait-Idle 'open the chat with the text'
    $n0 = @(Get-ClaudeTabs).Count
    if (@(Get-ChatRecords $Id).Count -gt 0) { Stop-Panel 5 "the chat has a process again (someone opened its tab after the driver closed it); the driver opened nothing and left that tab alone" }
    $s.OpenCalled = $true
    $r = Invoke-Bridge 'open' @{ sessionId = $Id; prompt = $Text } 25
    if ($null -ne $r -and -not $r.ok) { Stop-Panel 1 "the bridge refused 'open': $($r.error)" }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limit = Get-StepMs 10000
    while ($sw.ElapsedMilliseconds -lt $limit) {
        if (@(Get-ClaudeTabs).Count -eq $n0 + 1) {
            $s.Opened = $true; $s.Prefilled = $true; $s.TabOpen = $true
            if ($null -eq $r) { Add-Note 'the bridge did not answer open in time, but the tab list shows the tab opened' }
            $s.OwnPid = Get-OwnPid $Id
            return
        }
        Start-Sleep -Milliseconds 300
    }
    if ($null -eq $r) { Stop-Panel 2 'the bridge did not answer open within 25 seconds and no tab was added' }
    Stop-Panel 5 'the open added no tab, so the chat was already open somewhere else and the prompt was dropped; that tab was left as it is'
}

# The pid of the one process a driver open just started for the chat (0 when it cannot be told).
function Get-OwnPid([string]$Id) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limit = Get-StepMs 10000
    while ($sw.ElapsedMilliseconds -lt $limit) {
        $recs = @(Get-ChatRecords $Id)
        if ($recs.Count -eq 1) { $n = 0; if ([int]::TryParse([string]$recs[0].pid, [ref]$n)) { return $n } }
        if ($recs.Count -gt 1) { return 0 }
        Start-Sleep -Milliseconds 250
    }
    0
}

# After a failure before Send: take the driver's own unsent text away by closing the tab it opened (through
# the unbroken check, as its own tab, with its exact text or an empty input), then, whatever happened to
# that, open the chat's tab again if it was open at the start and is closed now. Open or closed is read
# from the bridge's tab list, never from the chat's process: the process outlives its tab for a while, and
# the panel opens no tab for a chat whose process still runs, so the reopen waits for it to end first.
function Restore-Chat {
    $s = $script:St
    if ($null -eq $s -or $s.Sent -or -not (Test-Touched)) { return }
    Start-Restoring
    try {
        if ($s.Opened -and (Get-TabState) -eq 'open') {
            $mode = 'empty'
            if ($s.Prefilled) { $mode = 'text' }
            try {
                Close-Checked $s.RowId $s.Id $mode $s.OwnLabel $s.RowId $false $s.OwnPid
                $s.Cleaned = $true
            } catch {
                $pe = Get-PanelExit $_
                $why = $_.Exception.Message
                if ($null -ne $pe) { $why = $pe.Reason }
                $now = Get-TabState
                if ($now -eq 'closed') {
                    # The close went through (late, or its process lives on): the unsent text is gone.
                    $s.Cleaned = $true
                    if (Test-ProcessLingers) { Add-Note 'the tab the driver opened was closed; its process has not ended yet' }
                    else { Add-Note ('the tab the driver opened is closed, though the close reported: ' + $why) }
                }
                elseif ($now -eq 'unknown') { Add-Note ('whether the tab the driver opened was closed is unknown (the tab list could not be read): ' + $why) }
                else { Add-Note ('the tab the driver opened was not closed: ' + $why) }
            }
        }
        if ($s.HadTab -and $s.Id -ne '') {
            $tab = Get-TabState
            if ($tab -eq 'unknown') {
                Add-Note ("whether the chat's tab is open is unknown (the tab list could not be read), so it was not opened again; to open it: " + (Get-RevealCommand $s.Id))
            } elseif ($tab -eq 'closed') {
                try {
                    if (-not (Wait-ProcessEnd $s.Id)) {
                        Add-Note ("the chat's tab was not opened again: its process has not ended yet, and the panel opens no tab while it runs; to open it once it has ended: " + (Get-RevealCommand $s.Id))
                    } else {
                        Wait-Idle "open the chat's tab again"
                        [void](Invoke-BridgeOk 'reveal' @{ sessionId = $s.Id })
                        $sw = [Diagnostics.Stopwatch]::StartNew()
                        $limit = Get-StepMs 10000
                        while ($sw.ElapsedMilliseconds -lt $limit) {
                            if ((Get-TabState) -eq 'open') { $s.Reopened = $true; $s.TabOpen = $true; break }
                            Start-Sleep -Milliseconds 300
                        }
                        if (-not $s.Reopened) { Add-Note ("the chat's tab did not come back after reveal; to open it: " + (Get-RevealCommand $s.Id)) }
                    }
                } catch {
                    $pe = Get-PanelExit $_
                    $why = $_.Exception.Message
                    if ($null -ne $pe) { $why = $pe.Reason }
                    Add-Note ("the chat's tab was not opened again: " + $why + '; to open it: ' + (Get-RevealCommand $s.Id))
                }
            }
        }
    } finally { $script:Restoring = $false }
}

function Select-UntitledTab {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)
    $items = @($script:Win.FindAll($TS::Descendants, $cond) | Where-Object { $_.Current.Name -like 'Claude Code,*' })
    if ($items.Count -lt 1) { return $false }
    $p = $items[0].GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
    $p.Select()
    for ($i = 0; $i -lt 20; $i++) { if ($p.Current.IsSelected) { break }; Start-Sleep -Milliseconds 150 }
    Start-Sleep -Milliseconds 900
    $true
}

# ---------------------------------------------------------------- the model key and the picker

function Read-ModelLine {
    $path = Join-Path (Get-ConfigDir) 'settings.json'
    $bytes = [System.IO.File]::ReadAllBytes($path)
    $text = $Utf8.GetString($bytes)
    $m = [regex]::Matches($text, '(?m)^(?<ind>[ \t]*)"model"[ \t]*:[ \t]*"(?<val>[^"]*)"(?<comma>,?)(?<cr>\r?)$')
    if ($m.Count -ne 1) { Stop-Panel 5 "settings.json has $($m.Count) lines that set the model; the key cannot be saved and restored safely, so no model was picked" }
    [pscustomobject]@{ Path = $path; Text = $text; Match = $m[0]; Value = $m[0].Groups['val'].Value }
}

function Write-ModelValue([string]$Value) {
    $cur = Read-ModelLine
    $mm = $cur.Match
    $line = $mm.Groups['ind'].Value + '"model": "' + $Value + '"' + $mm.Groups['comma'].Value + $mm.Groups['cr'].Value
    $text = $cur.Text.Substring(0, $mm.Index) + $line + $cur.Text.Substring($mm.Index + $mm.Length)
    [System.IO.File]::WriteAllBytes($cur.Path, $Utf8.GetBytes($text))
}

function Get-SettingsHash {
    (Get-FileHash -LiteralPath (Join-Path (Get-ConfigDir) 'settings.json') -Algorithm SHA256).Hash
}

# A picker option or the picker's value names the model when it is the name, or the name and a space
# (the rest is a description or an effort level): 'Opus 5' never matches 'Opus 5.5 ...'.
function Test-ModelMatch([string]$Option, [string]$Name) {
    if ([string]::IsNullOrEmpty($Option) -or [string]::IsNullOrEmpty($Name)) { return $false }
    ($Option -ieq $Name) -or $Option.StartsWith($Name + ' ', [StringComparison]::OrdinalIgnoreCase)
}

# With no name: the options and the active one (read only). With a name: save the global model key and the
# file hash, pick, write the key back at once, verify (exit 6 on a mismatch), confirm the picker shows it.
function Select-Model([string]$Name) {
    $combo = Get-Combo
    $ec = $combo.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
    $k0 = $null
    $sha0 = ''
    if ($Name -ne '') {
        $k0 = Read-ModelLine
        $sha0 = Get-SettingsHash
    }
    $result = ''
    $restored = $true
    try {
        Wait-Idle 'open the model picker'
        $ec.Expand()
        $items = @()
        for ($i = 0; $i -lt 16; $i++) {
            Start-Sleep -Milliseconds 250
            $items = @($script:Win.FindAll($TS::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::ListItem))) |
                Where-Object { $_.Current.ClassName -like '*modelItem_*' })
            if ($items.Count -gt 0) { break }
        }
        if ($items.Count -eq 0) { Stop-Panel 2 'the model picker listed no options' }
        if ($Name -eq '') {
            $active = @($items | Where-Object { $_.Current.ClassName -like '*activeModelItem_*' })
            $names = ($items | ForEach-Object { $_.Current.Name }) -join ' | '
            $result = ('{0} options: {1}; active: {2}' -f $items.Count, $names, $(if ($active.Count -ge 1) { $active[0].Current.Name } else { 'none' }))
        } else {
            $match = @($items | Where-Object { Test-ModelMatch $_.Current.Name $Name })
            if ($match.Count -ne 1) {
                $names = ($items | ForEach-Object { $_.Current.Name }) -join ' | '
                Stop-Panel 2 "$($match.Count) model options match '$Name'; the options are: $names"
            }
            if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { Stop-Panel 3 'the operator became active before the model pick; nothing was picked' }
            $restored = $false
            $o = $null
            if ($match[0].TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$o)) { $o.Invoke() }
            elseif ($match[0].TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$o)) { $o.Select() }
            else { Stop-Panel 1 'the model option offers no way to choose it' }
            # The pick writes the global key; wait for the write, then put the old value back at once.
            $sw = [Diagnostics.Stopwatch]::StartNew()
            while ($sw.ElapsedMilliseconds -lt 6000) {
                if ((Read-ModelLine).Value -ne $k0.Value) { break }
                Start-Sleep -Milliseconds 40
            }
        }
    } finally {
        if ($Name -ne '') {
            try {
                if ((Read-ModelLine).Value -ne $k0.Value) { Write-ModelValue $k0.Value }
                $restored = $true
            } catch { Add-Note ('model key restore error: ' + $_.Exception.Message) }
        }
        try { if ($ec.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) { $ec.Collapse() } } catch { }
    }
    if ($Name -eq '') { return $result }
    $now = ''
    $hash = ''
    try { $now = (Read-ModelLine).Value; $hash = Get-SettingsHash } catch { }
    if (-not $restored -or $now -ne $k0.Value -or $hash -ne $sha0) {
        Stop-Panel 6 ("MODEL KEY NOT RESTORED: the global model key was '$($k0.Value)' and is '$now'; settings.json hash was $sha0 and is $hash. " +
            'Fix ~\.claude\settings.json by hand now: set the one model line back to its old value.')
    }
    $pill = ''
    for ($i = 0; $i -lt 16; $i++) {
        Start-Sleep -Milliseconds 250
        $pill = Get-Pill
        if (Test-ModelMatch $pill $Name) { return $pill }
    }
    Stop-Panel 1 "the model picker still says '$pill', not '$Name'"
}

# ---------------------------------------------------------------- the actions

function Read-Text([string]$Path) {
    if ($Path -eq '') { Stop-Panel 1 '-TextFile is needed' }
    if (-not (Test-Path -LiteralPath $Path)) { Stop-Panel 1 "no such text file: $Path" }
    $t = [System.IO.File]::ReadAllText($Path, $Utf8)
    $t = $t.Replace("`r`n", "`n").Replace("`r", "`n").TrimEnd([char]10)
    if ($t.Trim() -eq '') { Stop-Panel 1 'the text file is empty' }
    if ($t.Length -gt $MaxChars) { Stop-Panel 1 "the text is longer than $MaxChars characters" }
    $t
}

# The last step of a send: the full read-back, then only the elements it verified are read again and that
# same Send button is invoked, with the operator idle at that moment. Returns the verified pair.
function Send-Now([string]$RowId, [string]$Text) {
    for ($try = 0; $try -lt 8; $try++) {
        Wait-Idle 'press Send'
        $ready = Test-Ready $RowId $Text
        if ($null -ne $ready.Why) { Stop-Panel 5 "read-back failed before Send ($($ready.Why))" }
        # From here to the Invoke nothing is searched: the verified elements are read again, nothing else.
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { continue }
        if (-not (Test-SameText (Get-InputValue $ready.Pair.Input) $Text)) { Stop-Panel 5 'the input changed right before Send' }
        if (-not $ready.Pair.Send.Current.IsEnabled -or $ready.Pair.Send.Current.IsOffscreen) { Stop-Panel 5 'Send was not enabled right before the press' }
        if ($null -ne $ready.Row -and -not (Test-RowActive $ready.Row)) { Stop-Panel 5 "the chat's row stopped being active right before Send" }
        $script:St.Sent = $true
        $ready.Pair.Send.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        return $ready.Pair
    }
    Stop-Panel 3 'the operator kept using the computer'
}

# After Send: the input cleared, then the transcript shows a new user line, and for a slash command its effect.
function Confirm-Sent([string]$Id, [string]$Text, $Mark, $Pair) {
    $cleared = $false
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limit = Get-StepMs 10000
    while ($sw.ElapsedMilliseconds -lt $limit) {
        $v = $null
        $en = $false
        try { $v = Get-InputValue $Pair.Input; $en = [bool]$Pair.Send.Current.IsEnabled } catch { $v = $null; $en = $false }
        if (-not $en -or ($null -ne $v -and -not (Test-SameText $v $Text))) { $cleared = $true; break }
        Start-Sleep -Milliseconds 200
    }
    if (-not $cleared) { Stop-Panel 1 'Send was invoked but the input did not clear' }
    $slash = $Text.StartsWith('/')
    $title = ''
    if ($slash -and $Text -match '^/rename\s+(.+)$') { $title = $Matches[1].Trim() }
    $isCompact = ($slash -and $Text -match '^/compact(\s|$)')
    $limit = 90
    if ($isCompact) { $limit = 600 }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limitMs = Get-StepMs ($limit * 1000)
    $userSeen = $false
    while ($sw.ElapsedMilliseconds -lt $limitMs) {
        $f = Read-TxSince $Id $Mark
        if ($f.User -ge 1) { $userSeen = $true }
        if ($isCompact) {
            if ($f.Boundary -ge 1) { return 'sent; the transcript gained a compact_boundary line' }
        } elseif ($title -ne '') {
            if ($userSeen -and ($f.Title -ceq $title -or (@(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id -and $_.Name -ceq $title }).Count -eq 1))) { return 'sent; the chat shows the new name' }
        } elseif ($userSeen) {
            return 'sent; the transcript gained the user line'
        }
        Start-Sleep -Milliseconds 700
    }
    if (-not $userSeen) { Stop-Panel 1 'Send was invoked but the transcript gained no user line in time' }
    if ($isCompact) { Stop-Panel 1 'the transcript gained no compact_boundary line in time; look at the chat before trusting it' }
    Stop-Panel 1 'the user line arrived but the chat does not show the new name (row and transcript title)'
}

# Wait until the chat's records and row say idle (a slash command ends fast), up to the given seconds.
function Wait-ChatIdle([string]$Id, [int]$Seconds) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $limit = Get-StepMs ($Seconds * 1000)
    while ($sw.ElapsedMilliseconds -lt $limit) {
        $rs = Get-RecordState $Id
        $row = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id })
        $recIdle = ($rs.Count -eq 0 -or $rs.Busy -eq '')
        $rowIdle = ($row.Count -eq 1 -and $row[0].Status -eq 'idle')
        if ($recIdle -and $rowIdle) { return $true }
        Start-Sleep -Milliseconds 700
    }
    $false
}

# The chat's row, once it is not running (within the wait budget).
function Wait-RowIdle([string]$Id) {
    $chat = Get-Chat $Id
    while ($chat.Status -eq 'running') {
        if ((Get-Date) -ge (Get-WaitUntil)) { Stop-Panel 3 'the chat is running (its row says so)' }
        Start-Sleep -Seconds 2
        $chat = Get-Chat $Id
    }
    $chat
}

function Do-List {
    Open-Window
    if ([PanelNative]::IsIconic($script:Hwnd)) {
        # Restoring the window changes his screen, so this list waits its turn like any other run.
        Enter-Lock 'list' ''
        Ensure-Screen
    } else { Wait-Tree }
    $rows = @(Get-Chats $script:Win | ForEach-Object { [pscustomobject]@{ session_id = $_.Id; name = $_.Name; status = $_.Status; active = $_.Active } })
    ConvertTo-Json -InputObject $rows -Compress
}

function Do-Send([string]$Id, [string]$Text) {
    New-RunState $Id $Text
    $s = $script:St
    $done = Invoke-Guarded {
        Test-Bridge
        Wait-Guards $Id
        Ensure-Screen
        $chat = Wait-RowIdle $Id
        $rs = Get-RecordState $Id
        if ($rs.Terminal -ne '') { Stop-Panel 5 $rs.Terminal }
        $s.HadTab = ($rs.Count -gt 0)
        $s.TabOpen = $s.HadTab
        Set-TabLabel (Get-TabLabel $chat.Name)
        if ($s.HadTab) {
            # His tab: reveal it, prove it is the one and holds no draft, close it, wait for the process to end.
            Close-Checked $Id $Id 'empty' (Get-TabLabel $chat.Name) $Id
        }
        $mark = Get-TxMark $Id
        Open-WithText $Id $Text
        [void](Wait-Ready $Id $Text 25000)
        $pair = Send-Now $Id $Text
        Confirm-Sent $Id $Text $mark $pair
    }
    # The text is sent and confirmed: from here a failure is a note, never a failed run. Only a tab the
    # driver opened in this run, for a chat whose tab was closed at the start, is closed again, and only
    # while its process is still the one the driver's open started (he may have reopened it meanwhile).
    if (-not $s.HadTab -and $s.Opened) {
        try {
            if ($Text.StartsWith('/')) {
                # A slash command ends fast; the tab that was closed before is closed again once the chat is idle.
                if ($s.OwnPid -le 0) { Add-Note "the chat's process could not be told apart, so its tab was left open" }
                elseif (Wait-ChatIdle $Id 150) {
                    Close-Checked $Id $Id 'empty' '' $Id $false $s.OwnPid
                    $done += '; its tab is closed again, as it was'
                } else { Add-Note 'the chat was still busy, so its tab was left open' }
            } else {
                Add-Note 'the chat is running the new turn, so its tab was left open'
            }
        } catch {
            $pe = Get-PanelExit $_
            $why = $_.Exception.Message
            if ($null -ne $pe) { $why = $pe.Reason }
            Add-Note ('the text was sent; its tab was left open: ' + $why)
        }
    }
    $done
}

function Do-New([string]$Text, [string]$ModelName, [string]$Group) {
    if ($ModelName -eq '') { Stop-Panel 1 '-Model is needed: a new chat would inherit the default model, which is the expensive one' }
    New-RunState '' $Text
    $s = $script:St
    $s.RowId = ''
    $s.OwnLabel = $UntitledLabel
    $s.Label = $UntitledLabel
    $newId = Invoke-Guarded {
        Test-Bridge
        Wait-Guards ''
        Ensure-Screen
        Wait-Idle 'open a new chat'
        # Nothing waits from here to the open: the new tab must be the only untitled one.
        $claude = @(Get-ClaudeTabs)
        $blank = @($claude | Where-Object { $_.label -ceq $UntitledLabel })
        if ($blank.Count -ne 0) { Stop-Panel 5 "$($blank.Count) untitled '$UntitledLabel' tab(s) are already open, so the new chat's tab could not be told apart; run close-untitled or close them, then start the chat" }
        $recBefore = @(Get-Records | ForEach-Object { [string]$_.sessionId })
        $bargs = @{ prompt = $Text }
        if ($Group -ne '') { $bargs.groupId = $Group }
        $s.OpenCalled = $true
        $r = Invoke-Bridge 'open' $bargs 25
        if ($null -ne $r -and -not $r.ok) { Stop-Panel 1 "the bridge refused 'open': $($r.error)" }
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $limit = Get-StepMs 10000
        while ($sw.ElapsedMilliseconds -lt $limit) {
            $now = @(Get-ClaudeTabs)
            if ($now.Count -eq $claude.Count + 1 -and @($now | Where-Object { $_.label -ceq $UntitledLabel }).Count -eq 1) { $s.Opened = $true; $s.Prefilled = $true; $s.TabOpen = $true; break }
            Start-Sleep -Milliseconds 300
        }
        if (-not $s.Opened) {
            if ($null -eq $r) { Stop-Panel 2 'the bridge did not answer open within 25 seconds and no untitled tab was added' }
            Stop-Panel 5 'the open added no untitled tab'
        }
        if ($null -eq $r) { Add-Note 'the bridge did not answer open in time, but the tab list shows the new tab' }
        [void](Wait-Ready '' $Text 30000)
        # The new chat is the one panel session record that appeared since the open (its process starts at once).
        $new = @()
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $limit = Get-StepMs 10000
        while ($sw.ElapsedMilliseconds -lt $limit) {
            $new = @(Get-Records | Where-Object { $recBefore -notcontains [string]$_.sessionId -and [string]$_.entrypoint -eq $PanelEntrypoint })
            if ($new.Count -ge 1) { break }
            Start-Sleep -Milliseconds 250
        }
        if ($new.Count -ne 1) { Stop-Panel 5 "$($new.Count) new panel session records appeared after the open, so the new chat cannot be told apart" }
        $s.Id = [string]$new[0].sessionId
        $n = 0
        if ([int]::TryParse([string]$new[0].pid, [ref]$n)) { $s.OwnPid = $n }
        # The model is set and shown before the first message exists.
        [void](Select-Model $ModelName)
        [void](Wait-Ready '' $Text 15000)
        $nowPill = Get-Pill
        if (-not (Test-ModelMatch $nowPill $ModelName)) { Stop-Panel 5 "the picker shows '$nowPill', not '$ModelName'" }
        $pair = Send-Now '' $Text
        # The row that appears must be the new chat's own record, not any new row.
        $seen = $false
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $limit = Get-StepMs 60000
        while ($sw.ElapsedMilliseconds -lt $limit) {
            if (@(Get-Chats $script:Win | Where-Object { $_.Id -eq $s.Id }).Count -eq 1) { $seen = $true; break }
            Start-Sleep -Milliseconds 500
        }
        if (-not $seen) { Stop-Panel 1 "the message was sent but no row for the new chat's session $($s.Id) appeared within 60 seconds" }
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $limit = Get-StepMs 90000
        $seen = $false
        while ($sw.ElapsedMilliseconds -lt $limit) {
            $f = Read-TxSince $s.Id ([pscustomobject]@{ Len = [long]0 })
            if ($f.User -ge 1) { $seen = $true; break }
            Start-Sleep -Milliseconds 700
        }
        if (-not $seen) { Stop-Panel 1 "the new chat $($s.Id) exists but its transcript has no user line after 90 seconds" }
        Add-Note "model shown: $nowPill; its tab is left open (it is running its first turn)"
        $s.Id
    }
    $newId
}

function Do-Model([string]$Id, [string]$ModelName) {
    New-RunState $Id ''
    $s = $script:St
    $out = Invoke-Guarded {
        Test-Bridge
        Wait-Guards $Id
        Ensure-Screen
        $chat = Wait-RowIdle $Id
        $rs = Get-RecordState $Id
        $s.HadTab = ($rs.Count -gt 0)
        $s.TabOpen = $s.HadTab
        Set-TabLabel (Get-TabLabel $chat.Name)
        Wait-Idle 'show the chat'
        $n0 = @(Get-ClaudeTabs).Count
        if (-not $s.HadTab) { $s.OpenCalled = $true }
        Show-Chat $Id
        if (-not $s.HadTab -and @(Get-ClaudeTabs).Count -eq $n0 + 1) { $s.Opened = $true; $s.TabOpen = $true }
        $act = @(Get-ActiveIds)
        if (-not ($act.Count -eq 1 -and $act[0] -eq $Id)) { Stop-Panel 5 'the target row is not the only active row after reveal; no model was picked' }
        $rs = Get-RecordState $Id
        if ($rs.Terminal -ne '') { Stop-Panel 5 $rs.Terminal }
        if ($rs.Busy -ne '') { Stop-Panel 3 "the chat is busy: $($rs.Busy)" }
        if ((Get-Chat $Id).Status -eq 'running') { Stop-Panel 3 'the chat started running; its model was not changed' }
        Select-Model $ModelName
    }
    if ($s.Opened) { Add-Note 'the chat had no open tab; its tab is left open because the pick lives in its process' }
    $out
}

function Do-CloseUntitled {
    Test-Bridge
    $blank = @(Get-ClaudeTabs | Where-Object { $_.label -ceq $UntitledLabel })
    if ($blank.Count -eq 0) { return 'closed 0: no untitled Claude Code tab is open' }
    New-RunState '' ''
    Wait-Guards ''
    Ensure-Screen
    $closed = 0
    $skipped = ''
    for ($n = 0; $n -lt 20; $n++) {
        $tabs = @(Get-ClaudeTabs | Where-Object { $_.label -ceq $UntitledLabel })
        if ($tabs.Count -eq 0) { break }
        $block = Get-UntitledBlock
        if ($block -ne '') { $skipped = $block; break }
        $before = @(Get-Records).Count
        Wait-Idle 'select an untitled tab'
        if (-not (Select-UntitledTab)) { $skipped = 'no untitled tab could be selected'; break }
        try {
            # The selected tab is the first untitled one, which is the one close-tab closes.
            Close-Checked '' '*' 'empty' $UntitledLabel '' $true
        } catch {
            $pe = Get-PanelExit $_
            if ($null -eq $pe) { throw }
            $skipped = $pe.Reason
            break
        }
        $gone = $false
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $limit = Get-StepMs 60000
        while ($sw.ElapsedMilliseconds -lt $limit) {
            if (@(Get-Records).Count -lt $before) { $gone = $true; break }
            Start-Sleep -Milliseconds 250
        }
        if (-not $gone) { Stop-Panel 1 "closed $($closed + 1) untitled tab(s), but the last one's empty chat process did not end; stopping" }
        $closed++
    }
    $msg = "closed $closed untitled tab(s)"
    if ($skipped -ne '') { $msg += "; stopped: $skipped" }
    $msg
}

# ---------------------------------------------------------------- one logged run

function Write-PanelLog([string]$ActionName, [string]$Id, [int]$Code, [string]$Reason) {
    try {
        if (-not (Test-Path -LiteralPath $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }
        $why = ''
        if ($Code -ne 0) { $why = ' ' + ($Reason -replace '[\r\n]+', ' ') }
        if ($script:Notes.Count -gt 0) { $why += ' [' + (($script:Notes -join '; ') -replace '[\r\n]+', ' ') + ']' }
        $line = '{0} {1} {2} exit={3} secs={4:N1}{5}' -f (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss'), $ActionName, $(if ($Id -ne '') { $Id } else { '-' }), $Code, $script:Clock.Elapsed.TotalSeconds, $why
        [System.IO.File]::AppendAllText($LogFile, $line + "`r`n", $Utf8)
    } catch { }
}

# One run: the lock (for a run that changes anything), the body, then his view and the window put back
# and the lock let go whatever happened, then one log line.
function Invoke-Logged([string]$ActionName, [string]$LogId, [bool]$Locked, [scriptblock]$Body) {
    $script:Notes = @()
    $script:Clock = [Diagnostics.Stopwatch]::StartNew()
    Set-Deadlines $ActionName
    $r = [pscustomobject]@{ Code = 0; Reason = 'ok'; Value = $null }
    try {
        try {
            if ($Locked) { Enter-Lock $ActionName $LogId }
            $r.Value = & $Body
        } catch {
            $pe = Get-PanelExit $_
            if ($null -ne $pe) {
                $r.Code = $pe.Code
                $r.Reason = $pe.Reason
            } else {
                $r.Code = 1
                $r.Reason = 'unexpected: ' + $_.Exception.Message + ' (line ' + $_.InvocationInfo.ScriptLineNumber + ')'
            }
        }
    } finally {
        Start-Restoring
        try { Restore-Screen } catch { Add-Note ('restore failed: ' + $_.Exception.Message) }
        $script:Restoring = $false
        Exit-Lock
    }
    # The selftest returns its own exit code as its value; the log line carries that code.
    $logCode = $r.Code
    $logReason = $r.Reason
    if ($logCode -eq 0 -and $ActionName -eq 'selftest' -and $null -ne $r.Value -and [int]$r.Value -ne 0) {
        $logCode = [int]$r.Value
        $logReason = 'a selftest step failed'
    }
    Write-PanelLog $ActionName $LogId $logCode $logReason
    $r
}

# The value on stdout on success, else the reason on stderr; the notes on stderr always.
function Out-Result($r) {
    if ($r.Code -eq 0) {
        if ($null -ne $r.Value) { [Console]::Out.WriteLine([string]$r.Value) }
    } else {
        [Console]::Error.WriteLine($r.Reason)
    }
    foreach ($n in $script:Notes) { [Console]::Error.WriteLine('note: ' + $n) }
    $r.Code
}

# ---------------------------------------------------------------- selftest

$SelfTestModel = 'Haiku 5.5'
$SelfTestSecondModel = 'Haiku 4.5'

function Write-StepLine([string]$Step, [bool]$Ok, [string]$Detail) {
    $verdict = 'FAIL'
    if ($Ok) { $verdict = 'PASS' }
    [Console]::Out.WriteLine(('{0} {1}: {2}' -f $verdict, $Step, $Detail))
}

# Runs one selftest step inside the one logged run: returns the value, or $null after printing the failure.
function Step-Run([string]$Step, [scriptblock]$Body, [string]$Detail) {
    try {
        $v = & $Body
        $d = $Detail
        if ($d -eq '') { $d = [string]$v }
        Write-StepLine $Step $true $d
        return ,$v
    } catch {
        $pe = Get-PanelExit $_
        if ($null -ne $pe) { Write-StepLine $Step $false ('exit {0}: {1}' -f $pe.Code, $pe.Reason) }
        else { Write-StepLine $Step $false ('unexpected: ' + $_.Exception.Message) }
    }
    $null
}

# Closes a chat's tab after the unbroken check (selftest's last step).
function Close-ChatTab([string]$Id) {
    New-RunState $Id ''
    $script:St.HadTab = $true
    $script:St.TabOpen = $true
    Close-Checked $Id $Id 'empty' '' $Id
    'tab closed, its process ended'
}

function Do-SelfTest {
    $failed = 0
    $name = 'zz-selftest-' + (Get-Date).ToString('HHmm')
    $first = "Reply with the single word OK.`nNothing else."
    $id = ''
    $r = Step-Run 'new (Haiku 5.5, two-line first prompt)' { Do-New $first $SelfTestModel '' } ''
    if ($null -eq $r) { return 1 }
    $script:Notes = @()   # the new chat's own notes (its tab is open) do not apply: this run closes it
    $id = [string]($r | Select-Object -First 1)
    [Console]::Out.WriteLine("session id: $id")
    # let the first reply finish; a send waits for an idle chat anyway
    $null = Wait-ChatIdle $id 120
    $r = Step-Run "send /rename $name" { Do-Send $id "/rename $name" } ''
    if ($null -eq $r) { $failed++ }
    $r = Step-Run 'send /compact' { Do-Send $id '/compact Keep only the word OK.' } ''
    if ($null -eq $r) { $failed++ }
    $r = Step-Run "model set to $SelfTestSecondModel and read back" { Do-Model $id $SelfTestSecondModel } ''
    if ($null -eq $r) { $failed++ }
    $r = Step-Run 'close the chat tab' { Close-ChatTab $id } ''
    if ($null -eq $r) { $failed++ }
    [Console]::Out.WriteLine("scratch chat: $id ($name)")
    if ($failed -gt 0) { return 1 }
    0
}

# ---------------------------------------------------------------- main

if ($Session -ne '' -and $Session -notmatch '^[0-9A-Fa-f-]{8,64}$') {
    [Console]::Error.WriteLine('-Session is a session id (hex digits and dashes)')
    exit 1
}
if ($GroupId -ne '' -and $GroupId -notmatch '^[0-9A-Za-z_-]{1,200}$') {
    [Console]::Error.WriteLine('-GroupId is a group id (letters, digits, dashes)')
    exit 1
}
$code = 1
switch ($Action) {
    'list' { $code = Out-Result (Invoke-Logged 'list' '' $false { Do-List }) }
    'send' {
        if ($Session -eq '') { [Console]::Error.WriteLine('send needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'send' $Session $true { Do-Send $Session (Read-Text $TextFile) })
        }
    }
    'new' { $code = Out-Result (Invoke-Logged 'new' '' $true { Do-New (Read-Text $TextFile) $Model $GroupId }) }
    'model' {
        if ($Session -eq '') { [Console]::Error.WriteLine('model needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'model' $Session $true { Do-Model $Session $Model })
        }
    }
    'close-untitled' { $code = Out-Result (Invoke-Logged 'close-untitled' '' $true { Do-CloseUntitled }) }
    'selftest' {
        $r = Invoke-Logged 'selftest' '' $true { Do-SelfTest }
        $code = [int]$r.Value
        if ($r.Code -ne 0) { [Console]::Error.WriteLine($r.Reason); $code = $r.Code }
        foreach ($n in $script:Notes) { [Console]::Error.WriteLine('note: ' + $n) }
    }
    default { [Console]::Error.WriteLine('usage: claude_panel.ps1 list | send | new | model | close-untitled | selftest'); $code = 1 }
}
exit $code
