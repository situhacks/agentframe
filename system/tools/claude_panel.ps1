# The Claude panel driver. It drives the Claude Code panel of the one VS Code window whose title holds
# the vault folder name, and nothing else: no other window, no pointer input, only the controls named
# below. It is not the general desktop tool. Text always arrives in a UTF-8 file, never as an argument
# (Windows PowerShell 5.1 mangles quoted multi-line arguments), and is never logged.
#
#   list                                          JSON array: session_id, name, status, active
#   send  -Session <id> -TextFile <path>          open that chat, type the text, press Enter
#   new   -TextFile <path> [-Model <name>]        open a new chat, type the first message, print its session id
#   model -Session <id> -Model <name>             open that chat and switch its model
#   selftest                                      the whole path on a scratch chat it creates (by hand, after a merge)
#
# Controls used: Edit 'Message input'; Button 'Send message'; ComboBox 'Switch model' and its modelItem
# options; Button 'New session'; the session rows (AutomationId sessions-list-row-<id>) with their
# statusDot child and the active class. Text that starts with a slash is typed with one leading space,
# or the slash palette takes the keys; a palette item is never invoked, because that sends the bare
# command at once.
#
# Guards, checked again immediately before each batch of keys:
#   1. the operator has been idle for -IdleSeconds, not counting this driver's own input; otherwise
#      wait up to -WaitMinutes, then exit 3 without acting
#   2. send and model: the chat's row shows idle and its session record says idle; same wait, then exit 3
#   3. the chat input is empty (Send disabled); a draft there is exit 4
#   4. the foreground window is the VS Code window, the focused element is the message input, and the
#      active row is the target (for new: no row is active); otherwise type nothing and exit 5
#   5. no input from the operator between the checks and the batch; otherwise type nothing and exit 3
# Exit codes: 0 done and the input cleared; 1 failed or could not confirm; 2 window, chat or control
# not found; 3 operator active or chat busy; 4 draft in the input; 5 a safety check failed.
# Every run appends one line to system/logs/claude-panel.log: time, action, session id, exit code,
# how long the foreground was held.

param(
    [Parameter(Position = 0)][string]$Action = '',
    [string]$Session = '',
    [string]$TextFile = '',
    [string]$Model = '',
    [int]$IdleSeconds = 120,
    [double]$WaitMinutes = 20
)

$ErrorActionPreference = 'Stop'
try { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) } catch { }

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, WindowsBase
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class PanelExit : Exception {
    public int Code;
    public string Reason;
    public PanelExit(int code, string reason) : base(reason) { Code = code; Reason = reason; }
}

