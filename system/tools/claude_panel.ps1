# The Claude panel driver, version 2. It manages chats in the Claude Code panel of the one VS Code window
# whose title holds the vault folder name, and nothing else. Except for the group actions below it sends no key
# events and no mouse events: it reads and acts through Windows UI Automation patterns (read, Invoke,
# ExpandCollapse, SelectionItem, ScrollItem) and through the AgentFrame bridge extension (system/vscode-bridge),
# which opens, reveals and closes chat tabs and prefills a chat's input from a file inbox. Besides the group
# actions the one exception is 'archive': the row's Archive button exists only while the pointer rests on the
# row, so 'archive' moves the pointer onto the row and back; nothing is clicked or typed. Text always arrives in
# a UTF-8 file, never as an argument, and is never logged (a group's name is the one short value passed as an
# argument; no group name and no typed text is ever logged).
#
# Group actions (groups reads only; group-create, group-rename, group-delete, group-add, group-remove change the
# list) are the one place that sends synthetic input, because the menus that hold them open only on a real
# right-click and a group's name is typed. All of it lives in one native class, PanelInput: a right click at the
# pointer, Unicode text, Ctrl+A, Enter, Escape, and the call that brings VS Code to the front. It is used only
# from the group functions, each behind its own check: a right click only while VS Code is in front, no menu is
# open, the target is on screen with the pointer on it and the pointer is where the driver last put it; typing
# only while the keyboard focus is the group's name field (inside the Sessions group), and Enter only while that
# field shows exactly the wanted name; Escape only while a context menu is seen open and VS Code is in front. A
# group action takes the operator's pointer and keyboard focus for a few seconds (the operator allowed it while he
# is idle), puts the pointer back, closes a menu it opened, and hands the foreground back to the window that had
# it. It refuses (exit 5) before touching anything when the request cannot be done, and verifies its outcome from
# a fresh read of the list.
# VS Code takes the keyboard focus during a run: the Claude extension opens a chat's panel with focus (the bridge's
# open and reveal), and an Invoke on a panel control (Send, the Archive button, a model item, a row) activates the
# window. The driver does not try to prevent that. At the end of a run, after an archive's z-order undo and before
# the window is minimized again, it makes ONE check: when VS Code has the foreground, a different window had it when
# the screen work began, that window still exists and is visible, and the last-input tick is unchanged (he did
# nothing), the foreground is given back to that recorded window with one native call (GiveForegroundBack, the only
# place the foreground is set, never with VS Code as its argument) and checked within 500 ms; if it did not work the
# note 'VS Code was left in front' is added and nothing else is tried. If he acted, nothing is moved. A minimized
# VS Code window is
# restored behind the window he has in front (to the bottom of the stack, shown without activation, then
# placed directly behind his foreground window unless that window is always-on-top; a window that was maximized
# comes back maximized), which does not reset the system's last-input time, and is minimized again at the end
# without activation, but only when VS Code is not his foreground window (it can only be there because he brought
# it there) and he is idle or another window covers it (else it is left open, with a note). 'archive' alone changes what he sees: when another window covers the row it raises
# VS Code above it without keyboard focus (and puts it back exactly where it was in the window order), and it waits while a
# full-screen window is in front (the shell's notification state 2, 3 or 4), then exits 3 at the deadline.
#
#   list                                       JSON array: session_id, name, status, active, group
#   send  -Session <id> -TextFile <path>       send the file's text to that chat (a prompt or a slash command)
#   new   -TextFile <path> -Model <name>       start a chat on that model with the file as its first message;
#                                              prints its session id on the last line
#   groups                                     JSON array of the list's groups: name, count, fixed (read only)
#   group-create -Group <name>                 add a group: press New group, type the name, Enter
#   group-rename -Group <old> -NewName <new>   right click the group, Rename group, type the new name, Enter
#   group-delete -Group <name> [-Force]        right click the group, Delete group (a group with chats needs -Force)
#   group-add -Session <id> -Group <name>      right click the chat's row, Add to group, that group's item
#   group-remove -Session <id>                 right click the chat's row, Remove from group
#   model -Session <id> [-Model <name>]        switch an idle chat's model; without -Model, list the options
#   archive -Session <id>                      archive an idle chat: close its tab if open, rest the pointer on its
#                                              row, invoke the row's 'Archive session' button, confirm the row left
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
#      (pid, session id, action, start); a second run waits within its budget, then exits 3. The lock is
#      taken AFTER the wait for him and the chat: a run waits for guards 1 and 2 without it (a waiting run
#      must not make every other chat's panel command wait), takes it, checks guards 1 and 2 once more with
#      no waiting, and if one fails lets the lock go and waits again; a run that already holds the lock
#      keeps it. Only when the lock and the guards hold together does it change anything
# The run tracks what it changed (tab closed, tab opened, text prefilled, Send pressed; a bridge step that
# timed out counts as attempted) and every exit says it, then states the tab's final state from a fresh
# read (the bridge's tab list, the chat's process, and its input when that tab is in front). On a failure
# before Send it puts the chat back where that is safe: its own prefilled tab is closed only through the
# unbroken check, as its own (the chat's process is the one its open started), and a tab that was open at
# the start is opened again whenever it is closed. Once the text is sent and confirmed the exit is 0; a
# later cleanup failure is a note, and it closes only a tab it opened in this run that was closed before.
# The window may be minimized: the accessibility tree is stale then, so the driver restores it behind his
# window (see above) first and minimizes it again at the end. Every model pick saves the global model key and
# the settings file hash first, writes the key back right after the pick, and verifies (exit 6 if the
# file does not match).
# Deadline: a run ends within -WaitMinutes plus $WorkSeconds; every step is bounded by it, and the chat is
# put back before it: $RestoreSeconds to wait for him, then $SettleSeconds of its own for the chat's
# process to end and its tab to come back. af's timeout for a run is that plus a margin (panel.rs:
# DRIVER_WORK_SECS). A list reads only and has a short deadline of its own ($ListWorkSeconds).
# Exit codes: 0 done and confirmed; 1 failed or could not confirm; 2 bridge, window, chat or control not
# found; 3 operator active, chat busy or another run holding the lock; 4 the input holds text that is
# not the driver's; 5 a safety check failed; 6 the model key or settings file does not match after a
# pick; 7 cancelled through -CancelFile before anything was changed. 3, 4 and 5 mean nothing was sent
# (nothing was archived, for 'archive').
# Every run appends one line to system/logs/claude-panel.log: time, action, session id, exit code, seconds.
# While a run waits it also appends one line per change of reason: time, action, session id, 'waiting: <reason>'
# (never a chat title or text), so a long run can be explained afterwards.
# Cancel: with -CancelFile <path> a run that is only waiting and has changed nothing yet (the operator and the
# chat guards, the wait for an idle row, the lock wait) stops when that file exists: exit 7, 'cancelled;
# nothing was changed'. Once it has closed a tab, opened one or prefilled text it never cancels. It writes
# <path with the extension .sending> when it first changes the panel, so af can tell the two apart.
# Archive (exit codes): 0 archived and the row left the list, or no row for the chat in the list as shown
# (nothing changed); 2 the sessions list is not readable; 3 the operator is active or came back, the chat is
# running, busy, in use again or the one he has in front, or a full-screen window stayed in front of the row;
# 4 a draft in the open tab; 5 a safety check failed (a record from outside the panel, the row covered by
# another window or with no usable place, no Archive button); 1 the row did not leave the list. A chat he has
# in front is never archived: the run waits for that to change, then exits 3. Every wait comes before the
# raise: after VS Code is raised nothing waits except the 3 second button poll and the 5 second row-gone
# confirm, so the raise is undone within about 8 seconds. The pointer is put back only if it is still where
# the driver put it. A window that was maximized before it was minimized is minimized again with its saved
# placement, so it comes back maximized.

param(
    [Parameter(Position = 0)][string]$Action = '',
    [string]$Session = '',
    [string]$TextFile = '',
    [string]$Model = '',
    [string]$Group = '',
    [string]$NewName = '',
    [switch]$Force,
    [int]$IdleSeconds = 120,
    [double]$WaitMinutes = 20,
    [string]$CancelFile = ''
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
    // The last input's tick: compared for equality, it changes when he does anything at all.
    public static uint InputTick() {
        LII l = new LII();
        l.cbSize = (uint)Marshal.SizeOf(typeof(LII));
        GetLastInputInfo(ref l);
        return l.dwTime;
    }
    public static string Title(IntPtr h) { StringBuilder sb = new StringBuilder(512); GetWindowTextW(h, sb, 512); return sb.ToString(); }

    // Only for 'archive': the pointer is moved onto a row and back, and VS Code is raised above a covering
    // window without taking the keyboard focus (and put back). Nothing here presses, types or activates.
    [StructLayout(LayoutKind.Sequential)]
    struct PT { public int X; public int Y; }
    [DllImport("user32.dll")] static extern bool GetCursorPos(out PT p);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(PT p);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    const uint SWP_NOSIZE = 0x1, SWP_NOMOVE = 0x2, SWP_NOACTIVATE = 0x10;
    // The pointer's position as { x, y }, or null when it cannot be read.
    public static int[] PointerPos() {
        PT p = new PT();
        if (!GetCursorPos(out p)) return null;
        return new int[] { p.X, p.Y };
    }
    public static bool MovePointer(int x, int y) { return SetCursorPos(x, y); }
    // The top-level window under a screen point (0 when there is none).
    public static IntPtr RootAt(int x, int y) {
        PT p = new PT(); p.X = x; p.Y = y;
        IntPtr h = WindowFromPoint(p);
        if (h == IntPtr.Zero) return IntPtr.Zero;
        return GetAncestor(h, 2);
    }
    // Always-on-top, no move, no resize, no activation: the window comes forward but gets no keyboard focus.
    public static bool RaiseNoActivate(IntPtr h) { return SetWindowPos(h, new IntPtr(-1), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); }
    public static bool ClearTopmost(IntPtr h) { return SetWindowPos(h, new IntPtr(-2), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); }
    // The window directly behind another one.
    public static bool PlaceBehind(IntPtr h, IntPtr front) { return SetWindowPos(h, front, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); }

    // Restoring and minimizing the VS Code window without the foreground or the idle time changing (measured:
    // the Window pattern does both). The window goes to the bottom of the stack while still minimized, is shown
    // without activation, and is put directly behind the window that has the foreground (PlaceBehind).
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("shell32.dll")] static extern int SHQueryUserNotificationState(out int state);
    // Only reads which window has the foreground, so the driver can prove it did not change.
    public static IntPtr Foreground() { return GetForegroundWindow(); }
    public static bool PlaceBottom(IntPtr h) { return SetWindowPos(h, new IntPtr(1), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); }
    public static bool ShowNoActivate(IntPtr h) { return ShowWindow(h, 4); }
    public static bool MinimizeNoActivate(IntPtr h) { return ShowWindow(h, 7); }
    // The one place the driver sets the foreground window: at the end of a run it goes back to the window that had it
    // when the screen work began. The argument is that earlier window, never the VS Code window (the caller refuses).
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    public static bool GiveForegroundBack(IntPtr previous) { return SetForegroundWindow(previous); }
    // Whether the window is in the always-on-top band (WS_EX_TOPMOST in its extended style).
    [DllImport("user32.dll")] static extern int GetWindowLongW(IntPtr h, int index);
    public static bool IsTopmost(IntPtr h) { return (GetWindowLongW(h, -20) & 0x8) != 0; }
    // The window directly above another one in the z-order (0 when it is at the very top): GW_HWNDPREV.
    [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr h, uint cmd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int attr, out int value, int size);
    public static IntPtr Prev(IntPtr h) { return GetWindow(h, 3); }   // GW_HWNDPREV: the window directly above
    public static IntPtr Owner(IntPtr h) { return GetWindow(h, 4); }  // GW_OWNER
    public static bool IsVisible(IntPtr h) { return IsWindowVisible(h); }
    // DWMWA_CLOAKED = 14: non-zero when the shell hides the window (another virtual desktop, a suspended app).
    // A failed call counts as not cloaked.
    public static bool IsCloaked(IntPtr h) { int v = 0; if (DwmGetWindowAttribute(h, 14, out v, 4) != 0) return false; return v != 0; }
    // A window's placement, saved before the no-focus restore and put back when it is minimized again, with the
    // flag that says it restores to maximized. (Measured: SW_SHOWNOACTIVATE restores a window that was minimized
    // from maximized to maximized, and SW_SHOWMINNOACTIVE or a placement with showCmd 7 keeps that flag; a
    // placement with showCmd 2, SW_SHOWMINIMIZED, led to a foreground change, so showCmd is always 7.)
    [StructLayout(LayoutKind.Sequential)]
    struct WP { public int length, flags, showCmd, minX, minY, maxX, maxY, left, top, right, bottom; }
    [DllImport("user32.dll")] static extern bool GetWindowPlacement(IntPtr h, ref WP p);
    [DllImport("user32.dll")] static extern bool SetWindowPlacement(IntPtr h, ref WP p);
    public static int[] GetPlacement(IntPtr h) {
        WP p = new WP();
        p.length = Marshal.SizeOf(typeof(WP));
        if (!GetWindowPlacement(h, ref p)) return null;
        return new int[] { p.flags, p.showCmd, p.minX, p.minY, p.maxX, p.maxY, p.left, p.top, p.right, p.bottom };
    }
    // Minimized without activation (SW_SHOWMINNOACTIVE, showCmd 7 and never 2), with the saved flags and rectangle.
    public static bool MinimizeWithPlacement(IntPtr h, int[] q) {
        WP p = new WP();
        p.length = Marshal.SizeOf(typeof(WP));
        p.flags = q[0]; p.showCmd = 7;
        p.minX = q[2]; p.minY = q[3]; p.maxX = q[4]; p.maxY = q[5];
        p.left = q[6]; p.top = q[7]; p.right = q[8]; p.bottom = q[9];
        return SetWindowPlacement(h, ref p);
    }
    // The shell's view of the screen: 2 a full-screen window is in front, 3 a Direct3D full-screen
    // application, 4 presentation mode (0 when it cannot be read). Idle time alone cannot see a video.
    public static int NotificationState() {
        int s = 0;
        if (SHQueryUserNotificationState(out s) != 0) return 0;
        return s;
    }
}

