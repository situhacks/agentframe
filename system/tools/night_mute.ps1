# Mutes every active output device at night once the operator has been idle a while, and never unmutes.
#
# Started by the Task Scheduler job "AgentFrame night mute" (register-night-mute.ps1) at 23:30. It runs
# until the window ends (06:00 by default) and checks the idle clock (GetLastInputInfo) once a minute:
# idle for -IdleMinutes or more and any output device not muted -> mute them all, and log it. If he
# unmutes and goes idle again before the window ends, it mutes again. Outside the window it exits at
# once, so daytime music with nobody at the keyboard is never touched (the task's StartWhenAvailable can
# start it late after sleep). He unmutes himself in the morning; this script never unmutes.
#
#   -Check    print the window, the idle time and the output devices; change nothing.
#   -DryRun   log "would mute" instead of muting (tests use it with -Now).
#   -Once     one check, then exit.
#   -Now      pretend the clock reads this time (HH:mm), for tests of the window; implies -Once.
#   -IdleSeconds  pretend the idle clock reads this (tests).
# Log: system/logs/night-mute.log (under AF_ROOT when set). An audio error (a monitor asleep, Bluetooth
# gone) is logged once and the next poll tries again, so the script never dies mid-window.
# -Check exits 1 when it cannot read the devices.
param(
    [string]$Start = '23:30',
    [string]$End = '06:00',
    [int]$IdleMinutes = 20,
    [int]$PollSeconds = 60,
    [switch]$Check,
    [switch]$DryRun,
    [switch]$Once,
    [string]$Now = '',
    [int]$IdleSeconds = -1
)
$ErrorActionPreference = 'Stop'
if ($Now) { $Once = [switch]$true }