public static class PanelNative {
    [StructLayout(LayoutKind.Sequential)]
    public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Explicit, Size = 32)]
    public struct INPUTUNION { [FieldOffset(0)] public KEYBDINPUT ki; }
    [StructLayout(LayoutKind.Sequential)]
    public struct INPUT { public uint type; public INPUTUNION u; }
    [StructLayout(LayoutKind.Sequential)]
    struct LII { public uint cbSize; public uint dwTime; }

    [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LII p);
    [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] i, int size);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern uint MapVirtualKeyW(uint c, uint t);

    // SendInput needs the 64-bit INPUT layout (40 bytes); the union is padded to hold it.
    public static bool InputLayoutOk() { return IntPtr.Size == 8 && Marshal.SizeOf(typeof(INPUT)) == 40; }

    public static uint LastTick() {
        LII l = new LII();
        l.cbSize = (uint)Marshal.SizeOf(typeof(LII));
        GetLastInputInfo(ref l);
        return l.dwTime;
    }
    public static double IdleMs() { return unchecked((uint)Environment.TickCount - LastTick()); }
    public static string Title(IntPtr h) { StringBuilder sb = new StringBuilder(512); GetWindowTextW(h, sb, 512); return sb.ToString(); }

    public static bool BringToFront(IntPtr h) {
        if (GetForegroundWindow() == h) return true;
        if (SetForegroundWindow(h) && GetForegroundWindow() == h) return true;
        IntPtr fg = GetForegroundWindow();
        uint pid;
        uint fgThread = fg == IntPtr.Zero ? 0 : GetWindowThreadProcessId(fg, out pid);
        uint self = GetCurrentThreadId();
        bool attached = fgThread != 0 && fgThread != self && AttachThreadInput(self, fgThread, true);
        try { BringWindowToTop(h); SetForegroundWindow(h); } finally { if (attached) AttachThreadInput(self, fgThread, false); }
        return GetForegroundWindow() == h;
    }

    static INPUT Key(ushort vk, bool up) {
        INPUT i = new INPUT();
        i.type = 1;
        i.u.ki.wVk = vk;
        i.u.ki.wScan = (ushort)MapVirtualKeyW(vk, 0);
        i.u.ki.dwFlags = up ? 2u : 0u;
        return i;
    }
    static INPUT Uni(char c, bool up) {
        INPUT i = new INPUT();
        i.type = 1;
        i.u.ki.wScan = c;
        i.u.ki.dwFlags = up ? 6u : 4u;
        return i;
    }
    static uint Send(List<INPUT> a) {
        INPUT[] arr = a.ToArray();
        return SendInput((uint)arr.Length, arr, Marshal.SizeOf(typeof(INPUT)));
    }
    static List<INPUT> Events(string s) {
        List<INPUT> a = new List<INPUT>();
        foreach (char c in s) {
            if (c == '\n') {
                a.Add(Key(0x10, false)); a.Add(Key(0x0D, false)); a.Add(Key(0x0D, true)); a.Add(Key(0x10, true));
            } else {
                a.Add(Uni(c, false)); a.Add(Uni(c, true));
            }
        }
        return a;
    }
    public static int CountEvents(string s) { return Events(s).Count; }
    // One batch of key events: a line break is Shift+Enter (a new line in the input, not a send).
    public static uint TypeBatch(string s) { return Send(Events(s)); }
    public static uint Enter() {
        List<INPUT> a = new List<INPUT>();
        a.Add(Key(0x0D, false)); a.Add(Key(0x0D, true));
        return Send(a);
    }
}
'@

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$CT = [System.Windows.Automation.ControlType]
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$LogDir = Join-Path $Root 'system\logs'
$LogFile = Join-Path $LogDir 'claude-panel.log'
$TickFile = Join-Path $LogDir 'claude-panel.tick'
$RowPrefix = 'sessions-list-row-'
$ChunkChars = 400
$MaxChars = 20000

$script:Win = $null
$script:Hwnd = [IntPtr]::Zero
$script:Expected = [uint32]0
$script:HeldWatch = $null
$script:Saved = $null

function Stop-Panel([int]$Code, [string]$Reason) {
    throw (New-Object PanelExit($Code, $Reason))
}

# ---------------------------------------------------------------- the window

function Get-VaultName {
    $base = [regex]::Replace($Root, '\\\.claude\\worktrees\\[^\\]+$', '')
    Split-Path -Leaf $base
}

# The title rule: the window title holds the vault folder name and says Visual Studio Code.
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
        if ($null -eq $proc) { continue }
        if ($proc.ProcessName -ne 'Code') { continue }
        $found += $w
    }
    if ($found.Count -eq 0) { Stop-Panel 2 "no VS Code window with '$vault' in its title" }
    if ($found.Count -gt 1) { Stop-Panel 2 "more than one VS Code window has '$vault' in its title; close the extra one" }
    $found[0]
}

function Open-Window {
    $script:Win = Find-Window
    $script:Hwnd = [IntPtr][int64]$script:Win.Current.NativeWindowHandle
    Wait-Tree
}

# ---------------------------------------------------------------- reading the panel

function Find-Named($win, [string]$Name, $Type) {
    $c1 = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $Name)
    $c2 = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $Type)
    $and = New-Object System.Windows.Automation.AndCondition($c1, $c2)
    @($win.FindAll($TS::Descendants, $and) | ForEach-Object { $_ })
}

# Chromium builds the accessibility tree only after a first query, so the panel is read until it answers.
function Wait-Tree {
    for ($i = 0; $i -lt 4; $i++) {
        if (@(Find-Named $script:Win 'New session' $CT::Button).Count -ge 1) { return }
        Start-Sleep -Milliseconds 1500
    }
    Stop-Panel 2 'the Claude panel session list is not showing in the VS Code window'
}

function Get-Chats($win) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Button)
    $tw = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $out = @()
    foreach ($b in $win.FindAll($TS::Descendants, $cond)) {
        $id = $b.Current.AutomationId
        if (-not $id.StartsWith($RowPrefix, [StringComparison]::Ordinal)) { continue }
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
            Id      = $id.Substring($RowPrefix.Length)
            Name    = $name
            Status  = $status
            Active  = [bool]($b.Current.ClassName -match '(^|\s)active_')
            Element = $b
        }
    }
    $out
}