public static class PanelInput {
    // The one place that sends synthetic input and takes the foreground, for the group actions only (see the
    // header). Exactly these methods: a right click at the pointer, Unicode text, Ctrl+A, Enter, Escape and
    // BringToFront. There is no general "press this key" method, and nothing else in the driver reaches the
    // imports below. Every caller checks its own conditions first (the group functions, each named in the tests).
    [StructLayout(LayoutKind.Sequential)]
    struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)]
    struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Explicit)]
    struct INPUTUNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
    [StructLayout(LayoutKind.Sequential)]
    struct INPUT { public uint type; public INPUTUNION u; }
    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint self, uint other, bool attach);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    const uint TYPE_MOUSE = 0, TYPE_KEYBOARD = 1;
    const uint RIGHT_DOWN = 0x8, RIGHT_UP = 0x10;
    const uint KEY_UP = 0x2, KEY_UNICODE = 0x4;
    const ushort VK_CONTROL = 0x11, VK_A = 0x41, VK_RETURN = 0x0D, VK_ESCAPE = 0x1B;
    const int MaxChars = 200;

    static INPUT Key(ushort vk, ushort scan, uint flags) {
        INPUT i = new INPUT();
        i.type = TYPE_KEYBOARD;
        i.u.ki.wVk = vk; i.u.ki.wScan = scan; i.u.ki.dwFlags = flags;
        return i;
    }
    static bool Send(INPUT[] a) { return SendInput((uint)a.Length, a, Marshal.SizeOf(typeof(INPUT))) == (uint)a.Length; }

    // A right button press and release where the pointer is now.
    public static bool RightClick() {
        INPUT[] a = new INPUT[2];
        a[0].type = TYPE_MOUSE; a[0].u.mi.dwFlags = RIGHT_DOWN;
        a[1].type = TYPE_MOUSE; a[1].u.mi.dwFlags = RIGHT_UP;
        return Send(a);
    }
    // The text as Unicode key events (no keyboard layout involved); at most MaxChars characters.
    public static bool TypeText(string text) {
        if (text == null || text.Length == 0 || text.Length > MaxChars) return false;
        INPUT[] a = new INPUT[text.Length * 2];
        for (int i = 0; i < text.Length; i++) {
            a[2 * i] = Key(0, text[i], KEY_UNICODE);
            a[2 * i + 1] = Key(0, text[i], KEY_UNICODE | KEY_UP);
        }
        return Send(a);
    }
    public static bool SelectAll() {
        INPUT[] a = new INPUT[4];
        a[0] = Key(VK_CONTROL, 0, 0);
        a[1] = Key(VK_A, 0, 0);
        a[2] = Key(VK_A, 0, KEY_UP);
        a[3] = Key(VK_CONTROL, 0, KEY_UP);
        return Send(a);
    }
    public static bool Enter() {
        INPUT[] a = new INPUT[2];
        a[0] = Key(VK_RETURN, 0, 0);
        a[1] = Key(VK_RETURN, 0, KEY_UP);
        return Send(a);
    }
    public static bool Escape() {
        INPUT[] a = new INPUT[2];
        a[0] = Key(VK_ESCAPE, 0, 0);
        a[1] = Key(VK_ESCAPE, 0, KEY_UP);
        return Send(a);
    }
    // The window to the foreground: SetForegroundWindow; when another window still has it, the input queues are
    // attached for the one call (measured to work from a background process) and detached again. True when the
    // window has the foreground afterwards.
    public static bool BringToFront(IntPtr h) {
        SetForegroundWindow(h);
        if (GetForegroundWindow() == h) return true;
        IntPtr front = GetForegroundWindow();
        uint self = GetCurrentThreadId();
        uint other = 0;
        if (front != IntPtr.Zero) { uint pid; other = GetWindowThreadProcessId(front, out pid); }
        bool attached = false;
        if (other != 0 && other != self) attached = AttachThreadInput(self, other, true);
        BringWindowToTop(h);
        SetForegroundWindow(h);
        if (attached) AttachThreadInput(self, other, false);
        return GetForegroundWindow() == h;
    }
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
# A group action changes the list in a few seconds (2 to 4 measured): its work time (panel.rs DRIVER_GROUP_WORK_SECS is the same number).
$GroupWorkSeconds = 60
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
$script:RunAction = ''
$script:RunId = ''
$script:LastWait = ''
$script:Cancelled = $false
$script:SavedPlacement = $null
$script:FgStart = $null
$script:FgFailed = $false
# Group actions: where the driver last put the pointer, the last-input tick right after the driver's own last input,
# what a failed run left half done (for its message), and whether a menu item was just invoked (its menu is closing).
$script:Placed = $null
$script:OwnTick = $null
$script:GroupLeft = ''
$script:MenuInvoked = $false
# A group run: the field that may be typed into must belong to the header the run is about (see Get-NameField), and
# Restore-Screen decides the re-minimize by the driver's own last input, not by idle time its input has reset.
$script:FieldOwner = $null
$script:GroupRun = $false

function Stop-Panel([int]$Code, [string]$Reason) {
    throw (New-Object PanelExit($Code, $Reason))
}

function Add-Note([string]$Note) { $script:Notes += $Note }

# ---------------------------------------------------------------- cancel, and why a run is waiting

# -CancelFile names a file that exists, and the run has changed nothing yet (never while it is putting
# things back): the run stops. A run that has begun changing the panel finishes or fails as it would have.
function Test-CancelRequested {
    if (-not $CancelFile -or $script:Restoring) { return $false }
    if ($null -ne $script:St -and (Test-Touched)) { return $false }
    [bool](Test-Path -LiteralPath $CancelFile)
}

# Called in every loop that only waits and has changed nothing yet.
function Stop-IfCancelled {
    if (Test-CancelRequested) { $script:Cancelled = $true; Stop-Panel 7 'cancelled; nothing was changed' }
}

