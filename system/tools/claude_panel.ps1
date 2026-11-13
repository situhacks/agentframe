# The Claude panel driver, version 2. It manages chats in the Claude Code panel of the one VS Code window
# whose title holds the vault folder name, and nothing else. It sends no key events and no mouse events:
# it reads and acts through Windows UI Automation patterns (read, Invoke, ExpandCollapse, SelectionItem,
# Window) and through the AgentFrame bridge extension (system/vscode-bridge), which opens, reveals and
# closes chat tabs and prefills a new chat's input from a file inbox. Text always arrives in a UTF-8
# file, never as an argument, and is never logged.
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
# input (the page does it; the bridge never presses Send). So: reveal the chat, prove its tab is the one
# (label and active tab), prove the input holds no draft, close the tab, open it with the text, read the
# input back through UI Automation, and only then invoke 'Send message'. Text that starts with a slash is
# sent as it is. A chat that has no open tab skips the close.
#
# Guards:
#   1. the operator has been idle for -IdleSeconds before any step that changes his screen, and again
#      immediately before 'Send message'; otherwise wait up to -WaitMinutes, then exit 3
#   2. the target chat is idle by its row and by its session record; the tab of a busy chat is never closed
#   3. a draft in the target's input is exit 4 (read only after the window is visible and the row is active)
#   4. before Send: the input equals the text, the target row is the only active row, Send is enabled;
#      otherwise nothing is sent (exit 5)
#   5. tab identity: exactly one Claude tab carries the label and it is the active tab, else exit 5
# The window may be minimized: the accessibility tree is stale then, so the driver restores it with the
# Window pattern first and minimizes it again at the end. Every model pick saves the global model key and
# the settings file hash first, writes the key back right after the pick, and verifies (exit 6 if the
# file does not match).
# Exit codes: 0 done and confirmed; 1 failed or could not confirm; 2 bridge, window, chat or control not
# found; 3 operator active or chat busy; 4 draft in the input; 5 a safety check failed; 6 the model key or
# settings file does not match after a pick.
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
$RowPrefix = 'sessions-list-row-'
$MaxChars = 200000
$Utf8 = New-Object System.Text.UTF8Encoding($false)

$script:Win = $null
$script:Hwnd = [IntPtr]::Zero
$script:Deadline = (Get-Date).AddMinutes($WaitMinutes)
$script:ScreenBegun = $false
$script:WasMinimized = $false
$script:PrevActive = @()
$script:OpenedUnsent = $false
$script:Notes = @()
$script:TxCache = @{}
$script:Clock = [Diagnostics.Stopwatch]::StartNew()

function Stop-Panel([int]$Code, [string]$Reason) {
    throw (New-Object PanelExit($Code, $Reason))
}

function Add-Note([string]$Note) { $script:Notes += $Note }

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

# VS Code cuts a tab label at 25 characters with an ellipsis.
function Get-TabLabel([string]$Title) {
    if ($Title.Length -gt 25) { return ($Title.Substring(0, 24) + [string][char]0x2026) }
    $Title
}

# ---------------------------------------------------------------- the idle guard

function Wait-Idle([string]$What) {
    while ($true) {
        if ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000)) { return }
        if ((Get-Date) -ge $script:Deadline) { Stop-Panel 3 "the operator kept using the computer for $WaitMinutes minutes; nothing was changed ($What)" }
        Start-Sleep -Seconds 2
    }
}

function Get-ConfigDir {
    if ($env:CLAUDE_CONFIG_DIR) { return $env:CLAUDE_CONFIG_DIR }
    Join-Path $env:USERPROFILE '.claude'
}

# The session records under the config dir's sessions\ (only the .json files).
function Get-Records {
    $dir = Join-Path (Get-ConfigDir) 'sessions'
    $out = @()
    if (-not (Test-Path -LiteralPath $dir)) { return $out }
    foreach ($f in Get-ChildItem -LiteralPath $dir -Filter '*.json' -ErrorAction SilentlyContinue) {
        try { $out += ([System.IO.File]::ReadAllText($f.FullName, $Utf8) | ConvertFrom-Json) } catch { }
    }
    $out
}

function Get-Record([string]$Id) {
    foreach ($r in @(Get-Records)) { if ($r.sessionId -eq $Id) { return $r } }
    $null
}