function Get-Chat($win, [string]$Id) {
    $rows = @(Get-Chats $win | Where-Object { $_.Id -eq $Id })
    if ($rows.Count -ne 1) { Stop-Panel 2 "no chat row for session $Id in the panel" }
    $rows[0]
}

function Get-InputBoxes($win) {
    @(Find-Named $win 'Message input' $CT::Edit | Where-Object { -not $_.Current.IsOffscreen })
}

function Get-SendButtons($win) {
    @(Find-Named $win 'Send message' $CT::Button | Where-Object { -not $_.Current.IsOffscreen })
}

function Get-SelectedTabNames($win) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)
    $out = @()
    foreach ($t in $win.FindAll($TS::Descendants, $cond)) {
        if ($t.Current.IsOffscreen) { continue }
        $o = $null
        if ($t.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$o)) {
            if ($o.Current.IsSelected) { $out += $t.Current.Name }
        }
    }
    $out
}

# ---------------------------------------------------------------- the guards

function Get-ConfigDir {
    if ($env:CLAUDE_CONFIG_DIR) { return $env:CLAUDE_CONFIG_DIR }
    Join-Path $env:USERPROFILE '.claude'
}

# The status in the Claude config dir's sessions/ record that names this chat, or '' when none does.
function Get-RecordStatus([string]$Id) {
    $dir = Join-Path (Get-ConfigDir) 'sessions'
    if (-not (Test-Path -LiteralPath $dir)) { return '' }
    foreach ($f in Get-ChildItem -LiteralPath $dir -Filter '*.json' -ErrorAction SilentlyContinue) {
        try {
            $j = Get-Content -LiteralPath $f.FullName -Raw | ConvertFrom-Json
            if ($j.sessionId -eq $Id) { return [string]$j.status }
        } catch { }
    }
    ''
}

# Guard 2. '' when the chat is idle, else why not.
function Get-ChatBusy($win, [string]$Id) {
    $row = Get-Chat $win $Id
    if ($row.Status -eq 'running') { return 'the chat is running' }
    $rec = Get-RecordStatus $Id
    if ($rec -ne '' -and $rec -ne 'idle') { return "its session record says $rec" }
    ''
}

function Get-OwnTick {
    if (-not (Test-Path -LiteralPath $TickFile)) { return $null }
    try { return [uint32](Get-Content -LiteralPath $TickFile -Raw) } catch { return $null }
}

# The driver's own SendInput resets the system's last-input time, so the tick it left is remembered:
# when the last input is still that one, nobody has touched the keyboard since.
function Save-OwnTick {
    if (-not (Test-Path -LiteralPath $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }
    Set-Content -LiteralPath $TickFile -Value ([PanelNative]::LastTick())
    $script:Expected = [PanelNative]::LastTick()
}

# Guard 1 as a state: the operator has been idle for -IdleSeconds, not counting this driver's own input.
function Test-OperatorIdle {
    $tick = [PanelNative]::LastTick()
    $own = Get-OwnTick
    if ($null -ne $own -and $tick -eq $own) { return $true }
    ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000))
}

# Guards 1 and 2: wait until both hold, up to -WaitMinutes, then exit 3. Sets the input baseline.
function Wait-Guards([string]$Id) {
    $deadline = (Get-Date).AddMinutes($WaitMinutes)
    while ($true) {
        $why = ''
        if (-not (Test-OperatorIdle)) {
            $why = 'the operator is using the computer'
        } elseif ($Id -ne '') {
            $why = Get-ChatBusy $script:Win $Id
        }
        if ($why -eq '') {
            $script:Expected = [PanelNative]::LastTick()
            return
        }
        if ((Get-Date) -ge $deadline) { Stop-Panel 3 "gave up after $WaitMinutes minutes: $why; nothing was typed" }
        Start-Sleep -Seconds 5
    }
}