# Called when the run first changes the panel: <CancelFile with the extension .sending> tells af that a
# cancel can no longer stop the run.
function Set-Begun {
    if (-not $CancelFile) { return }
    try { [System.IO.File]::WriteAllText([System.IO.Path]::ChangeExtension($CancelFile, 'sending'), (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss'), $Utf8) }
    catch { Add-Note 'could not write the marker that says the run has begun changing the panel' }
}

# The one place that appends to claude-panel.log.
function Add-LogLine([string]$Line) {
    try {
        if (-not (Test-Path -LiteralPath $LogDir)) { New-Item -ItemType Directory -Force -Path $LogDir | Out-Null }
        [System.IO.File]::AppendAllText($LogFile, $Line + "`r`n", $Utf8)
    } catch { }
}

# Why the run is waiting, as its own log line, once per change of reason (never a title or text).
function Write-Waiting([string]$Reason) {
    if ($Reason -eq $script:LastWait) { return }
    $script:LastWait = $Reason
    $id = '-'
    if ($script:RunId -ne '') { $id = $script:RunId }
    Add-LogLine ('{0} {1} {2} waiting: {3}' -f (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss'), $script:RunAction, $id, ($Reason -replace '[\r\n]+', ' '))
}

# A wait ended: the next wait, even for the same reason, is logged again.
function Clear-Waiting { $script:LastWait = '' }

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
    if ($ActionName -eq 'list' -or $ActionName -eq 'groups') { $work = $ListWorkSeconds; $script:RestoreSecs = 15; $script:SettleSecs = 0 }
    if ($ActionName -like 'group-*') { $work = $GroupWorkSeconds; $script:RestoreSecs = 15; $script:SettleSecs = 0 }
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
        Archive = $false
    }
}

function Test-Touched { $s = $script:St; [bool]($s.Closed -or $s.CloseAsked -or $s.OpenCalled) }

# What the run changed in the panel (from what it did), then the state of the chat's tab now (from a fresh
# read, never from what it did).
function Get-StateText {
    $s = $script:St
    if ($null -eq $s) { return '' }
    if ($s.Archive) { return (Get-ArchiveStateText) }
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

# The same for an archive, in its own words: nothing changed; the tab was closed (and is open again or is
# not, from a fresh read); or the Archive button was invoked and the row is still listed, or is gone.
function Get-ArchiveStateText {
    $s = $script:St
    if (-not (Test-Touched) -and -not $s.Sent) { return "The panel was not changed: the chat's tab is as it was." }
    $done = @()
    if ($s.Closed) { $done += "closed the chat's tab" }
    elseif ($s.CloseAsked) { $done += "asked the bridge to close the chat's tab (it did not confirm)" }
    if ($s.Sent) { $done += "invoked the row's Archive button" }
    if ($s.Reopened) { $done += "opened the chat's tab again" }
    $text = 'The driver ' + ($done -join ', then ') + '.'
    if ($s.Sent) {
        $row = 'unknown'
        try { if ($null -eq (Find-RowElement $s.Id)) { $row = 'gone' } else { $row = 'listed' } } catch { $row = 'unknown' }
        if ($row -eq 'gone') { return $text + " The chat's row is no longer in the list: it is archived." }
        if ($row -eq 'listed') { return $text + " The chat's row is still in the list, so the chat is not known to be archived." }
        return $text + " The chat's row could not be read afterwards, so whether the chat is archived is unknown."
    }
    $text + ' ' + (Read-ChatState)
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
function Invoke-Guarded([scriptblock]$Body, [string]$NothingText = 'Nothing was sent.', [string]$DoneText = 'Send was pressed, so the text may have reached the chat.') {
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
        $lead = $reason.TrimEnd('.') + '.'
        if ($code -eq 7) { $reason = $lead + ' ' + $state }
        elseif ($code -ge 3 -and $code -le 5) { $reason = (@($lead, $NothingText, $state) | Where-Object { $_ -ne '' }) -join ' ' }
        elseif ($script:St.Sent) { $reason = (@($lead, $DoneText, $state) | Where-Object { $_ -ne '' }) -join ' ' }
        else { $reason = (@($lead, $NothingText, $state) | Where-Object { $_ -ne '' }) -join ' ' }
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
    if ($null -ne $s -and -not $script:Restoring -and -not $s.Opened) { $s.CloseAsked = $true; Set-Begun }
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
            Clear-Waiting
            return
        }
        if ((Get-Date) -ge $script:WaitUntil) { Stop-Panel 3 ('another driver run held the panel for the whole wait (' + (Read-LockHolder) + ')') }
        Stop-IfCancelled
        Write-Waiting ('another driver run holds the panel (' + (Read-LockHolder) + ')')
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
        Stop-IfCancelled
        if ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000)) { Clear-Waiting; return }
        if ((Get-Date) -ge (Get-WaitUntil)) { Stop-Panel 3 "the operator kept using the computer until the deadline ($What)" }
        Write-Waiting "the operator is using the computer (needed to $What)"
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

# Guards 1 and 2 read once, with no waiting: '' when he is idle and the chat's records are idle, else why not (a record
# from outside the VS Code panel is exit 5).
function Get-GuardWhy([string]$Id) {
    if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { return 'the operator is using the computer' }
    if ($Id -ne '') {
        $rs = Get-RecordState $Id
        if ($rs.Terminal -ne '') { Stop-Panel 5 $rs.Terminal }
        return $rs.Busy
    }
    ''
}

# Waits for the guards before the screen is touched, up to -WaitMinutes. It holds no lock while it waits.
function Wait-Guards([string]$Id) {
    while ($true) {
        Stop-IfCancelled
        $why = Get-GuardWhy $Id
        if ($why -eq '') { Clear-Waiting; return }
        if ((Get-Date) -ge $script:WaitUntil) { Stop-Panel 3 "gave up after $WaitMinutes minutes: $why" }
        Write-Waiting $why
        Start-Sleep -Seconds 2
    }
}

# Every action's way in: wait for the guards WITHOUT the panel lock (a run that waits for him must not make every other
# chat's panel command wait), then take the lock (waiting for it within the same budget, honouring a cancel), then check the
# same guards once more with no waiting. If one fails (he came back, the chat became busy) the lock is let go and the run
# goes back to waiting. Only when the lock and the guards hold together does the run change anything. A run that already
# holds the lock when it comes in (the selftest's later steps) keeps it.
function Enter-Run([string]$Id) {
    $held = ($null -ne $script:Lock)
    while ($true) {
        Wait-Guards $Id
        Enter-Lock $script:RunAction $script:RunId
        $why = Get-GuardWhy $Id
        if ($why -eq '') { return }
        if (-not $held) { Exit-Lock }
        Write-Waiting ('a guard failed once the panel lock was taken: ' + $why)
        # Never round again at once: a guard that flips on every read must not spin the loop.
        Stop-IfCancelled
        if ((Get-Date) -ge $script:WaitUntil) { Stop-Panel 3 "gave up after $WaitMinutes minutes: $why" }
        Start-Sleep -Seconds 2
    }
}

# ---------------------------------------------------------------- the foreground at the end

# His foreground window and the last-input tick, recorded once, when the screen work begins.
function Start-ForegroundRecord {
    $script:FgStart = [pscustomobject]@{ Prev = [PanelNative]::Foreground(); Tick = [PanelNative]::InputTick() }
}

# The one check at the end of a run (Restore-Screen, before the minimize decision). When VS Code has the foreground, a
# different window had it when the screen work began, that window still exists and is visible, and the last-input tick is
# the record's (he did nothing), the foreground goes back to that recorded window with one call, checked within 500 ms;
# if it did not work a note says VS Code was left in front. If the tick changed, nothing is moved. The target is only the
# recorded window, never VS Code and never a window chosen from the z-order.
function Restore-ForegroundAtEnd {
    $g = $script:FgStart
    if ($null -eq $g -or $script:Hwnd -eq [IntPtr]::Zero -or $script:FgFailed) { return }
    if ([PanelNative]::Foreground() -ne $script:Hwnd) { return }
    $prev = $g.Prev
    if ($prev -eq [IntPtr]::Zero -or $prev -eq $script:Hwnd) { return }
    if (-not [PanelNative]::IsWindow($prev) -or -not [PanelNative]::IsVisible($prev)) { return }
    if ([PanelNative]::InputTick() -ne $g.Tick) { return }
    [void][PanelNative]::GiveForegroundBack($prev)
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt 500) {
        if ([PanelNative]::Foreground() -eq $prev) { return }
        Start-Sleep -Milliseconds 50
    }
    if ([PanelNative]::Foreground() -ne $prev) {
        # The driver put VS Code there, not him: the re-minimize decision must not call it his choice.
        $script:FgFailed = $true
        Add-Note 'VS Code was left in front'
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

# VS Code goes directly behind a window, but only a window that is not VS Code, still exists and is not itself
# always-on-top (behind an always-on-top window VS Code would become always-on-top too). True when it was placed.
function Set-VsCodeBehind($Behind) {
    if ($Behind -eq [IntPtr]::Zero -or $Behind -eq $script:Hwnd) { return $false }
    if (-not [PanelNative]::IsWindow($Behind)) { return $false }
    if ([PanelNative]::IsTopmost($Behind)) { return $false }
    [void][PanelNative]::PlaceBehind($script:Hwnd, $Behind)
    $true
}

# The last check of any z-order change: VS Code's extended style must not carry the always-on-top bit. When it
# does, it is cleared once more, and a note says when it stays set.
function Confirm-VsCodeNotTopmost {
    if ([PanelNative]::IsTopmost($script:Hwnd)) {
        [void][PanelNative]::ClearTopmost($script:Hwnd)
        if ([PanelNative]::IsTopmost($script:Hwnd)) { Add-Note 'VS Code is still in the always-on-top band; the driver could not clear it' }
    }
}

# A minimized window's tree is stale, so it is restored, and the run remembers to minimize it again at
# the end. It does not take the foreground and does not reset the idle time (measured): the window goes to
# the bottom of the stack while still minimized, is shown without activation (a window minimized from maximized
# comes back maximized), and is then put directly behind the window that has the foreground when that window
# is not always-on-top (else it stays at the bottom), so VS Code does not appear over his work. Used by
# Ensure-Screen and by the close retry (the window may be minimized again after Ensure-Screen ran).
function Restore-Minimized {
    Wait-Idle 'restore the minimized VS Code window'
    $front = [PanelNative]::Foreground()
    # The placement as found is kept to minimize it again the same way (showCmd 7, with the maximize flag).
    if ($null -eq $script:SavedPlacement) { $script:SavedPlacement = [PanelNative]::GetPlacement($script:Hwnd) }
    [void][PanelNative]::PlaceBottom($script:Hwnd)
    [void][PanelNative]::ShowNoActivate($script:Hwnd)
    [void](Set-VsCodeBehind $front)
    Confirm-VsCodeNotTopmost
    $script:WasMinimized = $true
    Start-Sleep -Milliseconds 2500
    if ([PanelNative]::Foreground() -ne $front) { Add-Note 'the foreground window changed while the minimized VS Code window was restored' }
    Wait-Tree
}

# Before the first read or step on the screen: the window is made visible (a minimized window's tree is
# stale), and his active chat is noted so it can be restored. Runs once per run.
function Ensure-Screen {
    if ($script:ScreenBegun) { return }
    Open-Window
    Start-ForegroundRecord
    if ([PanelNative]::IsIconic($script:Hwnd)) { Restore-Minimized } else { Wait-Tree }
    $script:ScreenBegun = $true
    $script:PrevActive = @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
}

# True when another window covers the centre of VS Code's own rectangle (so VS Code is not what he sees there).
# An unreadable or empty rectangle counts as not covered.
function Test-VsCodeCovered {
    try {
        $r = $script:Win.Current.BoundingRectangle
        foreach ($v in @($r.X, $r.Y, $r.Width, $r.Height)) { if ([double]::IsNaN($v) -or [double]::IsInfinity($v)) { return $false } }
        if ($r.Width -le 0 -or $r.Height -le 0) { return $false }
        $root = [PanelNative]::RootAt([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
        return [bool]($root -ne $script:Hwnd)
    } catch { return $false }
}

# Puts his active chat back, and the window as it was. Never fights him: what he can see is only his
# previously active chat, so the idle wait is needed only when that chat has to be activated again (and a
# cancelled run does not wait at all); if he is at the machine it waits a little (within the run's deadline)
# and then says it did not restore. The window is minimized again only when this run restored it, VS Code is not
# his foreground window (with the no-focus restore it is only there because he brought it there: it is left open)
# and either he is idle (at or above the threshold) or another window covers VS Code's centre; else it is visible
# on his screen and he is at the computer, so it is left open, with a note.
function Restore-Screen {
    if (-not $script:ScreenBegun) { return }
    $script:ScreenBegun = $false
    try {
        $want = ''
        $needRow = $false
        if ($script:PrevActive.Count -eq 1) {
            $want = $script:PrevActive[0]
            $now = @(Get-Chats $script:Win | Where-Object { $_.Active } | ForEach-Object { $_.Id })
            $needRow = -not ($now.Count -eq 1 -and $now[0] -eq $want)
        } else { Add-Note 'he had no chat row active, so none was restored' }
        if ($needRow) {
            $limit = Get-Date
            if (-not $script:Cancelled) {
                $limit = (Get-Date).AddSeconds(60)
                if ($limit -gt $script:RestoreUntil) { $limit = $script:RestoreUntil }
            }
            while ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000) -and (Get-Date) -lt $limit) { Start-Sleep -Seconds 2 }
            if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { Add-Note 'his view was not restored: the operator is at the machine' }
            else {
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
        }
    } catch {
        Add-Note ('could not put his previous chat back: ' + $_.Exception.Message)
    } finally {
        # The one foreground check of the run comes first, so a foreground the driver's own steps caused is not mistaken
        # for his choice in the decision below.
        try { Restore-ForegroundAtEnd } catch { Add-Note ('could not give the foreground back: ' + $_.Exception.Message) }
        if ($script:WasMinimized) {
            try {
                $idle = ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000))
                # A group run's own input has reset the idle time: there he counts as idle when nothing came after the driver's own
                # last input (a run that sent none, OwnTick unset, is judged by the idle time as every other action is).
                if ($script:GroupRun -and $null -ne $script:OwnTick) { $idle = ([PanelNative]::InputTick() -eq $script:OwnTick) }
                if ([PanelNative]::Foreground() -eq $script:Hwnd) {
                    # Left open either way; he only "has it in front" when the driver's own hand-back was not tried and failed.
                    if (-not $script:FgFailed) { Add-Note 'VS Code was left open: he has it in front' }
                } elseif ($idle -or (Test-VsCodeCovered)) {
                    if ($null -ne $script:SavedPlacement) { [void][PanelNative]::MinimizeWithPlacement($script:Hwnd, $script:SavedPlacement) }
                    else { [void][PanelNative]::MinimizeNoActivate($script:Hwnd) }
                } else { Add-Note 'VS Code was left open: it is visible on his screen and he is at the computer' }
            } catch { Add-Note ('could not minimize the window again: ' + $_.Exception.Message) }
            $script:WasMinimized = $false
            $script:SavedPlacement = $null
        }
    }
}

# ---------------------------------------------------------------- reading the panel

# The sessions list in document order, group headers and chat rows together (they are sibling buttons under the
# Sessions group): a header is Kind 'header' with Name (the group's name, from its first Text child), Count and
# Editing (its name field is open); a row is Kind 'row' with Id, Name (the chat title), Status, Active, Element and
# Group (the name of the header it follows, '' when none). Count is -1 when the header's count cannot be read.
function Get-ListItems($win) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Button)
    $tw = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $out = @()
    $group = ''
    foreach ($b in $win.FindAll($TS::Descendants, $cond)) {
        $id = ''
        $cls = ''
        try { $id = [string]$b.Current.AutomationId; $cls = [string]$b.Current.ClassName } catch { continue }
        if ($id -ne '' -and $id.StartsWith($RowPrefix, [StringComparison]::Ordinal)) {
            $status = 'none'
            $ch = $tw.GetFirstChild($b)
            while ($null -ne $ch) {
                $c = $ch.Current.ClassName
                if ($c -like '*statusDot*') {
                    if ($c -like '*Running*') { $status = 'running' } elseif ($c -like '*Idle*') { $status = 'idle' }
                    break
                }
                $ch = $tw.GetNextSibling($ch)
            }
            # The row's name is the chat title and an age (now, 12h, 4d).
            $name = [regex]::Replace($b.Current.Name, '\s+(now|\d+\s?[smhdwy])$', '')
            $out += [pscustomobject]@{
                Kind = 'row'; Id = $id.Substring($RowPrefix.Length); Name = $name; Status = $status; Count = 0
                Active = [bool]($b.Current.ClassName -match '(^|\s)active_'); Group = $group; Editing = $false; Element = $b
            }
        } elseif ($cls -match '(^|\s)groupHeader_' -and (Test-UnderSessions $b)) {
            $h = Get-HeaderInfo $b $tw
            $group = $h.Name
            $out += [pscustomobject]@{ Kind = 'header'; Id = ''; Name = $h.Name; Status = ''; Count = $h.Count; Active = $false; Group = ''; Editing = $h.Editing; Element = $b }
        }
    }
    $out
}

# An element's parent in the control view, or $null.
function Get-ParentOf($El) { try { [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($El) } catch { $null } }

# True when the element lies under the Group named Sessions (the sessions list). Only headers are asked, a handful per read,
# so the rows that send and archive poll cost nothing more.
function Test-UnderSessions($El) {
    $p = Get-ParentOf $El
    for ($i = 0; $i -lt 6 -and $null -ne $p; $i++) {
        try { if ($p.Current.ControlType -eq $CT::Group -and [string]$p.Current.Name -ceq 'Sessions') { return $true } } catch { return $false }
        $p = Get-ParentOf $p
    }
    $false
}

# A group header's name and count, from its children (never from the button's own name): the first Text child is the
# name and the second the count. While its name field is open (a child of class groupName*) the name is the field's
# text and the count is the last Text child.
function Get-HeaderInfo($Button, $Walker) {
    $texts = @()
    $field = $null
    $ch = $Walker.GetFirstChild($Button)
    while ($null -ne $ch) {
        try {
            if ($ch.Current.ControlType -eq $CT::Text) { $texts += [string]$ch.Current.Name }
            elseif ([string]$ch.Current.ClassName -match '(^|\s)groupName') { $field = $ch }
        } catch { }
        $ch = $Walker.GetNextSibling($ch)
    }
    $name = ''
    $count = -1
    $n = 0
    if ($null -ne $field) {
        $name = Get-FieldText $field
        if ($texts.Count -ge 1 -and [int]::TryParse($texts[$texts.Count - 1], [ref]$n)) { $count = $n }
    } else {
        if ($texts.Count -ge 1) { $name = $texts[0] }
        if ($texts.Count -ge 2 -and [int]::TryParse($texts[1], [ref]$n)) { $count = $n }
    }
    [pscustomobject]@{ Name = $name; Count = $count; Editing = [bool]($null -ne $field) }
}

# The text a name field shows: its Text children's names, joined.
function Get-FieldText($Field) {
    $tw = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $s = ''
    try {
        $ch = $tw.GetFirstChild($Field)
        while ($null -ne $ch) {
            if ($ch.Current.ControlType -eq $CT::Text) { $s += [string]$ch.Current.Name }
            $ch = $tw.GetNextSibling($ch)
        }
    } catch { return $null }
    $s
}

# The chat rows (with the name of the group each follows).
function Get-Chats($win) { @(Get-ListItems $win | Where-Object { $_.Kind -eq 'row' }) }

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
    if ([PanelNative]::IsIconic($script:Hwnd)) { return [pscustomobject]@{ Code = 3; Why = 'the VS Code window is minimized'; Retry = $true; Label = ''; Minimized = $true; Reveal = $false } }
    $claude = @(Get-ClaudeTabs)
    $label = $ExpectLabel
    if ($label -eq '') {
        $act = @($claude | Where-Object { $_.isActive })
        if ($act.Count -ne 1) { return [pscustomobject]@{ Code = 5; Why = "$($act.Count) Claude tabs are active, so the chat's tab cannot be told"; Retry = $true; Label = ''; Reveal = $true } }
        $label = [string]$act[0].label
    }
    $same = @($claude | Where-Object { $_.label -ceq $label })
    if ($same.Count -lt 1) { return [pscustomobject]@{ Code = 5; Why = "no Claude tab carries the chat's label"; Retry = $true; Label = '' } }
    if ($same.Count -gt 1 -and -not $FirstOfMany) { return [pscustomobject]@{ Code = 5; Why = "$($same.Count) Claude tabs carry the chat's label (labels are cut at 25 characters)"; Retry = $false; Label = '' } }
    if (-not $same[0].isActive) { return [pscustomobject]@{ Code = 5; Why = "the chat's tab is not the active tab"; Retry = $true; Label = ''; Reveal = $true } }
    $rows = @(Get-Chats $script:Win)
    $act = @($rows | Where-Object { $_.Active })
    if ($RowId -ne '') {
        if (-not ($act.Count -eq 1 -and $act[0].Id -eq $RowId)) { return [pscustomobject]@{ Code = 5; Why = "the chat's row is not the only active row"; Retry = $true; Label = ''; Reveal = $true } }
        if ($act[0].Status -ne 'idle') { return [pscustomobject]@{ Code = 3; Why = "the chat's row says $($act[0].Status)"; Retry = $true; Label = '' } }
    } elseif ($act.Count -ne 0) { return [pscustomobject]@{ Code = 5; Why = 'a chat row is active, so the active tab is not an untitled chat'; Retry = $true; Label = ''; Reveal = $true } }
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
# runs again after 2 seconds. The chat is revealed (when $RevealId is given) on the first try, and on a later
# try only when the last failure was that its tab or its row was not the active one. When the check says the
# window is minimized, the window is restored again (under Wait-Idle) and minimized again at the end of the
# run. Why the run is waiting is logged once per change of reason.
function Close-Checked([string]$RowId, [string]$RecId, [string]$Mode, [string]$ExpectLabel, [string]$RevealId = '', [bool]$FirstOfMany = $false, [int]$OwnPid = 0) {
    $reveal = $true
    while ($true) {
        Wait-Idle 'close a chat tab'
        if ($RevealId -ne '' -and $reveal) { Show-Chat $RevealId }
        $c = Test-SafeToClose $RowId $RecId $Mode $ExpectLabel $FirstOfMany $OwnPid
        if ($c.Code -eq 0) {
            # Nothing waits between the check above and this close.
            Close-TabByLabel $c.Label $c.Count
            $script:St.TabOpen = $false
            if (-not $script:Restoring -and -not $script:St.Opened) { $script:St.Closed = $true }
            break
        }
        if (-not $c.Retry -or (Get-Date) -ge (Get-WaitUntil)) { Stop-Panel $c.Code ('the tab was not closed: ' + $c.Why) }
        Write-Waiting ('before closing the chat tab: ' + $c.Why)
        $reveal = [bool]$c.Reveal
        if ($c.Minimized) { Restore-Minimized } else { Start-Sleep -Seconds 2 }
    }
    Clear-Waiting
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
    Set-Begun
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
        Stop-IfCancelled
        if ((Get-Date) -ge (Get-WaitUntil)) { Stop-Panel 3 'the chat is running (its row says so)' }
        Write-Waiting "the chat's row says it is running"
        Start-Sleep -Seconds 2
        $chat = Get-Chat $Id
    }
    Clear-Waiting
    $chat
}

# 'list' (the chats) and 'groups' (the list's groups): read only, the same way in.
function Do-List([string]$What = 'list') {
    Open-Window
    if ([PanelNative]::IsIconic($script:Hwnd)) {
        # Restoring the window changes his screen, so this list waits its turn like any other run: for his idle time
        # without the lock, then the lock, then one more look at his idle time (back to waiting if he came back).
        while ($true) {
            Wait-Idle 'restore the minimized VS Code window'
            Enter-Lock $What ''
            if ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000)) { break }
            Exit-Lock
            Write-Waiting 'the operator came back once the panel lock was taken'
        }
        Ensure-Screen
    } else { Wait-Tree }
    if ($What -eq 'groups') {
        $groups = @(Get-ListItems $script:Win | Where-Object { $_.Kind -eq 'header' } | ForEach-Object { [pscustomobject]@{ name = $_.Name; count = $_.Count; fixed = (Test-GroupFixed $_.Name) } })
        return (ConvertTo-Json -InputObject $groups -Compress)
    }
    $rows = @(Get-Chats $script:Win | ForEach-Object { [pscustomobject]@{ session_id = $_.Id; name = $_.Name; status = $_.Status; active = $_.Active; group = $_.Group } })
    ConvertTo-Json -InputObject $rows -Compress
}