# '' when the chat is idle by its record, else why not. No record means no process.
function Get-RecordBusy([string]$Id) {
    $rec = Get-Record $Id
    if ($null -eq $rec) { return '' }
    $st = [string]$rec.status
    if ($st -ne '' -and $st -ne 'idle') { return "its session record says $st" }
    ''
}

# Guards 1 and 2 before the screen is touched: operator idle and the record idle, up to -WaitMinutes.
function Wait-Guards([string]$Id) {
    while ($true) {
        $why = ''
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { $why = 'the operator is using the computer' }
        elseif ($Id -ne '') { $why = Get-RecordBusy $Id }
        if ($why -eq '') { return }
        if ((Get-Date) -ge $script:Deadline) { Stop-Panel 3 "gave up after $WaitMinutes minutes: $why; nothing was changed" }
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
# a little and then says it did not restore.
function Restore-Screen {
    if (-not $script:ScreenBegun) { return }
    $script:ScreenBegun = $false
    try {
        $limit = (Get-Date).AddSeconds(60)
        while ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000) -and (Get-Date) -lt $limit) { Start-Sleep -Seconds 2 }
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { Add-Note 'not restored: the operator is at the machine'; return }
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

function Get-InputBoxes { @(Find-Named $script:Win 'Message input' $CT::Edit | Where-Object { -not $_.Current.IsOffscreen }) }
function Get-SendButtons { @(Find-Named $script:Win 'Send message' $CT::Button | Where-Object { -not $_.Current.IsOffscreen }) }

function Read-InputValue {
    $boxes = Get-InputBoxes
    if ($boxes.Count -ne 1) { return $null }
    $o = $null
    if (-not $boxes[0].TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$o)) { return $null }
    [string]$o.Current.Value
}

function Get-SendState {
    $s = Get-SendButtons
    if ($s.Count -ne 1) { return 'none' }
    if ($s[0].Current.IsEnabled) { 'enabled' } else { 'disabled' }
}

function ConvertTo-PlainValue([string]$Value) {
    if ($null -eq $Value) { return '' }
    $v = $Value.Replace([string][char]0xA0, ' ').Replace([string][char]0x200B, '').Replace([string][char]0xFEFF, '')
    $v.Replace("`r`n", "`n").Replace("`r", "`n").TrimEnd([char]10)
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

# $null when the panel shows what a send needs, else what is wrong. $Id '' means a new chat (no row active).
function Test-Ready([string]$Id, [string]$Text) {
    $act = @(Get-ActiveIds)
    if ($Id -ne '') {
        if (-not ($act.Count -eq 1 -and $act[0] -eq $Id)) { return "the target row is not the only active row ($($act.Count) active)" }
    } elseif ($act.Count -ne 0) { return "a chat row is active ($($act.Count)), so this is not the new chat's tab" }
    $v = Read-InputValue
    if ($null -eq $v) { return 'the message input is not on screen' }
    if (-not (Test-SameText $v $Text)) { return 'the input does not hold the text' }
    if ((Get-SendState) -ne 'enabled') { return 'Send is not enabled' }
    $null
}

function Wait-Ready([string]$Id, [string]$Text, [int]$Ms) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $why = 'not read'
    while ($sw.ElapsedMilliseconds -lt $Ms) {
        $why = Test-Ready $Id $Text
        if ($null -eq $why) { return }
        Start-Sleep -Milliseconds 250
    }
    Stop-Panel 5 "read-back failed ($why); nothing was sent"
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

# Reveal the chat's tab, prove through tabs and the panel that the label is unique and the tab is the
# active one and the target row is active, then close it and wait for the chat's process to end.
function Close-ChatTab([string]$Id, [string]$Title) {
    $label = Get-TabLabel $Title
    Wait-Idle 'reveal the chat before closing its tab'
    [void](Invoke-BridgeOk 'reveal' @{ sessionId = $Id })
    $ok = $false
    for ($i = 0; $i -lt 32; $i++) {
        $act = @(Get-ActiveIds)
        if ($act.Count -eq 1 -and $act[0] -eq $Id) { $ok = $true; break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $ok) { Stop-Panel 5 'after reveal the target row is not the only active row; its tab was not closed' }
    $same = @(Get-BridgeTabs | Where-Object { $_.claude -and $_.label -ceq $label })
    if ($same.Count -ne 1) { Stop-Panel 5 "$($same.Count) Claude tabs carry the label '$label' (labels are cut at 25 characters); no tab was closed" }
    if (-not $same[0].isActive) { Stop-Panel 5 "the tab labelled '$label' is not the active tab; no tab was closed" }
    $busy = Get-RecordBusy $Id
    if ($busy -ne '') { Stop-Panel 3 "refusing to close the tab of a busy chat: $busy" }
    Wait-Idle 'close the chat tab'
    $r = Invoke-BridgeOk 'close-tab' @{ label = $label; all = $false }
    if ($r.closed -ne 1) { Stop-Panel 5 "close-tab closed $($r.closed) tabs, not 1" }
    $gone = $false
    for ($i = 0; $i -lt 360; $i++) {
        if ($null -eq (Get-Record $Id)) { $gone = $true; break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $gone) { Stop-Panel 1 'the tab was closed but the chat process did not end within 90 seconds' }
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

function Test-ModelMatch([string]$Option, [string]$Name) {
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
        if ((Test-ModelMatch $pill $Name) -or $pill.StartsWith($Name, [StringComparison]::OrdinalIgnoreCase)) { return $pill }
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

# The last step of a send: readback, then Send, with the operator idle at that moment.
function Send-Now([string]$Id, [string]$Text) {
    for ($try = 0; $try -lt 8; $try++) {
        Wait-Idle 'press Send'
        $why = Test-Ready $Id $Text
        if ($null -ne $why) { Stop-Panel 5 "read-back failed before Send ($why); nothing was sent" }
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { continue }
        $btn = Get-SendButtons
        $btn[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        return
    }
    Stop-Panel 3 'the operator kept using the computer; nothing was sent'
}

# After Send: the input cleared, then the transcript shows a new user line, and for a slash command its effect.
function Confirm-Sent([string]$Id, [string]$Text, $Mark) {
    $cleared = $false
    for ($i = 0; $i -lt 50; $i++) {
        $v = Read-InputValue
        if ((Get-SendState) -ne 'enabled' -or ($null -ne $v -and -not (Test-SameText $v $Text))) { $cleared = $true; break }
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
    $userSeen = $false
    while ($sw.Elapsed.TotalSeconds -lt $limit) {
        $f = Read-TxSince $Id $Mark
        if ($f.User -ge 1) { $userSeen = $true }
        if ($isCompact) {
            if ($f.Boundary -ge 1) { return 'sent; the transcript gained a compact_boundary line' }
        } elseif ($title -ne '') {
            if ($userSeen -and ($f.Title -ceq $title -or (@(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id -and $_.Name -ceq $title }).Count -eq 1))) { return "sent; the chat is renamed to '$title'" }
        } elseif ($userSeen) {
            return 'sent; the transcript gained the user line'
        }
        Start-Sleep -Milliseconds 700
    }
    if (-not $userSeen) { Stop-Panel 1 'Send was invoked but the transcript gained no user line within 90 seconds' }
    if ($isCompact) { Stop-Panel 1 'the transcript gained no compact_boundary line within 10 minutes; look at the chat before trusting it' }
    Stop-Panel 1 "the user line arrived but the chat is not renamed to '$title' (row and transcript title)"
}

# Wait until the chat's record and row say idle (a slash command ends fast), up to the given seconds.
function Wait-ChatIdle([string]$Id, [int]$Seconds) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.Elapsed.TotalSeconds -lt $Seconds) {
        $rec = Get-Record $Id
        $row = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id })
        $recIdle = ($null -eq $rec -or [string]$rec.status -eq 'idle' -or [string]$rec.status -eq '')
        $rowIdle = ($row.Count -eq 1 -and $row[0].Status -ne 'running')
        if ($recIdle -and $rowIdle) { return $true }
        Start-Sleep -Milliseconds 700
    }
    $false
}

function Do-List {
    Open-Window
    $min = [PanelNative]::IsIconic($script:Hwnd)
    if ($min) { Ensure-Screen } else { Wait-Tree; $script:ScreenBegun = $false }
    $rows = @(Get-Chats $script:Win | ForEach-Object { [pscustomobject]@{ session_id = $_.Id; name = $_.Name; status = $_.Status; active = $_.Active } })
    ConvertTo-Json -InputObject $rows -Compress
}

function Do-Send([string]$Id, [string]$Text) {
    Test-Bridge
    Wait-Guards $Id
    Ensure-Screen
    $chat = Get-Chat $Id
    if ($chat.Status -eq 'running') {
        $limit = $script:Deadline
        while ($chat.Status -eq 'running') {
            if ((Get-Date) -ge $limit) { Stop-Panel 3 'the chat is running (its row says so); nothing was changed' }
            Start-Sleep -Seconds 2
            $chat = Get-Chat $Id
        }
    }
    $rec = Get-Record $Id
    $hadTab = ($null -ne $rec)
    if ($hadTab) {
        $busy = Get-RecordBusy $Id
        if ($busy -ne '') { Stop-Panel 3 "refusing to close the tab of a busy chat: $busy" }
        # Reveal it and read its input: closing the tab would throw a draft away.
        Wait-Idle 'reveal the chat to read its input'
        [void](Invoke-BridgeOk 'reveal' @{ sessionId = $Id })
        $ok = $false
        for ($i = 0; $i -lt 32; $i++) {
            $act = @(Get-ActiveIds)
            if ($act.Count -eq 1 -and $act[0] -eq $Id) { $ok = $true; break }
            Start-Sleep -Milliseconds 250
        }
        if (-not $ok) { Stop-Panel 5 'after reveal the target row is not the only active row; nothing was changed further' }
        Start-Sleep -Milliseconds 700
        if ((Get-SendState) -ne 'disabled') { Stop-Panel 4 "the chat has a draft typed in its input box (Send is enabled): $($chat.Name)" }
        Close-ChatTab $Id $chat.Name
    }
    $mark = Get-TxMark $Id
    Wait-Idle 'open the chat with the text'
    [void](Invoke-BridgeOk 'open' @{ sessionId = $Id; prompt = $Text })
    $script:OpenedUnsent = $true
    Wait-Ready $Id $Text 25000
    Send-Now $Id $Text
    $script:OpenedUnsent = $false
    $done = Confirm-Sent $Id $Text $mark
    if (-not $hadTab) {
        if ($Text.StartsWith('/')) {
            # A slash command ends fast; the tab that was closed before is closed again once the chat is idle.
            if (Wait-ChatIdle $Id 150) {
                $row = Get-Chat $Id
                Close-ChatTab $Id $row.Name
                $done += '; its tab is closed again'
            } else { Add-Note 'the chat was still busy, so its tab was left open' }
        } else {
            Add-Note 'the chat is running the new turn, so its tab was left open'
        }
    }
    $done
}

function Do-New([string]$Text, [string]$ModelName, [string]$Group) {
    if ($ModelName -eq '') { Stop-Panel 1 '-Model is needed: a new chat would inherit the default model, which is the expensive one' }
    Test-Bridge
    Wait-Guards ''
    Ensure-Screen
    $rowsBefore = @(Get-Chats $script:Win | ForEach-Object { $_.Id })
    $recBefore = @(Get-Records | ForEach-Object { $_.sessionId })
    Wait-Idle 'open a new chat'
    $bargs = @{ prompt = $Text }
    if ($Group -ne '') { $bargs.groupId = $Group }
    [void](Invoke-BridgeOk 'open' $bargs)
    $script:OpenedUnsent = $true
    Wait-Ready '' $Text 30000
    # The new chat's process and record exist at once.
    $newRec = @()
    for ($i = 0; $i -lt 40; $i++) {
        $newRec = @(Get-Records | Where-Object { $recBefore -notcontains $_.sessionId })
        if ($newRec.Count -ge 1) { break }
        Start-Sleep -Milliseconds 250
    }
    # The model is set and shown before the first message exists.
    $pill = Select-Model $ModelName
    Wait-Ready '' $Text 15000
    $nowPill = Get-Pill
    if (-not ((Test-ModelMatch $nowPill $ModelName) -or $nowPill.StartsWith($ModelName, [StringComparison]::OrdinalIgnoreCase))) {
        Stop-Panel 5 "the picker shows '$nowPill', not '$ModelName'; nothing was sent"
    }
    Send-Now '' $Text
    $script:OpenedUnsent = $false
    $newId = ''
    for ($i = 0; $i -lt 120; $i++) {
        $now = @(Get-Chats $script:Win | Where-Object { $rowsBefore -notcontains $_.Id })
        if ($now.Count -ge 1) { $newId = $now[0].Id; break }
        Start-Sleep -Milliseconds 500
    }
    if ($newId -eq '') { Stop-Panel 1 'the message was sent but no new chat row appeared within 60 seconds' }
    if ($newRec.Count -eq 1 -and $newRec[0].sessionId -ne $newId) { Add-Note "the new record named $($newRec[0].sessionId) but the new row is $newId" }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $seen = $false
    while ($sw.Elapsed.TotalSeconds -lt 90) {
        $f = Read-TxSince $newId ([pscustomobject]@{ Len = [long]0 })
        if ($f.User -ge 1) { $seen = $true; break }
        Start-Sleep -Milliseconds 700
    }
    if (-not $seen) { Stop-Panel 1 "the new chat $newId exists but its transcript has no user line after 90 seconds" }
    Add-Note "model shown: $nowPill; its tab is left open (it is running its first turn)"
    $newId
}

function Do-Model([string]$Id, [string]$ModelName) {
    Test-Bridge
    Wait-Guards $Id
    Ensure-Screen
    $chat = Get-Chat $Id
    if ($chat.Status -eq 'running') { Stop-Panel 3 'the chat is running; its model was not changed' }
    Wait-Idle 'show the chat'
    $rec = Get-Record $Id
    [void](Invoke-BridgeOk 'reveal' @{ sessionId = $Id })
    $ok = $false
    for ($i = 0; $i -lt 40; $i++) {
        $act = @(Get-ActiveIds)
        if ($act.Count -eq 1 -and $act[0] -eq $Id) { $ok = $true; break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $ok) { Stop-Panel 5 'the target row is not the only active row after reveal; no model was picked' }
    Start-Sleep -Milliseconds 800
    if ($null -eq $rec) { Add-Note 'the chat had no open tab; its tab is left open because the pick lives in its process' }
    $busy = Get-RecordBusy $Id
    if ($busy -ne '') { Stop-Panel 3 "the chat is busy: $busy" }
    $chat = Get-Chat $Id
    if ($chat.Status -eq 'running') { Stop-Panel 3 'the chat started running; its model was not changed' }
    Select-Model $ModelName
}

function Do-CloseUntitled {
    Test-Bridge
    $blank = @(Get-BridgeTabs | Where-Object { $_.claude -and $_.label -ceq 'Claude Code' })
    if ($blank.Count -eq 0) { return 'closed 0: no untitled Claude Code tab is open' }
    Wait-Guards ''
    Ensure-Screen
    $closed = 0
    $skipped = ''
    for ($n = 0; $n -lt 20; $n++) {
        $tabs = @(Get-BridgeTabs | Where-Object { $_.claude -and $_.label -ceq 'Claude Code' })
        if ($tabs.Count -eq 0) { break }
        # Candidate records: a process with no transcript and no row (a chat that has not had a message).
        $rows = @(Get-Chats $script:Win | ForEach-Object { $_.Id })
        $cand = @(Get-Records | Where-Object { $rows -notcontains $_.sessionId -and (Find-Transcript $_.sessionId) -eq '' })
        if ($cand.Count -lt 1) { $skipped = 'no live chat without a transcript and a row is left'; break }
        Wait-Idle 'select an untitled tab'
        if (-not (Select-UntitledTab)) { $skipped = 'no untitled tab could be selected'; break }
        $first = @(Get-BridgeTabs | Where-Object { $_.claude -and $_.label -ceq 'Claude Code' })[0]
        $act = @(Get-ActiveIds)
        if (-not $first.isActive -or $act.Count -ne 0) { $skipped = 'the first untitled tab is not the selected one, or a chat row is active'; break }
        if ((Get-SendState) -ne 'disabled') { $skipped = 'an untitled tab holds an unsent draft; it was left open'; break }
        Wait-Idle 'close an untitled tab'
        $r = Invoke-BridgeOk 'close-tab' @{ label = 'Claude Code'; all = $false }
        if ($r.closed -ne 1) { Stop-Panel 1 "close-tab closed $($r.closed) tabs, not 1" }
        $gone = $false
        for ($i = 0; $i -lt 240; $i++) {
            $left = @(Get-Records | Where-Object { $rows -notcontains $_.sessionId -and (Find-Transcript $_.sessionId) -eq '' })
            if ($left.Count -lt $cand.Count) { $gone = $true; break }
            Start-Sleep -Milliseconds 250
        }
        if (-not $gone) { Stop-Panel 1 'an untitled tab was closed but no empty chat process ended; stopping' }
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

# Close a tab this run opened with text in it but did not send, so no prefilled draft is left behind.
function Clean-UnsentTab([string]$Id) {
    if (-not $script:OpenedUnsent) { return }
    $script:OpenedUnsent = $false
    try {
        if ($Id -ne '' -and [PanelNative]::IdleMs() -ge ($IdleSeconds * 1000)) {
            $row = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id })
            if ($row.Count -eq 1) { Close-ChatTab $Id $row[0].Name; Add-Note 'the tab opened with the unsent text was closed again' }
        } else {
            Add-Note 'an empty chat tab with unsent text was left open; close it by hand or run close-untitled'
        }
    } catch { Add-Note ('could not close the unsent tab: ' + $_.Exception.Message) }
}

function Invoke-Logged([string]$ActionName, [string]$LogId, [scriptblock]$Body) {
    $script:Notes = @()
    $script:Clock = [Diagnostics.Stopwatch]::StartNew()
    $script:Deadline = (Get-Date).AddMinutes($WaitMinutes)
    $r = [pscustomobject]@{ Code = 0; Reason = 'ok'; Value = $null }
    try {
        $r.Value = & $Body
    } catch [PanelExit] {
        $r.Code = $_.Exception.Code
        $r.Reason = $_.Exception.Reason
    } catch {
        $r.Code = 1
        $r.Reason = 'unexpected: ' + $_.Exception.Message + ' (line ' + $_.InvocationInfo.ScriptLineNumber + ')'
    }
    if ($r.Code -ne 0 -and $script:ScreenBegun) { Clean-UnsentTab $LogId }
    try { Restore-Screen } catch { Add-Note ('restore failed: ' + $_.Exception.Message) }
    Write-PanelLog $ActionName $LogId $r.Code $r.Reason
    $r
}

function Out-Result($r) {
    if ($r.Code -eq 0) {
        if ($null -ne $r.Value) { [Console]::Out.WriteLine([string]$r.Value) }
        foreach ($n in $script:Notes) { [Console]::Error.WriteLine('note: ' + $n) }
    } else {
        [Console]::Error.WriteLine($r.Reason)
    }
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
    } catch [PanelExit] {
        Write-StepLine $Step $false ('exit {0}: {1}' -f $_.Exception.Code, $_.Exception.Reason)
    } catch {
        Write-StepLine $Step $false ('unexpected: ' + $_.Exception.Message)
    }
    $null
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
    $r = Step-Run 'close the chat tab' {
        $row = Get-Chat $id
        Close-ChatTab $id $row.Name
        'tab closed, its process ended'
    } ''
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
    'list' { $code = Out-Result (Invoke-Logged 'list' '' { Do-List }) }
    'send' {
        if ($Session -eq '') { [Console]::Error.WriteLine('send needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'send' $Session { Do-Send $Session (Read-Text $TextFile) })
        }
    }
    'new' { $code = Out-Result (Invoke-Logged 'new' '' { Do-New (Read-Text $TextFile) $Model $GroupId }) }
    'model' {
        if ($Session -eq '') { [Console]::Error.WriteLine('model needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'model' $Session { Do-Model $Session $Model })
        }
    }
    'close-untitled' { $code = Out-Result (Invoke-Logged 'close-untitled' '' { Do-CloseUntitled }) }
    'selftest' {
        $r = Invoke-Logged 'selftest' '' { Do-SelfTest }
        $code = [int]$r.Value
        if ($r.Code -ne 0) { [Console]::Error.WriteLine($r.Reason); $code = $r.Code }
        foreach ($n in $script:Notes) { [Console]::Error.WriteLine('note: ' + $n) }
    }
    default { [Console]::Error.WriteLine('usage: claude_panel.ps1 list | send | new | model | close-untitled | selftest'); $code = 1 }
}
exit $code