# Guards 5, 2, 3 and 4, in that order, immediately before a batch of keys.
#   InputMode: empty (Send is disabled), typed (Send is enabled), any (not checked)
#   Target: the session id that must be the active row, or '' for a new chat (no row is active)
function Assert-Ready([string]$Target, [string]$InputMode, [string]$ChatName) {
    $win = $script:Win
    if ([PanelNative]::LastTick() -ne $script:Expected) { Stop-Panel 3 'the operator touched the keyboard or pointer just before typing; nothing more was typed' }
    if ($Target -ne '') {
        $busy = Get-ChatBusy $win $Target
        if ($busy -ne '') { Stop-Panel 3 "$busy; nothing was typed" }
    }
    if ($InputMode -ne 'any') {
        $sends = Get-SendButtons $win
        if ($sends.Count -ne 1) { Stop-Panel 5 'the Send button is not showing; nothing was typed' }
        $enabled = [bool]$sends[0].Current.IsEnabled
        if ($InputMode -eq 'empty' -and $enabled) { Stop-Panel 4 "$ChatName has text in its input box (a draft); nothing was typed" }
        if ($InputMode -eq 'typed' -and -not $enabled) { Stop-Panel 1 'the typed text did not register in the input box' }
    }
    $fg = [PanelNative]::GetForegroundWindow()
    if ($fg -ne $script:Hwnd) { Stop-Panel 5 'the foreground window is not the VS Code window; nothing was typed' }
    if (-not (Test-VaultTitle ([PanelNative]::Title($fg)) (Get-VaultName))) { Stop-Panel 5 'the foreground window title does not name the vault; nothing was typed' }
    $focused = $AE::FocusedElement
    if ($null -eq $focused -or $focused.Current.Name -ne 'Message input' -or $focused.Current.ControlType -ne $CT::Edit) {
        Stop-Panel 5 'the keyboard focus is not on the message input; nothing was typed'
    }
    $active = @(Get-Chats $win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
    if ($Target -ne '') {
        if ($active.Count -ne 1 -or $active[0] -ne $Target) { Stop-Panel 5 'the active chat is not the target; nothing was typed' }
    } elseif ($active.Count -ne 0) {
        Stop-Panel 5 'the open tab is a chat that already has a row, not a new one; nothing was typed'
    }
}

# ---------------------------------------------------------------- the view, saved and restored

function Save-View {
    $script:Saved = [pscustomobject]@{
        Fg     = [PanelNative]::GetForegroundWindow()
        Active = @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
        Tabs   = @(Get-SelectedTabNames $script:Win)
    }
}

# Best effort; never throws. The previously active row, the previously selected tabs, the foreground window.
function Restore-View {
    if ($null -eq $script:Saved -or $null -eq $script:Win) { return }
    try {
        $win = $script:Win
        $prev = $script:Saved
        if ($prev.Active.Count -eq 1) {
            $now = @(Get-Chats $win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
            if (-not ($now.Count -eq 1 -and $now[0] -eq $prev.Active[0])) {
                $row = @(Get-Chats $win | Where-Object { $_.Id -eq $prev.Active[0] })
                if ($row.Count -eq 1) {
                    $row[0].Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    Start-Sleep -Milliseconds 800
                }
            }
        }
        $have = @(Get-SelectedTabNames $win)
        foreach ($name in $prev.Tabs) {
            if ($have -contains $name) { continue }
            $tabs = @(Find-Named $win $name $CT::TabItem | Where-Object { -not $_.Current.IsOffscreen })
            if ($tabs.Count -eq 1) {
                $o = $null
                if ($tabs[0].TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$o)) { $o.Select() }
            }
        }
        if ($prev.Fg -ne [IntPtr]::Zero -and $prev.Fg -ne $script:Hwnd -and [PanelNative]::IsWindow($prev.Fg)) {
            [void][PanelNative]::BringToFront($prev.Fg)
        }
    } catch { }
}

function Open-Row([string]$Id) {
    $win = $script:Win
    $row = Get-Chat $win $Id
    if (-not $row.Active) {
        $row.Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    }
    for ($i = 0; $i -lt 24; $i++) {
        $active = @(Get-Chats $win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
        $inputs = (Get-InputBoxes $win).Count
        if ($active.Count -eq 1 -and $active[0] -eq $Id -and $inputs -eq 1) { return }
        if ($i -eq 12 -and -not ($active -contains $Id)) {
            # The row did not take the first time: once more, by the same pattern.
            $again = @(Get-Chats $win | Where-Object { $_.Id -eq $Id })
            if ($again.Count -eq 1) { $again[0].Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
        }
        Start-Sleep -Milliseconds 250
    }
    $isTarget = 'no'
    if ($active -contains $Id) { $isTarget = 'yes' }
    Stop-Panel 5 ('the chat did not open in the panel ({0} active rows, target active: {1}, {2} message inputs showing)' -f $active.Count, $isTarget, $inputs)
}

# ---------------------------------------------------------------- typing

# The text as typed: line breaks as LF, a tab as a space, other control characters dropped, one
# leading space when it starts with a slash (idempotent: a text that already starts with a space stays).
function Format-TypedText([string]$Raw) {
    $t = $Raw.TrimStart([char]0xFEFF)
    $t = $t.Replace("`r`n", "`n").Replace("`r", "`n")
    $t = $t.TrimEnd([char]10)
    $t = $t.Replace("`t", ' ')
    $t = [regex]::Replace($t, '[\x00-\x09\x0B\x0C\x0E-\x1F\x7F]', '')
    if ($t.StartsWith('/')) { $t = ' ' + $t }
    $t
}

function Read-TextFile([string]$Path) {
    if (-not [PanelNative]::InputLayoutOk()) { Stop-Panel 1 'the 64-bit Windows PowerShell is needed to send keys' }
    if ($Path -eq '' -or -not (Test-Path -LiteralPath $Path)) { Stop-Panel 1 'no -TextFile, or it does not exist' }
    $text = Format-TypedText ([System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8))
    if ($text.Trim().Length -eq 0) { Stop-Panel 1 'the text file is empty' }
    if ($text.Length -gt $MaxChars) { Stop-Panel 1 "the text is longer than $MaxChars characters" }
    $text
}

function Split-Text([string]$Text) {
    $parts = @()
    $i = 0
    while ($i -lt $Text.Length) {
        $n = [Math]::Min($ChunkChars, $Text.Length - $i)
        if ($n -lt ($Text.Length - $i) -and [char]::IsHighSurrogate($Text[$i + $n - 1])) { $n++ }
        $parts += $Text.Substring($i, $n)
        $i += $n
    }
    $parts
}

function Set-InputFocus {
    $win = $script:Win
    if ([PanelNative]::IsIconic($script:Hwnd)) { [void][PanelNative]::ShowWindow($script:Hwnd, 9) }
    if (-not [PanelNative]::BringToFront($script:Hwnd)) { Stop-Panel 5 'could not take the foreground for the VS Code window' }
    $script:HeldWatch = [Diagnostics.Stopwatch]::StartNew()
    $boxes = Get-InputBoxes $win
    if ($boxes.Count -ne 1) { Stop-Panel 5 "the panel shows $($boxes.Count) message inputs, not one; nothing was typed" }
    for ($i = 0; $i -lt 4; $i++) {
        try { $boxes[0].SetFocus() } catch { Stop-Panel 5 'the message input would not take the keyboard focus' }
        Start-Sleep -Milliseconds 300
        $f = $AE::FocusedElement
        if ($null -ne $f -and $f.Current.Name -eq 'Message input') { break }
    }
    Start-Sleep -Milliseconds 500
}

# What can be said about a typing that did not register, without any of the text: the input's length
# and what has the keyboard focus, the foreground window, and whether the operator touched input since.
function Get-TypingFacts {
    $len = 'unknown'
    $boxes = Get-InputBoxes $script:Win
    if ($boxes.Count -eq 1) {
        $o = $null
        if ($boxes[0].TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$o)) { $len = [string]$o.Current.Value.Length }
    }
    $f = $AE::FocusedElement
    $focus = 'none'
    if ($null -ne $f) { $focus = [string]$f.Current.Name }
    if ($focus.Length -gt 24) { $focus = $focus.Substring(0, 24) }
    $fgIs = ([PanelNative]::GetForegroundWindow() -eq $script:Hwnd)
    $moved = ([PanelNative]::LastTick() -ne $script:Expected)
    'Input value length {0}, focus on [{1}], VS Code in front {2}, operator input since typing {3}.' -f $len, $focus, $fgIs, $moved
}

# Guards 3 to 5 for the text, then the Enter that sends it; confirms the input cleared.
function Send-Text([string]$Text, [string]$Target, [string]$ChatName) {
    $win = $script:Win
    Set-InputFocus
    $first = $true
    foreach ($chunk in (Split-Text $Text)) {
        if ($first) { Assert-Ready $Target 'empty' $ChatName } else { Assert-Ready $Target 'any' $ChatName }
        $want = [PanelNative]::CountEvents($chunk)
        $sent = [PanelNative]::TypeBatch($chunk)
        Save-OwnTick
        if ($sent -ne $want) { Stop-Panel 1 "only $sent of $want key events were accepted; the typed text is left in the input" }
        $first = $false
        Start-Sleep -Milliseconds 150
    }
    $typed = $false
    for ($i = 0; $i -lt 10; $i++) {
        $s = Get-SendButtons $win
        if ($s.Count -eq 1 -and $s[0].Current.IsEnabled) { $typed = $true; break }
        Start-Sleep -Milliseconds 200
    }
    if (-not $typed) { Stop-Panel 1 ('the typed text did not register in the input box (Send stayed disabled); it may be left in the input. ' + (Get-TypingFacts)) }
    Assert-Ready $Target 'typed' $ChatName
    $n = [PanelNative]::Enter()
    Save-OwnTick
    if ($n -ne 2) { Stop-Panel 1 'Enter was not accepted' }
    # Cleared: Send is disabled, or gone because the chat is now running.
    for ($i = 0; $i -lt 25; $i++) {
        Start-Sleep -Milliseconds 200
        $s = Get-SendButtons $win
        if ($s.Count -eq 0 -or -not $s[0].Current.IsEnabled) { return }
    }
    Stop-Panel 1 'Enter did not clear the input; the text may be unsent in the chat'
}

# ---------------------------------------------------------------- the model picker

function Select-Model([string]$Name) {
    $win = $script:Win
    $combos = @(Find-Named $win 'Switch model' $CT::ComboBox | Where-Object { -not $_.Current.IsOffscreen })
    if ($combos.Count -ne 1) { Stop-Panel 2 'the model picker is not showing' }
    $combo = $combos[0]
    $ec = $combo.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
    $vp = $combo.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    try {
        if ($ec.Current.ExpandCollapseState -ne [System.Windows.Automation.ExpandCollapseState]::Expanded) { $ec.Expand() }
        $items = @()
        for ($i = 0; $i -lt 12; $i++) {
            Start-Sleep -Milliseconds 250
            $items = @($win.FindAll($TS::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::ListItem))) |
                Where-Object { $_.Current.ClassName -like '*modelItem_*' })
            if ($items.Count -gt 0) { break }
        }
        if ($items.Count -eq 0) { Stop-Panel 2 'the model picker listed no options' }
        if ($Name -eq '') {
            $active = @($items | Where-Object { $_.Current.ClassName -like '*activeModelItem_*' })
            return ('{0} options, active: {1}' -f $items.Count, $(if ($active.Count -ge 1) { $active[0].Current.Name } else { 'none' }))
        }
        $match = @($items | Where-Object { $_.Current.Name -ieq $Name -or $_.Current.Name.StartsWith($Name + ' ', [StringComparison]::OrdinalIgnoreCase) })
        if ($match.Count -ne 1) {
            $names = ($items | ForEach-Object { $_.Current.Name }) -join ' | '
            Stop-Panel 2 "$($match.Count) model options match '$Name'; the options are: $names"
        }
        $o = $null
        if ($match[0].TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$o)) {
            $o.Invoke()
        } elseif ($match[0].TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$o)) {
            $o.Select()
        } else {
            Stop-Panel 1 'the model option offers no way to choose it'
        }
        for ($i = 0; $i -lt 12; $i++) {
            Start-Sleep -Milliseconds 250
            $v = $vp.Current.Value
            if ($v -ieq $Name -or $v.StartsWith($Name + ' ', [StringComparison]::OrdinalIgnoreCase)) { return $v }
        }
        Stop-Panel 1 "the model picker still says '$($vp.Current.Value)', not '$Name'"
    } finally {
        try { if ($ec.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) { $ec.Collapse() } } catch { }
    }
}