function Do-Send([string]$Id, [string]$Text) {
    New-RunState $Id $Text
    $s = $script:St
    $done = Invoke-Guarded {
        Test-Bridge
        Enter-Run $Id
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

function Do-New([string]$Text, [string]$ModelName) {
    if ($ModelName -eq '') { Stop-Panel 1 '-Model is needed: a new chat would inherit the default model, which is the expensive one' }
    New-RunState '' $Text
    $s = $script:St
    $s.RowId = ''
    $s.OwnLabel = $UntitledLabel
    $s.Label = $UntitledLabel
    $newId = Invoke-Guarded {
        Test-Bridge
        Enter-Run ''
        Ensure-Screen
        Wait-Idle 'open a new chat'
        # Nothing waits from here to the open: the new tab must be the only untitled one.
        $claude = @(Get-ClaudeTabs)
        $blank = @($claude | Where-Object { $_.label -ceq $UntitledLabel })
        if ($blank.Count -ne 0) { Stop-Panel 5 "$($blank.Count) untitled '$UntitledLabel' tab(s) are already open, so the new chat's tab could not be told apart; run close-untitled or close them, then start the chat" }
        $recBefore = @(Get-Records | ForEach-Object { [string]$_.sessionId })
        $bargs = @{ prompt = $Text }
        $s.OpenCalled = $true
        Set-Begun
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
        Enter-Run $Id
        Ensure-Screen
        $chat = Wait-RowIdle $Id
        $rs = Get-RecordState $Id
        $s.HadTab = ($rs.Count -gt 0)
        $s.TabOpen = $s.HadTab
        Set-TabLabel (Get-TabLabel $chat.Name)
        Wait-Idle 'show the chat'
        $n0 = @(Get-ClaudeTabs).Count
        if (-not $s.HadTab) { $s.OpenCalled = $true; Set-Begun }
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

# ---------------------------------------------------------------- archive

# A session row's element, by its AutomationId (null when the list has no such row).
function Find-RowElement([string]$Id) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, ($RowPrefix + $Id))
    $script:Win.FindFirst($TS::Descendants, $cond)
}

# The centre of a row's bounding rectangle, or $null when the rectangle is empty, the row is off screen or
# its centre lies outside the VS Code window.
function Get-RowCenter($Row) {
    if ($Row.Current.IsOffscreen) { return $null }
    $r = $Row.Current.BoundingRectangle
    foreach ($v in @($r.X, $r.Y, $r.Width, $r.Height)) {
        if ([double]::IsNaN($v) -or [double]::IsInfinity($v)) { return $null }
    }
    if ($r.Width -le 0 -or $r.Height -le 0) { return $null }
    $cx = [int]($r.X + $r.Width / 2)
    $cy = [int]($r.Y + $r.Height / 2)
    $w = $script:Win.Current.BoundingRectangle
    if ($cx -lt $w.X -or $cx -gt ($w.X + $w.Width) -or $cy -lt $w.Y -or $cy -gt ($w.Y + $w.Height)) { return $null }
    [pscustomobject]@{ X = $cx; Y = $cy }
}

# The 'Archive session' button among the target row's own children (it exists only while the pointer rests
# on the row), or $null; a button whose parent is another row does not count.
function Get-ArchiveButton([string]$Id) {
    $row = Find-RowElement $Id
    if ($null -eq $row) { return $null }
    $c1 = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, 'Archive session')
    $c2 = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Button)
    $btn = $row.FindFirst($TS::Children, (New-Object System.Windows.Automation.AndCondition($c1, $c2)))
    if ($null -eq $btn) { return $null }
    $parent = [System.Windows.Automation.TreeWalker]::RawViewWalker.GetParent($btn)
    if ($null -eq $parent -or [string]$parent.Current.AutomationId -ne ($RowPrefix + $Id)) { return $null }
    $btn
}

function Test-PointerAt($Spot) {
    $p = [PanelNative]::PointerPos()
    [bool]($null -ne $p -and $p[0] -eq $Spot.X -and $p[1] -eq $Spot.Y)
}

# The pointer goes back where it was, but only while it is still where the driver put it: if he moved it,
# it is his and is left alone.
function Restore-Pointer($Old, $Placed) {
    if ($null -eq $Old -or $null -eq $Placed) { return }
    if (Test-PointerAt $Placed) { [void][PanelNative]::MovePointer($Old[0], $Old[1]) }
    else { Add-Note 'the pointer was moved by him after the driver put it on the row, so the driver left it where he put it' }
}

# True when the owner chain of a window (followed upward, bounded) reaches the owner.
function Test-OwnedBy($Window, $Owner) {
    $o = [PanelNative]::Owner($Window)
    for ($i = 0; $i -lt 20 -and $o -ne [IntPtr]::Zero; $i++) {
        if ($o -eq $Owner) { return $true }
        $o = [PanelNative]::Owner($o)
    }
    $false
}

# The first REAL window above VS Code in the z-order: walk upward with GW_HWNDPREV (at most 400 steps) and skip a window
# that is not visible, that is cloaked, or whose owner chain reaches the VS Code window. (Windows keeps a window's owned
# windows directly above it, so a bare 'window above' is usually VS Code's own hidden window, which rises and drops with
# it. A visible window of VS Code's own process is not skipped: it can be his other project window, and VS Code goes back
# behind it.) Known is false when the walk ran out of steps; Hwnd is zero when nothing real is above (VS Code is the top
# real window); Top says whether that window is always-on-top.
function Find-RealWindowAbove {
    $w = $script:Hwnd
    for ($i = 0; $i -lt 400; $i++) {
        $w = [PanelNative]::Prev($w)
        if ($w -eq [IntPtr]::Zero) { return [pscustomobject]@{ Hwnd = [IntPtr]::Zero; Top = $false; Known = $true } }
        if (-not [PanelNative]::IsVisible($w)) { continue }
        if ([PanelNative]::IsCloaked($w)) { continue }
        if (Test-OwnedBy $w $script:Hwnd) { continue }
        return [pscustomobject]@{ Hwnd = $w; Top = [PanelNative]::IsTopmost($w); Known = $true }
    }
    [pscustomobject]@{ Hwnd = [IntPtr]::Zero; Top = $false; Known = $false }
}