$root = if ($env:AF_ROOT) { $env:AF_ROOT } else { (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path }
$logDir = Join-Path $root 'system\logs'
$log = Join-Path $logDir 'night-mute.log'
function Note([string]$m) {
    try {
        if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Force $logDir | Out-Null }
        Add-Content -Path $log -Value ("{0} {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), $m) -Encoding utf8
    } catch { }
}

if (-not ('NightMute.Audio' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
namespace NightMute {
    [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumeratorCom {}
    [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDeviceEnumerator {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IMMDeviceCollection devices);
    }
    [Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDeviceCollection {
        [PreserveSig] int GetCount(out uint count);
        [PreserveSig] int Item(uint index, out IMMDevice device);
    }
    [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDevice {
        [PreserveSig] int Activate(ref Guid iid, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object iface);
    }
    [Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IAudioEndpointVolume {
        [PreserveSig] int RegisterControlChangeNotify(IntPtr notify);
        [PreserveSig] int UnregisterControlChangeNotify(IntPtr notify);
        [PreserveSig] int GetChannelCount(out uint count);
        [PreserveSig] int SetMasterVolumeLevel(float level, IntPtr ctx);
        [PreserveSig] int SetMasterVolumeLevelScalar(float level, IntPtr ctx);
        [PreserveSig] int GetMasterVolumeLevel(out float level);
        [PreserveSig] int GetMasterVolumeLevelScalar(out float level);
        [PreserveSig] int SetChannelVolumeLevel(uint channel, float level, IntPtr ctx);
        [PreserveSig] int SetChannelVolumeLevelScalar(uint channel, float level, IntPtr ctx);
        [PreserveSig] int GetChannelVolumeLevel(uint channel, out float level);
        [PreserveSig] int GetChannelVolumeLevelScalar(uint channel, out float level);
        [PreserveSig] int SetMute(int mute, IntPtr ctx);
        [PreserveSig] int GetMute(out int mute);
    }
    public static class Audio {
        // Every active output device (speakers, monitor audio, headphones), not only the default one.
        static List<IAudioEndpointVolume> Endpoints() {
            var e = (IMMDeviceEnumerator)new MMDeviceEnumeratorCom();
            IMMDeviceCollection c;
            int hr = e.EnumAudioEndpoints(0 /* eRender */, 1 /* DEVICE_STATE_ACTIVE */, out c);
            if (hr < 0) throw new Exception("cannot list the output devices (0x" + hr.ToString("X8") + ")");
            uint n;
            hr = c.GetCount(out n);
            if (hr < 0) throw new Exception("cannot count the output devices (0x" + hr.ToString("X8") + ")");
            var list = new List<IAudioEndpointVolume>();
            Guid iid = typeof(IAudioEndpointVolume).GUID;
            for (uint i = 0; i < n; i++) {
                IMMDevice d;
                if (c.Item(i, out d) < 0) continue;
                object o;
                if (d.Activate(ref iid, 23 /* CLSCTX_ALL */, IntPtr.Zero, out o) < 0) continue;
                list.Add((IAudioEndpointVolume)o);
            }
            return list;
        }
        // { active output devices, how many of them are not muted }
        public static int[] Count() {
            var eps = Endpoints();
            int unmuted = 0;
            foreach (var v in eps) { int m; if (v.GetMute(out m) >= 0 && m == 0) unmuted++; }
            return new int[] { eps.Count, unmuted };
        }
        // Mutes every active output device that is not muted; returns how many it muted.
        public static int MuteAll() {
            int done = 0;
            foreach (var v in Endpoints()) {
                int m;
                if (v.GetMute(out m) >= 0 && m != 0) continue;
                // S_FALSE (1) means the state was already that: success.
                int hr = v.SetMute(1, IntPtr.Zero);
                if (hr < 0) throw new Exception("cannot mute an output device (0x" + hr.ToString("X8") + ")");
                done++;
            }
            return done;
        }
    }
    public static class Idle {
        [StructLayout(LayoutKind.Sequential)] struct LII { public uint cbSize; public uint dwTime; }
        [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LII p);
        public static double Seconds() {
            LII l = new LII(); l.cbSize = (uint)Marshal.SizeOf(typeof(LII));
            if (!GetLastInputInfo(ref l)) return -1;
            return unchecked((uint)Environment.TickCount - l.dwTime) / 1000.0;
        }
    }
}
'@
}

function ClockNow { if ($Now) { [datetime]::ParseExact($Now, 'HH:mm', $null) } else { Get-Date } }
function IdleNow { if ($IdleSeconds -ge 0) { [double]$IdleSeconds } else { [NightMute.Idle]::Seconds() } }

# The window runs from Start to End and may cross midnight. Returns the end of the window that holds $t, or $null.
function WindowEnd([datetime]$t) {
    $s = [datetime]::ParseExact($Start, 'HH:mm', $null).TimeOfDay
    $e = [datetime]::ParseExact($End, 'HH:mm', $null).TimeOfDay
    $tod = $t.TimeOfDay
    if ($s -eq $e) { return $null }
    if ($s -lt $e) {
        if ($tod -ge $s -and $tod -lt $e) { return $t.Date + $e }
        return $null
    }
    if ($tod -ge $s) { return $t.Date.AddDays(1) + $e }
    if ($tod -lt $e) { return $t.Date + $e }
    return $null
}

if ($Check) {
    $t = ClockNow; $endAt = WindowEnd $t
    $inWin = if ($endAt) { "inside the window until $($endAt.ToString('HH:mm'))" } else { "outside the window ($Start to $End)" }
    try { $c = [NightMute.Audio]::Count() } catch { Write-Output "error: $($_.Exception.Message)"; exit 1 }
    Write-Output ("now {0}, {1}; idle {2:N0} s (mutes at {3} min); output devices {4}, not muted {5}" -f $t.ToString('HH:mm'), $inWin, (IdleNow), $IdleMinutes, $c[0], $c[1])
    exit 0
}

try { $first = ClockNow; $endAt = WindowEnd $first }
catch { Note "error: bad -Start, -End or -Now ($Start, $End, $Now): $($_.Exception.Message)"; exit 2 }
if (-not $endAt) {
    Note "started at $($first.ToString('HH:mm')), outside the window ($Start to $End); nothing to do"
    Write-Output "outside the window; nothing to do"
    exit 0
}
Note "watching until $($endAt.ToString('HH:mm')); mutes after $IdleMinutes min idle$(if ($DryRun) { ' (dry run)' })"

$lastError = ''
$idleBroken = $false
while ($true) {
    $t = ClockNow
    if ($t -ge $endAt -or -not (WindowEnd $t)) { Note "window ended"; break }
    try {
        $idle = IdleNow
        if ($idle -lt 0) {
            if (-not $idleBroken) { Note "error: the idle clock cannot be read; trying again each minute" }
            $idleBroken = $true
        } else {
            $idleBroken = $false
            if ($idle -ge ($IdleMinutes * 60)) {
                $c = [NightMute.Audio]::Count()
                if ($c[1] -gt 0) {
                    if ($DryRun) {
                        Note ("would mute {0} of {1} output devices: idle {2:N0} min" -f $c[1], $c[0], ($idle / 60))
                        Write-Output 'would mute'
                    } else {
                        $n = [NightMute.Audio]::MuteAll()
                        Note ("muted {0} of {1} output devices: idle {2:N0} min" -f $n, $c[0], ($idle / 60))
                        Write-Output 'muted'
                    }
                }
            }
        }
        $lastError = ''
    } catch {
        $m = $_.Exception.Message
        if ($m -ne $lastError) { Note "error: $m; trying again next minute" }
        $lastError = $m
    }
    if ($Once) { break }
    Start-Sleep -Seconds $PollSeconds
}
exit 0