# ---------------------------------------------------------------- the actions

function Invoke-Send([string]$Id, [string]$File) {
    $text = Read-TextFile $File
    Open-Window
    Wait-Guards $Id
    Save-View
    $chat = Get-Chat $script:Win $Id
    Open-Row $Id
    Send-Text $text $Id $chat.Name
    'sent'
}

function Invoke-New([string]$File, [string]$ModelName) {
    $text = Read-TextFile $File
    Open-Window
    Wait-Guards ''
    Save-View
    $win = $script:Win
    $before = @(Get-Chats $win | ForEach-Object { $_.Id })
    $btn = @(Find-Named $win 'New session' $CT::Button | Where-Object { $_.Current.IsEnabled })
    if ($btn.Count -lt 1) { Stop-Panel 2 'the New session button is not available' }
    $btn[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $ready = $false
    for ($i = 0; $i -lt 32; $i++) {
        Start-Sleep -Milliseconds 250
        $active = @(Get-Chats $win | Where-Object { $_.Active })
        if ($active.Count -eq 0 -and (Get-InputBoxes $win).Count -eq 1) { $ready = $true; break }
    }
    if (-not $ready) { Stop-Panel 5 'a new chat tab did not open' }
    if ($ModelName -ne '') { [void](Select-Model $ModelName) }
    Send-Text $text '' 'the new chat'
    for ($i = 0; $i -lt 120; $i++) {
        $now = @(Get-Chats $win | Where-Object { $before -notcontains $_.Id })
        if ($now.Count -ge 1) { return $now[0].Id }
        Start-Sleep -Milliseconds 500
    }
    Stop-Panel 1 'the message was sent but no new chat row appeared within 60 seconds'
}

function Invoke-Model([string]$Id, [string]$ModelName) {
    if ($ModelName -eq '') { Stop-Panel 1 '-Model is needed' }
    Open-Window
    Wait-Guards $Id
    Save-View
    Open-Row $Id
    if ((Get-ChatBusy $script:Win $Id) -ne '') { Stop-Panel 3 'the chat started running; the model was not changed' }
    Select-Model $ModelName
}

function Invoke-List {
    Open-Window
    $rows = @(Get-Chats $script:Win | ForEach-Object {
        [pscustomobject]@{ session_id = $_.Id; name = $_.Name; status = $_.Status; active = $_.Active }
    })
    ConvertTo-Json -InputObject $rows -Compress
}

# ---------------------------------------------------------------- one logged run

function Write-PanelLog([string]$ActionName, [string]$Id, [int]$Code, [string]$Reason) {
    try {
        if (-not (Test-Path -LiteralPath $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }
        $held = 0
        if ($null -ne $script:HeldWatch) { $held = [int]$script:HeldWatch.ElapsedMilliseconds }
        $why = ''
        if ($Code -ne 0) { $why = ' ' + ($Reason -replace '[\r\n]+', ' ') }
        $line = '{0} {1} {2} exit={3} held_ms={4}{5}' -f (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss'), $ActionName, $(if ($Id -ne '') { $Id } else { '-' }), $Code, $held, $why
        [System.IO.File]::AppendAllText($LogFile, $line + "`r`n", (New-Object System.Text.UTF8Encoding($false)))
    } catch { }
}

# Runs one action's body, restores the view, logs the run, and returns Code, Reason and Value.
function Invoke-Logged([string]$ActionName, [string]$LogId, [scriptblock]$Body) {
    $script:HeldWatch = $null
    $script:Saved = $null
    $r = [pscustomobject]@{ Code = 0; Reason = 'ok'; Value = $null }
    try {
        $r.Value = & $Body
    } catch [PanelExit] {
        $r.Code = $_.Exception.Code
        $r.Reason = $_.Exception.Reason
    } catch {
        $r.Code = 1
        $r.Reason = 'unexpected: ' + $_.Exception.Message
    }
    Restore-View
    if ($null -ne $script:HeldWatch) { $script:HeldWatch.Stop() }
    Write-PanelLog $ActionName $LogId $r.Code $r.Reason
    $r
}

function Out-Result($r) {
    if ($r.Code -eq 0) {
        if ($null -ne $r.Value) { [Console]::Out.WriteLine([string]$r.Value) }
    } else {
        [Console]::Error.WriteLine($r.Reason)
    }
    $r.Code
}

# ---------------------------------------------------------------- selftest

function Get-TranscriptBoundaries([string]$Id) {
    $found = @(Get-ChildItem -LiteralPath (Join-Path (Get-ConfigDir) 'projects') -Recurse -Filter "$Id.jsonl" -ErrorAction SilentlyContinue)
    if ($found.Count -eq 0) { return -1 }
    @(Select-String -LiteralPath $found[0].FullName -Pattern 'compact_boundary' -SimpleMatch).Count
}

function Write-StepLine([string]$Step, $r, [string]$Detail) {
    $verdict = 'FAIL'
    if ($r.Code -eq 0) { $verdict = 'PASS' }
    $note = $Detail
    if ($r.Code -ne 0) { $note = 'exit {0}: {1}' -f $r.Code, $r.Reason }
    [Console]::Out.WriteLine(('{0} {1}: {2}' -f $verdict, $Step, $note))
}

function Invoke-SelfTest {
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ('claude-panel-selftest-' + [guid]::NewGuid().ToString('N') + '.txt')
    $failed = 0
    $id = ''
    try {
        [System.IO.File]::WriteAllText($tmp, 'Reply with the single word OK.', (New-Object System.Text.UTF8Encoding($false)))
        $r = Invoke-Logged 'new' '' { Invoke-New $tmp '' }
        if ($r.Code -eq 0) { $id = [string]$r.Value }
        Write-StepLine 'new' $r $id
        if ($r.Code -ne 0) { return 1 }

        $newName = 'zz-selftest-' + (Get-Date).ToString('yyyyMMddHHmm')
        [System.IO.File]::WriteAllText($tmp, "/rename $newName", (New-Object System.Text.UTF8Encoding($false)))
        $r = Invoke-Logged 'send' $id { Invoke-Send $id $tmp }
        if ($r.Code -eq 0) {
            $seen = $false
            for ($i = 0; $i -lt 30 -and -not $seen; $i++) {
                Start-Sleep -Milliseconds 500
                $row = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $id })
                if ($row.Count -eq 1 -and $row[0].Name -eq $newName) { $seen = $true }
            }
            if (-not $seen) { $r.Code = 1; $r.Reason = 'the panel row did not take the new name' }
        }
        Write-StepLine 'send /rename' $r 'the row shows the new name'
        if ($r.Code -ne 0) { $failed++ }

        $before = Get-TranscriptBoundaries $id
        [System.IO.File]::WriteAllText($tmp, '/compact Keep only the word OK.', (New-Object System.Text.UTF8Encoding($false)))
        $r = Invoke-Logged 'send' $id { Invoke-Send $id $tmp }
        if ($r.Code -eq 0) {
            $seen = $false
            for ($i = 0; $i -lt 240 -and -not $seen; $i++) {
                if ((Get-TranscriptBoundaries $id) -gt $before) { $seen = $true } else { Start-Sleep -Milliseconds 500 }
            }
            if (-not $seen) { $r.Code = 1; $r.Reason = 'the transcript gained no compact_boundary line within 2 minutes' }
        }
        Write-StepLine 'send /compact' $r 'the transcript gained a compact_boundary line'
        if ($r.Code -ne 0) { $failed++ }

        $r = Invoke-Logged 'model' $id { Open-Window; Wait-Guards $id; Save-View; Open-Row $id; Select-Model '' }
        Write-StepLine 'model (read only)' $r ([string]$r.Value)
        if ($r.Code -ne 0) { $failed++ }
    } finally {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    }
    if ($id -ne '') { [Console]::Out.WriteLine("scratch chat: $id (left open; close it by hand)") }
    if ($failed -gt 0) { return 1 }
    0
}

# ---------------------------------------------------------------- main

if ($Session -ne '' -and $Session -notmatch '^[0-9A-Fa-f-]{8,64}$') {
    [Console]::Error.WriteLine('-Session is a session id (hex digits and dashes)')
    exit 1
}
$code = 1
switch ($Action) {
    'list'     { $code = Out-Result (Invoke-Logged 'list' '' { Invoke-List }) }
    'send'     {
        if ($Session -eq '') { [Console]::Error.WriteLine('send needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'send' $Session { Invoke-Send $Session $TextFile })
        }
    }
    'new'      { $code = Out-Result (Invoke-Logged 'new' '' { Invoke-New $TextFile $Model }) }
    'model'    {
        if ($Session -eq '') { [Console]::Error.WriteLine('model needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'model' $Session { Invoke-Model $Session $Model })
        }
    }
    'selftest' { $code = Invoke-SelfTest }
    default    { [Console]::Error.WriteLine('usage: claude_panel.ps1 list | send | new | model | selftest'); $code = 1 }
}
exit $code