# Undoes the raise and puts VS Code exactly where it was in the z-order. Before the raise the first real window above VS
# Code was recorded (Above, and whether it is always-on-top, AboveTop). Not topmost first. Then: when that window still
# exists and is not always-on-top, directly behind it (where VS Code was). When it was always-on-top, or there was none,
# VS Code was the top real window and the first step has just put it there. When it is gone, or was not found: directly
# behind his foreground window if that is not VS Code and not always-on-top, else at the back, with a note. Never behind an
# always-on-top window. Then the model is checked against the screen: when the window that covered the row still exists, is
# visible and is not always-on-top (VS Code cannot be above one: it simply moved), and VS Code is not his foreground window,
# the row's centre must no longer belong to VS Code; if it does, VS Code is placed once more behind that window, else behind
# his foreground window (never at the back: the foreground window would end up behind everything), and if it is still in
# front a note says so. At the end the always-on-top bit is read and cleared once more if it is set.
function Undo-Raise($Front) {
    if (-not $Front.Raised) { return }
    [void][PanelNative]::ClearTopmost($script:Hwnd)
    $lost = $false
    if (-not $Front.AboveKnown) { $lost = $true }
    elseif ($Front.Above -eq [IntPtr]::Zero -or $Front.AboveTop) { }
    elseif ([PanelNative]::IsWindow($Front.Above)) { [void](Set-VsCodeBehind $Front.Above) }
    else { $lost = $true }
    if ($lost -and -not (Set-VsCodeBehind ([PanelNative]::Foreground()))) {
        [void][PanelNative]::PlaceBottom($script:Hwnd)
        Add-Note 'the window that was above VS Code is gone or was not found, so VS Code was put at the back'
    }
    Confirm-VsCodeNotTopmost
    $cover = $Front.Cover
    $spot = $Front.Spot
    if ($null -ne $spot -and $cover -ne [IntPtr]::Zero -and $cover -ne $script:Hwnd -and [PanelNative]::IsWindow($cover) -and [PanelNative]::IsVisible($cover) -and -not [PanelNative]::IsTopmost($cover) -and [PanelNative]::Foreground() -ne $script:Hwnd) {
        if ([PanelNative]::RootAt($spot.X, $spot.Y) -eq $script:Hwnd) {
            [void]((Set-VsCodeBehind $cover) -or (Set-VsCodeBehind ([PanelNative]::Foreground())))
            Confirm-VsCodeNotTopmost
            if ([PanelNative]::RootAt($spot.X, $spot.Y) -eq $script:Hwnd) { Add-Note 'VS Code is still in front of the window that covered the row' }
        }
    }
    $Front.Raised = $false
}

# The shell says a full-screen window, a Direct3D full-screen application or a presentation is in front.
function Test-FullScreenFront {
    $q = [PanelNative]::NotificationState()
    [bool]($q -eq 2 -or $q -eq 3 -or $q -eq 4)
}

# '' while he has not come back, else why he has: the pointer is not exactly where the driver put it, or the
# idle time fell below what it was before the move (he used the keyboard or the pointer since).
function Get-OperatorBack($Placed, [double]$IdleBefore) {
    if (-not (Test-PointerAt $Placed)) { return 'the pointer is no longer where the driver put it' }
    if ([PanelNative]::IdleMs() -lt $IdleBefore) { return 'the idle time fell, so he used the computer after the driver moved the pointer' }
    ''
}

# Read right before the Invoke, after every wait: '' when the chat is still one that may be archived, else why
# not. The chat's tab was closed (or it had none), so it has no live record at all; its row does not say running
# and is not the active row. A chat that started running, or that he opened, while the driver waited is not archived.
function Get-ArchiveBlock([string]$Id) {
    $rs = Get-RecordState $Id
    if ($rs.Count -gt 0) { return 'the chat has a live session record again, so it started or he opened it while the driver waited' }
    $rows = @(Get-Chats $script:Win | Where-Object { $_.Id -eq $Id })
    if ($rows.Count -ne 1) { return "the chat's row could not be read" }
    if ($rows[0].Status -eq 'running') { return "the chat's row says it is running" }
    if ($rows[0].Active) { return "the chat's row is the active row" }
    ''
}

# After the driver closed the chat's tab: its row stops being the active row, within a few seconds.
function Wait-RowNotActive([string]$Id) {
    $until = (Get-Date).AddSeconds(5)
    while (@(Get-ActiveIds) -contains $Id) {
        if ((Get-Date) -ge $until) { Stop-Panel 3 "the chat's row is still the active row after its tab was closed; nothing was archived" }
        Start-Sleep -Milliseconds 250
    }
}

# The part of an archive that shows on his screen. The row is scrolled into view; then, in one loop, everything
# that waits happens, before anything on his screen changes: he is idle, no full-screen window is in front of a
# covered row, and he is still idle at the moment of the raise (VS Code is raised above a covering window without
# keyboard focus, with nothing between that last check and the raise). After the raise nothing waits except the
# 3 second button poll and the 5 second row-gone confirm, and anything not as required is exit 3 or 5 at once.
# The pointer rests on the row until the row's Archive button exists, and that button is invoked. In every
# outcome the pointer goes back (if it is still where the driver put it) and the raise is undone. Nothing is
# clicked or typed.
function Invoke-ArchiveStage([string]$Id) {
    $row = Find-RowElement $Id
    if ($null -eq $row) { Stop-Panel 5 "the chat's row left the list while the driver was getting ready; nothing was archived" }
    $sp = $null
    if ($row.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$sp)) { try { $sp.ScrollIntoView() } catch { } }
    Start-Sleep -Milliseconds 300
    $front = [pscustomobject]@{ Raised = $false; Cover = [IntPtr]::Zero; Spot = $null; Above = [IntPtr]::Zero; AboveTop = $false; AboveKnown = $true }
    $old = $null
    $placed = $null
    $spot = $null
    try {
        $fsUntil = $null
        $fsLogged = $false
        while ($true) {
            Wait-Idle 'put the pointer on the chat row'
            $row = Find-RowElement $Id
            if ($null -eq $row) { Stop-Panel 5 "the chat's row left the list while the driver was getting ready; nothing was archived" }
            $spot = Get-RowCenter $row
            if ($null -eq $spot) { Stop-Panel 5 "the chat's row has no usable place on the screen (an empty rectangle, off screen, or outside the VS Code window); nothing was archived" }
            $cover = [PanelNative]::RootAt($spot.X, $spot.Y)
            if ($cover -eq $script:Hwnd) { break }
            # Covered: the raise shows on his screen. A full-screen window in front (idle time cannot see a video)
            # is waited out, within the wait budget or 60 seconds of its own, then idle is waited for again.
            if (Test-FullScreenFront) {
                if ($null -eq $fsUntil) {
                    $fsUntil = $script:WaitUntil
                    $own = (Get-Date).AddSeconds(60)
                    if ($own -gt $fsUntil) { $fsUntil = $own }
                }
                Stop-IfCancelled
                if ((Get-Date) -ge $fsUntil) { Stop-Panel 3 'a full-screen window stayed in front of the row until the deadline; VS Code was not raised and the pointer was not moved' }
                # The idle wait clears the last waiting reason each time round, so this one is logged once here.
                if (-not $fsLogged) { Write-Waiting 'a full-screen window is in front'; $fsLogged = $true }
                Start-Sleep -Seconds 2
                continue
            }
            $fsLogged = $false
            Clear-Waiting
            # Where VS Code is in the window order, read now, so the raise can be undone to exactly there.
            $above = Find-RealWindowAbove
            $front.Above = $above.Hwnd
            $front.AboveTop = $above.Top
            $front.AboveKnown = $above.Known
            $front.Spot = $spot
            # He may have come back since the idle wait: wait for him again rather than raise over his work.
            if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { continue }
            # Nothing waits from here to the raise, or after it.
            $front.Cover = $cover
            [void][PanelNative]::RaiseNoActivate($script:Hwnd)
            $front.Raised = $true
            break
        }
        if ([PanelNative]::RootAt($spot.X, $spot.Y) -ne $script:Hwnd) { Stop-Panel 5 'the row is covered by another window that VS Code could not be raised above without taking the keyboard focus; the pointer was not moved and nothing was archived' }
        if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { Stop-Panel 3 'the operator came back (the idle time is below the idle threshold); the pointer was not moved and nothing was archived' }
        $idleBefore = [PanelNative]::IdleMs()
        $old = [PanelNative]::PointerPos()
        if ($null -eq $old) { Stop-Panel 5 'the pointer position could not be read; the pointer was not moved and nothing was archived' }
        if (-not [PanelNative]::MovePointer($spot.X, $spot.Y)) { Stop-Panel 5 'the pointer could not be moved onto the row; nothing was archived' }
        # Placed where the driver asked, whatever a read-back says, so it is put back whatever happens next.
        $placed = [pscustomobject]@{ X = $spot.X; Y = $spot.Y }
        $at = [PanelNative]::PointerPos()
        if ($null -ne $at) { $placed = [pscustomobject]@{ X = $at[0]; Y = $at[1] } }
        $btn = $null
        $until = (Get-Date).AddSeconds(3)
        while ($null -eq $btn -and (Get-Date) -lt $until) {
            Start-Sleep -Milliseconds 150
            $btn = Get-ArchiveButton $Id
        }
        if ($null -eq $btn) { Stop-Panel 5 'no archive button appeared under the row within 3 seconds of the pointer resting on it; nothing was archived' }
        $inv = $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        # Right before the Invoke, with nothing waiting from here: the chat is still one that may be archived, and
        # he has not come back.
        $block = Get-ArchiveBlock $Id
        if ($block -ne '') { Stop-Panel 3 "$block; nothing was archived" }
        $back = Get-OperatorBack $placed $idleBefore
        if ($back -ne '') { Stop-Panel 3 "the operator came back ($back); nothing was archived" }
        $script:St.Sent = $true
        $inv.Invoke()
        $gone = $false
        $until = (Get-Date).AddSeconds(5)
        while ((Get-Date) -lt $until) {
            if ($null -eq (Find-RowElement $Id)) { $gone = $true; break }
            Start-Sleep -Milliseconds 200
        }
        if (-not $gone) { Stop-Panel 1 "the Archive button was invoked but the chat's row is still in the list after 5 seconds" }
    } finally {
        # Each step in its own try, so one that throws cannot skip the next.
        try { Restore-Pointer $old $placed } catch { Add-Note ('could not put the pointer back: ' + $_.Exception.Message) }
        try { Undo-Raise $front } catch { Add-Note ('could not undo the raise of VS Code: ' + $_.Exception.Message) }
    }
}

function Do-Archive([string]$Id) {
    New-RunState $Id ''
    $s = $script:St
    $s.Archive = $true
    Invoke-Guarded {
        Enter-Run $Id
        Ensure-Screen
        if (@(Find-Named $script:Win 'New session' $CT::Button).Count -lt 1) { Stop-Panel 2 'the sessions list is not readable in the VS Code window (no New session button); nothing was changed' }
        if ($null -eq (Find-RowElement $Id)) {
            return 'no row for this chat is in the list as shown (already archived, never listed, or inside a collapsed group); nothing was changed'
        }
        # A chat he has in front is never archived, and there is no waiting for him to switch away: the run stops at once
        # (the lock is let go by the normal exit) and the caller can try again later.
        if (@(Get-ActiveIds) -contains $Id) { Stop-Panel 3 'it is the chat he has in front; nothing was changed' }
        $chat = Wait-RowIdle $Id
        $rs = Get-RecordState $Id
        if ($rs.Terminal -ne '') { Stop-Panel 5 $rs.Terminal }
        if ($rs.Busy -ne '') { Stop-Panel 3 "the chat is busy: $($rs.Busy)" }
        $s.HadTab = ($rs.Count -gt 0)
        $s.TabOpen = $s.HadTab
        if ($s.HadTab) {
            # His open tab: only now is the bridge needed. Reveal it, prove it holds no draft, close it.
            Test-Bridge
            Set-TabLabel (Get-TabLabel $chat.Name)
            Close-Checked $Id $Id 'empty' (Get-TabLabel $chat.Name) $Id
            Wait-RowNotActive $Id
        }
        Invoke-ArchiveStage $Id
        if ($s.HadTab) { return "archived session $Id; its row left the list (its open tab was closed first)" }
        "archived session $Id; its row left the list"
    } 'Nothing was archived.' ''
}

# ---------------------------------------------------------------- group actions

# The decisions come first, as pure functions on the list as plain data: an array of items { Kind = 'header' or 'row';
# Name; Count; Id } in list order (a row belongs to the header before it). Each plan is { Act = 'do', 'nothing' or
# 'refuse'; Code (the exit code of a refusal); Text }.

# The panel's own headers, which are never one of his groups: Ungrouped and the archived section.
function Test-GroupFixed([string]$Name) {
    if ($Name -ceq 'Ungrouped') { return $true }
    $Name.StartsWith('Archived sessions', [StringComparison]::OrdinalIgnoreCase)
}

# '' when the name can be given to a group, else why not: trimmed, 1 to 60 characters, no control character or line
# break, and not a name the panel keeps for itself (compared without case).
function Test-GroupName([string]$Name) {
    if ($Name.Length -lt 1) { return 'it is empty' }
    if ($Name -cne $Name.Trim()) { return 'it has spaces at its start or end' }
    if ($Name.Length -gt 60) { return 'it is longer than 60 characters' }
    if ($Name -match '[\p{Cc}\p{Zl}\p{Zp}]') { return 'it holds a control character or a line break' }
    foreach ($kept in @('Ungrouped', 'New group')) {
        if ([string]::Equals($Name, $kept, [StringComparison]::OrdinalIgnoreCase)) { return 'it is a name the panel keeps for itself' }
    }
    if ($Name.StartsWith('Archived sessions', [StringComparison]::OrdinalIgnoreCase)) { return 'it is a name the panel keeps for itself' }
    ''
}

# A plan that refuses while any group's name field is open in the list (typing could go into the wrong one), else $null.
function Get-EditingPlan($Items) {
    $e = @(@($Items) | Where-Object { $_.Kind -eq 'header' -and $_.Editing })
    if ($e.Count -gt 0) { return (New-GroupPlan 'refuse' 5 "a group's name field is already open in the list (it shows '$($e[0].Name)'); finish or cancel that first; nothing was changed") }
    $null
}

function New-GroupPlan([string]$Act, [int]$Code, [string]$Text) { [pscustomobject]@{ Act = $Act; Code = $Code; Text = $Text } }

# The headers with exactly this name (ordinal, case-sensitive).
function Get-GroupHeaders($Items, [string]$Name) { @(@($Items) | Where-Object { $_.Kind -eq 'header' -and $_.Name -ceq $Name }) }

# The ids of the rows under the header of this name, in order.
function Get-HeaderRowIds($Items, [string]$Name) {
    $ids = @()
    $in = $false
    foreach ($it in @($Items)) {
        if ($it.Kind -eq 'header') { $in = ($it.Name -ceq $Name); continue }
        if ($in) { $ids += [string]$it.Id }
    }
    $ids
}

# The name of the header a row follows ('' when it follows none), or $null when no row has this id.
function Get-RowGroupName($Items, [string]$Id) {
    $group = ''
    foreach ($it in @($Items)) {
        if ($it.Kind -eq 'header') { $group = [string]$it.Name; continue }
        if ($it.Kind -eq 'row' -and $it.Id -ceq $Id) { return $group }
    }
    $null
}

function Get-GroupNoRowText { 'no row for this chat is in the list as shown (a collapsed group or the archived section hides its rows); nothing was changed' }

function Get-GroupCreatePlan($Items, [string]$Name) {
    $editing = Get-EditingPlan $Items
    if ($null -ne $editing) { return $editing }
    $why = Test-GroupName $Name
    if ($why -ne '') { return (New-GroupPlan 'refuse' 5 "'$Name' cannot be a group name: $why; nothing was changed") }
    if (@(Get-GroupHeaders $Items $Name).Count -gt 0) { return (New-GroupPlan 'refuse' 5 "a group named '$Name' already exists; nothing was changed") }
    if (@(Get-GroupHeaders $Items 'New group').Count -gt 0) {
        return (New-GroupPlan 'refuse' 5 "a group named 'New group' already exists, so the new group could not be told apart from it; rename or delete that one first; nothing was changed")
    }
    New-GroupPlan 'do' 0 ''
}

function Get-GroupRenamePlan($Items, [string]$Old, [string]$New) {
    $editing = Get-EditingPlan $Items
    if ($null -ne $editing) { return $editing }
    $hs = @(Get-GroupHeaders $Items $Old)
    if ($hs.Count -eq 0) { return (New-GroupPlan 'refuse' 5 "there is no group named '$Old'; nothing was changed") }
    if ($hs.Count -gt 1) { return (New-GroupPlan 'refuse' 5 "$($hs.Count) groups are named '$Old', so the right one cannot be told apart; nothing was changed") }
    if (Test-GroupFixed $Old) { return (New-GroupPlan 'refuse' 5 "'$Old' is the panel's own header, not one of his groups; nothing was changed") }
    $why = Test-GroupName $New
    if ($why -ne '') { return (New-GroupPlan 'refuse' 5 "'$New' cannot be a group name: $why; nothing was changed") }
    if ($Old -ceq $New) { return (New-GroupPlan 'nothing' 0 "the group is already named '$New'; nothing to do") }
    if (@(Get-GroupHeaders $Items $New).Count -gt 0) { return (New-GroupPlan 'refuse' 5 "a group named '$New' already exists; nothing was changed") }
    New-GroupPlan 'do' 0 ''
}

function Get-GroupDeletePlan($Items, [string]$Name, [bool]$Force) {
    $editing = Get-EditingPlan $Items
    if ($null -ne $editing) { return $editing }
    $hs = @(Get-GroupHeaders $Items $Name)
    if ($hs.Count -eq 0) { return (New-GroupPlan 'refuse' 5 "there is no group named '$Name'; nothing was changed") }
    if ($hs.Count -gt 1) { return (New-GroupPlan 'refuse' 5 "$($hs.Count) groups are named '$Name', so the right one cannot be told apart; nothing was changed") }
    if (Test-GroupFixed $Name) { return (New-GroupPlan 'refuse' 5 "'$Name' is the panel's own header, not one of his groups; nothing was changed") }
    $shown = -1
    $prop = $hs[0].PSObject.Properties['Count']
    if ($null -ne $prop -and $null -ne $prop.Value) { $shown = [int]$prop.Value }
    $count = [Math]::Max($shown, @(Get-HeaderRowIds $Items $Name).Count)
    if ($count -gt 0 -and -not $Force) {
        return (New-GroupPlan 'refuse' 5 "the group '$Name' holds $count chat(s); deleting it moves them to Ungrouped, and the panel asks nothing before it deletes; nothing was changed. Give -Force to delete it anyway")
    }
    # A count that could not be read is unknown, not empty.
    if ($shown -lt 0 -and -not $Force) {
        return (New-GroupPlan 'refuse' 5 "the chat count of the group '$Name' could not be read, so it may hold chats; deleting it moves any to Ungrouped, and the panel asks nothing before it deletes; nothing was changed. Give -Force to delete it anyway")
    }
    New-GroupPlan 'do' 0 ''
}

function Get-GroupAddPlan($Items, [string]$Id, [string]$Name) {
    $editing = Get-EditingPlan $Items
    if ($null -ne $editing) { return $editing }
    $cur = Get-RowGroupName $Items $Id
    if ($null -eq $cur) { return (New-GroupPlan 'refuse' 2 (Get-GroupNoRowText)) }
    $hs = @(Get-GroupHeaders $Items $Name)
    if ($hs.Count -eq 0) { return (New-GroupPlan 'refuse' 5 "there is no group named '$Name'; nothing was changed") }
    if ($hs.Count -gt 1) { return (New-GroupPlan 'refuse' 5 "$($hs.Count) groups are named '$Name', so the right one cannot be told apart; nothing was changed") }
    if (Test-GroupFixed $Name) { return (New-GroupPlan 'refuse' 5 "'$Name' is the panel's own header, not one of his groups; nothing was changed") }
    if ((Test-GroupFixed $cur) -and $cur -cne 'Ungrouped') { return (New-GroupPlan 'refuse' 5 'the chat is in the archived section, which has a menu of its own; nothing was changed') }
    if ($cur -ceq $Name) { return (New-GroupPlan 'nothing' 0 "the chat is already in the group '$Name'; nothing to do") }
    New-GroupPlan 'do' 0 ''
}

function Get-GroupRemovePlan($Items, [string]$Id) {
    $editing = Get-EditingPlan $Items
    if ($null -ne $editing) { return $editing }
    $cur = Get-RowGroupName $Items $Id
    if ($null -eq $cur) { return (New-GroupPlan 'refuse' 2 (Get-GroupNoRowText)) }
    if ($cur -ceq '' -or $cur -ceq 'Ungrouped') { return (New-GroupPlan 'nothing' 0 'the chat is not in a group (it is under Ungrouped); nothing to do') }
    if (Test-GroupFixed $cur) { return (New-GroupPlan 'refuse' 5 'the chat is in the archived section, which has a menu of its own; nothing was changed') }
    New-GroupPlan 'do' 0 ''
}

function Get-GroupPlan([string]$Kind, $Items, [string]$Id, [string]$Name, [string]$NewName, [bool]$ForceDelete) {
    switch ($Kind) {
        'create' { return (Get-GroupCreatePlan $Items $Name) }
        'rename' { return (Get-GroupRenamePlan $Items $Name $NewName) }
        'delete' { return (Get-GroupDeletePlan $Items $Name $ForceDelete) }
        'add' { return (Get-GroupAddPlan $Items $Id $Name) }
        'remove' { return (Get-GroupRemovePlan $Items $Id) }
    }
    New-GroupPlan 'refuse' 5 "unknown group action '$Kind'"
}

# Whether a fresh read of the list shows the outcome.
function Test-GroupCreateDone($Items, [string]$Name) {
    if (@(Get-GroupHeaders $Items $Name).Count -ne 1) { return $false }
    @(@($Items) | Where-Object { $_.Kind -eq 'header' -and $_.Editing }).Count -eq 0
}
function Test-GroupRenameDone($Items, [string]$Old, [string]$New, $RowsBefore) {
    if (@(Get-GroupHeaders $Items $New).Count -ne 1) { return $false }
    if (@(Get-GroupHeaders $Items $Old).Count -ne 0) { return $false }
    ((@(Get-HeaderRowIds $Items $New)) -join '|') -ceq ((@($RowsBefore)) -join '|')
}
function Test-GroupDeleteDone($Items, [string]$Name) {
    # An empty or half-drawn read has no header of that name either, so the Ungrouped header must be in the same read.
    if (@(Get-GroupHeaders $Items 'Ungrouped').Count -eq 0) { return $false }
    @(Get-GroupHeaders $Items $Name).Count -eq 0
}
function Test-GroupAddDone($Items, [string]$Id, [string]$Name) { [bool]((Get-RowGroupName $Items $Id) -ceq $Name) }
function Test-GroupRemoveDone($Items, [string]$Id) { [bool]((Get-RowGroupName $Items $Id) -ceq 'Ungrouped') }

# ---- reading the screen for a group action

function Get-FocusedElement { try { $AE::FocusedElement } catch { $null } }

# The element that has the keyboard focus when it is a group's name field: class groupNameEditing*, an ancestor that is the
# Group named Sessions, and a header above it that is the one this run is about ($script:FieldOwner): for a rename the header
# that was right-clicked, for a create a header that was not in the list before the button was pressed. $null in any other
# state, so nothing is typed into another group's field.
function Get-NameField {
    $f = Get-FocusedElement
    if ($null -eq $f) { return $null }
    try { if ([string]$f.Current.ClassName -notlike '*groupNameEditing_*') { return $null } } catch { return $null }
    $header = $null
    $inSessions = $false
    $p = Get-ParentOf $f
    for ($i = 0; $i -lt 8 -and $null -ne $p; $i++) {
        try {
            if ($null -eq $header -and [string]$p.Current.ClassName -match '(^|\s)groupHeader_') { $header = $p }
            if ($p.Current.ControlType -eq $CT::Group -and [string]$p.Current.Name -ceq 'Sessions') { $inSessions = $true; break }
        } catch { return $null }
        $p = Get-ParentOf $p
    }
    if (-not $inSessions -or $null -eq $header) { return $null }
    $owner = $script:FieldOwner
    if ($null -eq $owner) { return $null }
    if ($owner.Mode -eq 'is') {
        if (-not (Test-SameElement $header $owner.Element)) { return $null }
    } else {
        $key = Get-RuntimeKey $header
        if ([string]::IsNullOrEmpty($key) -or @($owner.Known) -contains $key) { return $null }
    }
    $f
}

function Test-SameElement($A, $B) { try { [bool][System.Windows.Automation.Automation]::Compare($A, $B) } catch { $false } }

# An element's runtime id as one string, or $null.
function Get-RuntimeKey($El) { try { (@($El.GetRuntimeId()) -join '.') } catch { $null } }

# True when the keyboard focus is inside an open context menu (on one of its items, or on a submenu's item).
function Test-FocusInOpenMenu {
    $p = Get-FocusedElement
    for ($i = 0; $i -lt 8 -and $null -ne $p; $i++) {
        try { if ($p.Current.ControlType -eq $CT::Menu -and [string]$p.Current.ClassName -like '*contextMenu_*') { return $true } } catch { return $false }
        $p = Get-ParentOf $p
    }
    $false
}

# The context menus open now (a Menu of class contextMenu_*; the submenu of Add to group is a second one).
function Get-OpenMenus {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Menu)
    $out = @()
    foreach ($m in $script:Win.FindAll($TS::Descendants, $cond)) {
        try { if ([string]$m.Current.ClassName -like '*contextMenu_*') { $out += $m } } catch { }
    }
    $out
}

# The one item of a menu with this name: exact, or the start of the name when $Prefix. $null unless there is exactly one.
function Find-MenuItem($Menu, [string]$Wanted, [bool]$Prefix) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::MenuItem)
    $hit = @()
    foreach ($i in $Menu.FindAll($TS::Descendants, $cond)) {
        $n = ''
        try { $n = [string]$i.Current.Name } catch { continue }
        if ($Prefix) { if ($n.StartsWith($Wanted, [StringComparison]::Ordinal)) { $hit += $i } }
        elseif ($n -ceq $Wanted) { $hit += $i }
    }
    if ($hit.Count -eq 1) { return $hit[0] }
    $null
}

# Polls for an open menu (other than $Skip) that holds the item: { Menu; Item }, or $null after $Ms milliseconds.
function Wait-MenuItem([string]$Wanted, [bool]$Prefix, $Skip, [int]$Ms = 2500) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        foreach ($m in @(Get-OpenMenus)) {
            if ($null -ne $Skip -and [System.Windows.Automation.Automation]::Compare($m, $Skip)) { continue }
            $i = Find-MenuItem $m $Wanted $Prefix
            if ($null -ne $i) { return [pscustomobject]@{ Menu = $m; Item = $i } }
        }
        if ($sw.ElapsedMilliseconds -ge $Ms) { return $null }
        Start-Sleep -Milliseconds 100
    }
}

# Polls the list until the named check (one of the Test-Group...Done functions, given the items and these arguments) is
# true: { Ok; Items }, the last read when it never was.
function Wait-GroupList([string]$Check, $CheckArgs, [int]$Ms) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $items = @(Get-ListItems $script:Win)
        if (& $Check $items @CheckArgs) { return [pscustomobject]@{ Ok = $true; Items = $items } }
        if ($sw.ElapsedMilliseconds -ge $Ms) { return [pscustomobject]@{ Ok = $false; Items = $items } }
        Start-Sleep -Milliseconds 250
    }
}

# The name field has the keyboard focus within $Ms milliseconds, or $null.
function Wait-NameField([int]$Ms) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $f = Get-NameField
        if ($null -ne $f) { return $f }
        if ($sw.ElapsedMilliseconds -ge $Ms) { return $null }
        Start-Sleep -Milliseconds 100
    }
}

# The focused name field shows exactly this text within $Ms milliseconds.
function Wait-FieldText([string]$Want, [int]$Ms) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $f = Get-NameField
        if ($null -ne $f -and (Get-FieldText $f) -ceq $Want) { return $true }
        if ($sw.ElapsedMilliseconds -ge $Ms) { return $false }
        Start-Sleep -Milliseconds 100
    }
}

# The group header with this exact name, found again right before it is used: exactly one, and not a fixed one.
function Get-GroupHeaderElement([string]$Name) {
    $hs = @(Get-GroupHeaders @(Get-ListItems $script:Win) $Name)
    if ($hs.Count -ne 1) { Stop-Panel 5 "the list shows $($hs.Count) groups named '$Name', not one; no input was sent" }
    if (Test-GroupFixed $Name) { Stop-Panel 5 "'$Name' is the panel's own header; no input was sent" }
    $hs[0].Element
}

function Find-NewGroupButton {
    $b = @(Find-Named $script:Win 'New group' $CT::Button | Where-Object { try { [string]$_.Current.ClassName -match '(^|\s)newGroupButton_' } catch { $false } })
    if ($b.Count -ne 1) { Stop-Panel 2 "the New group button is showing $($b.Count) times, not once; nothing was changed" }
    $b[0]
}

# True when the element at a screen point is the target or lies inside it.
function Test-PointerOnElement($Target, $Spot) {
    try {
        $el = $AE::FromPoint((New-Object System.Windows.Point($Spot.X, $Spot.Y)))
        $tw = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        for ($i = 0; $i -lt 8 -and $null -ne $el; $i++) {
            if ([System.Windows.Automation.Automation]::Compare($el, $Target)) { return $true }
            $el = $tw.GetParent($el)
        }
    } catch { }
    $false
}

# What is left half done and how the list looks now, for the message of a run that stopped.
function Get-GroupLeftText {
    $list = ''
    try {
        $parts = @(Get-ListItems $script:Win | Where-Object { $_.Kind -eq 'header' } | ForEach-Object { '{0} ({1})' -f $_.Name, $_.Count })
        $list = 'The list now shows these groups: ' + ($parts -join ', ') + '.'
    } catch { $list = 'The list could not be read afterwards.' }
    (@($script:GroupLeft, $list) | Where-Object { $_ -ne '' }) -join ' '
}

# ---- the steps that send input: each checks its own conditions first, and nothing else in the driver may use PanelInput

# The last-input tick right after the driver's own input, so the end of the run can tell whether he did anything after it.
function Set-OwnTick {
    Start-Sleep -Milliseconds 40
    $script:OwnTick = [PanelNative]::InputTick()
}

# He came back when the pointer is no longer where the driver last put it.
function Assert-GroupPointer {
    if (-not (Test-PointerAt $script:Placed)) { Stop-Panel 3 'the operator came back: the pointer is no longer where the driver put it' }
}

# VS Code to the foreground (measured method), or exit 3 with nothing sent.
function Move-GroupFront {
    if ([PanelNative]::Foreground() -eq $script:Hwnd) { return }
    [void][PanelInput]::BringToFront($script:Hwnd)
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt 1500) {
        if ([PanelNative]::Foreground() -eq $script:Hwnd) { return }
        Start-Sleep -Milliseconds 50
    }
    Stop-Panel 3 'VS Code did not come to the foreground within 1.5 seconds (Windows kept another window in front); the list was not touched'
}

# Opens the context menu of a row or a header: the pointer goes onto it, and one right press is sent, only when VS Code
# is in front, no context menu is open, the target is on screen with a real rectangle (scrolled into view first), the
# pointer is where the driver put it, the window under it is VS Code and the element under it is the target.
function Open-GroupMenu($Target) {
    if ([PanelNative]::Foreground() -ne $script:Hwnd) { Stop-Panel 3 'VS Code is no longer the foreground window; no input was sent' }
    if (@(Get-OpenMenus).Count -gt 0) { Stop-Panel 5 'a context menu is already open; no input was sent' }
    $sp = $null
    if ($Target.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$sp)) { try { $sp.ScrollIntoView() } catch { } }
    Start-Sleep -Milliseconds 300
    $spot = Get-RowCenter $Target
    if ($null -eq $spot) { Stop-Panel 5 'the target has no usable place on the screen (an empty rectangle, off screen, or outside the VS Code window); no input was sent' }
    Assert-GroupPointer
    if (-not [PanelNative]::MovePointer($spot.X, $spot.Y)) { Stop-Panel 5 'the pointer could not be moved onto the target; no input was sent' }
    # Placed where the driver asked, whatever a read-back says, so it is put back whatever happens next.
    $script:Placed = [pscustomobject]@{ X = $spot.X; Y = $spot.Y }
    $at = [PanelNative]::PointerPos()
    if ($null -ne $at) { $script:Placed = [pscustomobject]@{ X = $at[0]; Y = $at[1] } }
    Start-Sleep -Milliseconds 150
    # The last checks, then the one input call with nothing in between.
    if ([PanelNative]::Foreground() -ne $script:Hwnd) { Stop-Panel 3 'VS Code is no longer the foreground window; no input was sent' }
    if (@(Get-OpenMenus).Count -gt 0) { Stop-Panel 5 'a context menu is already open; no input was sent' }
    Assert-GroupPointer
    if ([PanelNative]::RootAt($script:Placed.X, $script:Placed.Y) -ne $script:Hwnd) { Stop-Panel 5 'the window under the pointer is not VS Code; no input was sent' }
    if (-not (Test-PointerOnElement $Target $script:Placed)) { Stop-Panel 5 'the element under the pointer is not the target; no input was sent' }
    if (-not [PanelInput]::RightClick()) { Stop-Panel 1 'Windows did not accept the right button press; the context menu was not opened' }
    Set-OwnTick
}

# Typing: each step first checks that the keyboard focus is the name field, and that he has not moved the pointer.
function Send-GroupSelectAll {
    Assert-GroupPointer
    if ($null -eq (Get-NameField)) { Stop-Panel 1 'the keyboard focus is not the group name field; nothing was typed' }
    if (-not [PanelInput]::SelectAll()) { Stop-Panel 1 'Windows did not accept the select-all; nothing was typed' }
    Set-OwnTick
}

function Send-GroupText([string]$Text) {
    Assert-GroupPointer
    if ($null -eq (Get-NameField)) { Stop-Panel 1 'the keyboard focus is not the group name field; nothing was typed' }
    if (-not [PanelInput]::TypeText($Text)) { Stop-Panel 1 'Windows did not accept the typed text' }
    Set-OwnTick
}

# Enter only while the focused field shows exactly the wanted name.
function Send-GroupEnter([string]$Want) {
    Assert-GroupPointer
    $f = Get-NameField
    if ($null -eq $f) { Stop-Panel 1 'the keyboard focus is not the group name field; Enter was not sent' }
    $shown = Get-FieldText $f
    if ($shown -cne $Want) { Stop-Panel 1 "the name field shows '$shown', not the wanted name; Enter was not sent and the field was left open" }
    if (-not [PanelInput]::Enter()) { Stop-Panel 1 'Windows did not accept Enter' }
    Set-OwnTick
}

# Invokes a menu item (an item of the menu just opened).
function Invoke-MenuItem($Item) {
    $script:MenuInvoked = $true
    $Item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

# Closes a context menu that is still open: Escape only while a menu is seen open, VS Code is in front and the keyboard focus
# is inside an open menu (measured: while a menu is open the focus is on one of its items), never in any other state (in a
# chat Escape stops a running turn). After each Escape the open menus are counted again for up to a second before anything
# else is decided, so a menu that is only slow to leave the tree gets no second Escape. After an item was invoked its menu is
# closing by itself, so that is waited for first.
function Close-GroupMenus {
    if ($script:MenuInvoked) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        while ($sw.ElapsedMilliseconds -lt 1500 -and @(Get-OpenMenus).Count -gt 0) { Start-Sleep -Milliseconds 100 }
    }
    for ($i = 0; $i -lt 4; $i++) {
        $open = @(Get-OpenMenus).Count
        if ($open -eq 0) { return }
        if ([PanelNative]::Foreground() -ne $script:Hwnd) { Add-Note 'a context menu was left open: VS Code is no longer in front'; return }
        if (-not (Test-FocusInOpenMenu)) { Add-Note 'a context menu was left open: the keyboard focus is not in it, so Escape was not sent'; return }
        [void][PanelInput]::Escape()
        Set-OwnTick
        $sw = [Diagnostics.Stopwatch]::StartNew()
        while ($sw.ElapsedMilliseconds -lt 1000 -and @(Get-OpenMenus).Count -ge $open) { Start-Sleep -Milliseconds 50 }
    }
    if (@(Get-OpenMenus).Count -gt 0) { Add-Note 'a context menu is still open' }
}

# The foreground goes back to the window that had it before the run, only while VS Code still has it, that window
# still exists and is visible, and he has not used the computer since the driver's own last input.
function Restore-GroupForeground($Prev) {
    if ($Prev -eq [IntPtr]::Zero -or $Prev -eq $script:Hwnd) { return }
    if ([PanelNative]::Foreground() -ne $script:Hwnd) { return }
    if (-not [PanelNative]::IsWindow($Prev) -or -not [PanelNative]::IsVisible($Prev)) { return }
    if ($null -ne $script:OwnTick -and [PanelNative]::InputTick() -ne $script:OwnTick) {
        Add-Note 'he used the computer after the driver, so the foreground was left as it is'
        return
    }
    [void][PanelInput]::BringToFront($Prev)
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt 500) {
        if ([PanelNative]::Foreground() -eq $Prev) { return }
        Start-Sleep -Milliseconds 50
    }
    if ([PanelNative]::Foreground() -ne $Prev) {
        $script:FgFailed = $true
        Add-Note 'VS Code was left in front'
    }
}

# ---- the five actions

function Step-GroupCreate([string]$Name) {
    $known = @(Get-ListItems $script:Win | Where-Object { $_.Kind -eq 'header' } | ForEach-Object { Get-RuntimeKey $_.Element } | Where-Object { $_ })
    $script:FieldOwner = [pscustomobject]@{ Mode = 'new'; Element = $null; Known = $known }
    $btn = Find-NewGroupButton
    $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $script:GroupLeft = "A group named 'New group' was added to the list and is not named: its name field was left open."
    if ($null -eq (Wait-NameField 3000)) { Stop-Panel 1 'the New group button was pressed but the keyboard focus did not go to the name field' }
    Send-GroupSelectAll
    Send-GroupText $Name
    [void](Wait-FieldText $Name 2500)
    Send-GroupEnter $Name
    $r = Wait-GroupList 'Test-GroupCreateDone' @($Name) 3000
    if (-not $r.Ok) { Stop-Panel 1 "Enter was sent, but the list does not show exactly one group named '$Name' after 3 seconds" }
    $script:GroupLeft = ''
    "created the group '$Name'; the list shows it"
}

function Step-GroupRename([string]$Old, [string]$New, $RowsBefore) {
    $header = Get-GroupHeaderElement $Old
    $script:FieldOwner = [pscustomobject]@{ Mode = 'is'; Element = $header; Known = @() }
    Open-GroupMenu $header
    $found = Wait-MenuItem 'Rename group' $false $null
    if ($null -eq $found) { Stop-Panel 1 'the context menu did not show Rename group; nothing was changed' }
    Invoke-MenuItem $found.Item
    $script:GroupLeft = "The group '$Old' was put in rename mode and is not renamed: its name field was left open."
    if ($null -eq (Wait-NameField 3000)) { Stop-Panel 1 'Rename group was pressed but the keyboard focus did not go to the name field' }
    Send-GroupSelectAll
    Send-GroupText $New
    [void](Wait-FieldText $New 2500)
    Send-GroupEnter $New
    $r = Wait-GroupList 'Test-GroupRenameDone' @($Old, $New, ($RowsBefore -join '|')) 3000
    if (-not $r.Ok) { Stop-Panel 1 "Enter was sent, but the list does not show '$New' with the chats that were in '$Old' after 3 seconds" }
    $script:GroupLeft = ''
    "renamed the group '$Old' to '$New'; the list shows it with its chats"
}

function Step-GroupDelete([string]$Name) {
    Open-GroupMenu (Get-GroupHeaderElement $Name)
    $found = Wait-MenuItem 'Delete group' $false $null
    if ($null -eq $found) { Stop-Panel 1 'the context menu did not show Delete group; nothing was changed' }
    Invoke-MenuItem $found.Item
    $script:GroupLeft = "Delete group was pressed for '$Name'; the list was not seen to change."
    $r = Wait-GroupList 'Test-GroupDeleteDone' @($Name) 3000
    if (-not $r.Ok) { Stop-Panel 1 "Delete group was pressed, but the list still shows a group named '$Name' after 3 seconds" }
    $script:GroupLeft = ''
    "deleted the group '$Name'; its chats are under Ungrouped"
}

function Step-GroupAdd([string]$Id, [string]$Name) {
    $row = Find-RowElement $Id
    if ($null -eq $row) { Stop-Panel 5 "the chat's row left the list while the driver was getting ready; nothing was changed" }
    Open-GroupMenu $row
    $found = Wait-MenuItem 'Add to group' $true $null
    if ($null -eq $found) { Stop-Panel 1 'the context menu did not show Add to group; nothing was changed' }
    $ec = $null
    if (-not $found.Item.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$ec)) { Stop-Panel 5 'the Add to group item cannot be expanded; nothing was changed' }
    $ec.Expand()
    $sub = Wait-MenuItem $Name $false $found.Menu
    if ($null -eq $sub) { Stop-Panel 5 "the Add to group submenu has no item named '$Name'; nothing was changed" }
    Invoke-MenuItem $sub.Item
    $script:GroupLeft = "The item '$Name' was pressed for the chat; its row was not seen to move."
    $r = Wait-GroupList 'Test-GroupAddDone' @($Id, $Name) 3000
    if (-not $r.Ok) { Stop-Panel 1 "the group item was pressed, but the chat's row does not sit under '$Name' after 3 seconds" }
    $script:GroupLeft = ''
    "added session $Id to the group '$Name'; its row sits under that header"
}

function Step-GroupRemove([string]$Id) {
    $row = Find-RowElement $Id
    if ($null -eq $row) { Stop-Panel 5 "the chat's row left the list while the driver was getting ready; nothing was changed" }
    Open-GroupMenu $row
    $found = Wait-MenuItem 'Remove from group' $false $null
    if ($null -eq $found) { Stop-Panel 5 'the context menu does not show Remove from group; nothing was changed' }
    Invoke-MenuItem $found.Item
    $script:GroupLeft = 'Remove from group was pressed for the chat; its row was not seen to move.'
    $r = Wait-GroupList 'Test-GroupRemoveDone' @($Id) 3000
    if (-not $r.Ok) { Stop-Panel 1 "Remove from group was pressed, but the chat's row does not sit under Ungrouped after 3 seconds" }
    $script:GroupLeft = ''
    "removed session $Id from its group; its row sits under Ungrouped"
}

# One group action. Kind is create, rename, delete, add or remove. Names that can never be used are refused before the
# run waits for anything. Then, in order: the operator's idle guard (Enter-Run; no chat has to be idle), the full-screen
# guard, a fresh read and the decision (a refusal, or nothing to do, changes nothing), the pointer and the foreground
# window recorded, VS Code to the front (exit 3 when it does not come), the steps, and in the end: an open context menu
# closed, the pointer put back, the foreground handed back.
function Do-Group([string]$Kind, [string]$Id, [string]$Name, [string]$NewName, [bool]$ForceDelete) {
    $Name = $Name.Trim()
    $NewName = $NewName.Trim()
    $script:GroupLeft = ''
    $script:MenuInvoked = $false
    $script:Placed = $null
    $script:OwnTick = $null
    $script:FieldOwner = $null
    $script:GroupRun = $true
    if ($Kind -eq 'create' -or $Kind -eq 'rename') {
        $wanted = $Name
        if ($Kind -eq 'rename') { $wanted = $NewName }
        $why = Test-GroupName $wanted
        if ($why -ne '') { Stop-Panel 5 "'$wanted' cannot be a group name: $why; nothing was changed" }
    }
    Enter-Run ''
    Ensure-Screen
    $fsLogged = $false
    while ($true) {
        Wait-Idle 'bring VS Code to the front'
        if (Test-FullScreenFront) {
            Stop-IfCancelled
            if ((Get-Date) -ge $script:WaitUntil) { Stop-Panel 3 'a full-screen window stayed in front until the deadline; VS Code was not brought forward and nothing was changed' }
            if (-not $fsLogged) { Write-Waiting 'a full-screen window is in front'; $fsLogged = $true }
            Start-Sleep -Seconds 2
            continue
        }
        Clear-Waiting
        if ([PanelNative]::IdleMs() -ge ($IdleSeconds * 1000)) { break }
    }
    $items = @(Get-ListItems $script:Win)
    $plan = Get-GroupPlan $Kind $items $Id $Name $NewName $ForceDelete
    if ($plan.Act -eq 'refuse') { Stop-Panel $plan.Code $plan.Text }
    if ($plan.Act -eq 'nothing') { return $plan.Text }
    # He may have come back while the list was read and the plan made: nothing has been touched yet, so just stop.
    if ([PanelNative]::IdleMs() -lt ($IdleSeconds * 1000)) { Stop-Panel 3 'he came back (the idle time fell after the list was read); nothing was changed' }
    $rowsBefore = @()
    if ($Kind -eq 'rename') { $rowsBefore = @(Get-HeaderRowIds $items $Name) }
    $old = [PanelNative]::PointerPos()
    if ($null -eq $old) { Stop-Panel 5 'the pointer position could not be read; nothing was changed' }
    $prev = [PanelNative]::Foreground()
    $script:Placed = [pscustomobject]@{ X = $old[0]; Y = $old[1] }
    $script:OwnTick = [PanelNative]::InputTick()
    try {
        Move-GroupFront
        # Bringing a window forward is not his input, but the tick is read again so only what comes after it counts.
        Set-OwnTick
        switch ($Kind) {
            'create' { return (Step-GroupCreate $Name) }
            'rename' { return (Step-GroupRename $Name $NewName $rowsBefore) }
            'delete' { return (Step-GroupDelete $Name) }
            'add' { return (Step-GroupAdd $Id $Name) }
            'remove' { return (Step-GroupRemove $Id) }
        }
    } catch {
        $pe = Get-PanelExit $_
        $code = 1
        $reason = ''
        if ($null -ne $pe) { $code = $pe.Code; $reason = $pe.Reason }
        else { $reason = 'unexpected: ' + $_.Exception.Message + ' (line ' + $_.InvocationInfo.ScriptLineNumber + ')' }
        Stop-Panel $code ($reason.TrimEnd('.') + '. ' + (Get-GroupLeftText))
    } finally {
        try { Close-GroupMenus } catch { Add-Note ('could not close a context menu: ' + $_.Exception.Message) }
        try { Restore-Pointer $old $script:Placed } catch { Add-Note ('could not put the pointer back: ' + $_.Exception.Message) }
        try { Restore-GroupForeground $prev } catch { Add-Note ('could not give the foreground back: ' + $_.Exception.Message) }
    }
}

function Do-CloseUntitled {
    Test-Bridge
    $blank = @(Get-ClaudeTabs | Where-Object { $_.label -ceq $UntitledLabel })
    if ($blank.Count -eq 0) { return 'closed 0: no untitled Claude Code tab is open' }
    New-RunState '' ''
    Enter-Run ''
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
        $why = ''
        # A group action's reason can hold a group name, so its line carries the exit code only (the reason is on stderr).
        if ($Code -ne 0 -and $ActionName -notlike 'group*') { $why = ' ' + ($Reason -replace '[\r\n]+', ' ') }
        if ($script:Notes.Count -gt 0) { $why += ' [' + (($script:Notes -join '; ') -replace '[\r\n]+', ' ') + ']' }
        $line = '{0} {1} {2} exit={3} secs={4:N1}{5}' -f (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss'), $ActionName, $(if ($Id -ne '') { $Id } else { '-' }), $Code, $script:Clock.Elapsed.TotalSeconds, $why
        Add-LogLine $line
    } catch { }
}

# One run: the lock (for a run that changes anything), the body, then his view and the window put back
# and the lock let go whatever happened, then one log line.
function Invoke-Logged([string]$ActionName, [string]$LogId, [bool]$Locked, [scriptblock]$Body) {
    $script:Notes = @()
    $script:Clock = [Diagnostics.Stopwatch]::StartNew()
    $script:RunAction = $ActionName
    $script:RunId = $LogId
    $script:LastWait = ''
    $script:FgStart = $null
    $script:FgFailed = $false
    $script:GroupRun = $false
    Set-Deadlines $ActionName
    $r = [pscustomobject]@{ Code = 0; Reason = 'ok'; Value = $null }
    try {
        try {
            # The body takes the panel lock itself, after it has waited for him (Enter-Run).
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
    $r = Step-Run 'new (Haiku 5.5, two-line first prompt)' { Do-New $first $SelfTestModel } ''
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
$code = 1
switch ($Action) {
    'list' { $code = Out-Result (Invoke-Logged 'list' '' $false { Do-List }) }
    'send' {
        if ($Session -eq '') { [Console]::Error.WriteLine('send needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'send' $Session $true { Do-Send $Session (Read-Text $TextFile) })
        }
    }
    'new' { $code = Out-Result (Invoke-Logged 'new' '' $true { Do-New (Read-Text $TextFile) $Model }) }
    'model' {
        if ($Session -eq '') { [Console]::Error.WriteLine('model needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'model' $Session $true { Do-Model $Session $Model })
        }
    }
    'archive' {
        if ($Session -eq '') { [Console]::Error.WriteLine('archive needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'archive' $Session $true { Do-Archive $Session })
        }
    }
    'groups' { $code = Out-Result (Invoke-Logged 'groups' '' $false { Do-List 'groups' }) }
    'group-create' {
        if ($Group -eq '') { [Console]::Error.WriteLine('group-create needs -Group'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'group-create' '' $true { Do-Group 'create' '' $Group '' $false })
        }
    }
    'group-rename' {
        if ($Group -eq '' -or $NewName -eq '') { [Console]::Error.WriteLine('group-rename needs -Group and -NewName'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'group-rename' '' $true { Do-Group 'rename' '' $Group $NewName $false })
        }
    }
    'group-delete' {
        if ($Group -eq '') { [Console]::Error.WriteLine('group-delete needs -Group'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'group-delete' '' $true { Do-Group 'delete' '' $Group '' ([bool]$Force) })
        }
    }
    'group-add' {
        if ($Session -eq '' -or $Group -eq '') { [Console]::Error.WriteLine('group-add needs -Session and -Group'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'group-add' $Session $true { Do-Group 'add' $Session $Group '' $false })
        }
    }
    'group-remove' {
        if ($Session -eq '') { [Console]::Error.WriteLine('group-remove needs -Session'); $code = 1 } else {
            $code = Out-Result (Invoke-Logged 'group-remove' $Session $true { Do-Group 'remove' $Session '' '' $false })
        }
    }
    'close-untitled' { $code = Out-Result (Invoke-Logged 'close-untitled' '' $true { Do-CloseUntitled }) }
    'selftest' {
        $r = Invoke-Logged 'selftest' '' $true { Do-SelfTest }
        $code = [int]$r.Value
        if ($r.Code -ne 0) { [Console]::Error.WriteLine($r.Reason); $code = $r.Code }
        foreach ($n in $script:Notes) { [Console]::Error.WriteLine('note: ' + $n) }
    }
    default { [Console]::Error.WriteLine('usage: claude_panel.ps1 list | send | new | model | archive | groups | group-create | group-rename | group-delete | group-add | group-remove | close-untitled | selftest'); $code = 1 }
}
exit $code
