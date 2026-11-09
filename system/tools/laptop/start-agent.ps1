<#
.SYNOPSIS
    Launcher and watchdog for the always-on AgentFrame session. Runs at logon as the normal user,
    registered by register-agent-logon.ps1.

.DESCRIPTION
    From the vault root (three folders above this script):
      1. Starts Chrome if it is not running, so --chrome can reach the Claude extension.
      2. Runs `af sync` once, then starts the session in its own console window with the command
         line set at the top of this file. Without -Discord the session is reached through Remote
         Control only.
      3. Gives the session its own conversation id, kept in system/logs/start-agent-session-id.txt,
         so a restart reopens this conversation and never one left by a background `claude -p` run
         in the same folder. When the id's transcript exists under the user's
         .claude\projects\<folder>\<id>.jsonl, the start uses --resume <id>; otherwise
         --session-id <id>, with the same id. A new id is made only when the id file is missing
         or holds no valid GUID; deleting the file forces a new conversation.
      4. Watches the session. When its process is gone, starts it again. After -MaxRestarts
         restarts within -RestartWindowMinutes it waits -BackoffMinutes and sends one status
         message through system/tools/notify.py.
      5. Every -SyncMinutes runs `af sync` when af is on the PATH and logs one line:
           exit 0      healthy.
           exit 3      a conflict a chat must resolve: one message per distinct set of
                       SYNC_CONFLICT lines.
           exit 1, "the private layer has no hub remote"
                       not set up yet (`af sync init` not run): logged once, no message.
           otherwise   (hub unreachable, lock held, problems, timeout): logged; one message once
                       -SyncFailuresBeforeNotice syncs in a row have failed. A timeout ends the
                       whole process tree.
    A session this script started (pid and start time in system/logs/start-agent-session.txt) is
    adopted when the watchdog restarts; any other remote-control session is logged and left alone.
    Log: system/logs/start-agent.log. Only one copy runs at a time.

    To stop: end the watchdog first (Stop-ScheduledTask -TaskName 'AgentFrame agent', or close its
    window), then close the session window. Closing the session alone gets it restarted.

.PARAMETER SessionName
    The name the session shows in Remote Control on the phone.

.PARAMETER PermissionMode
    The session's starting permission mode. A permission prompt stalls an unattended session.

.PARAMETER NotifyChannel
    The notify.py --channel prefix for status messages; empty uses notify.py's unprefixed names.

.PARAMETER LockScreen
    Locks the screen once the session has started, for a laptop that signs in by itself.

.PARAMETER Discord
    Adds the Discord direct-message bridge, a research preview, to the session. Use it only once
    the Discord plugin is installed and paired: without the plugin the session exits at once and
    the watchdog backs off.
#>
[CmdletBinding()]
param(
    [string]$SessionName = 'AgentFrame laptop',
    [string]$PermissionMode = 'auto',
    [ValidateRange(1, 1440)][int]$SyncMinutes = 15,
    [ValidateRange(1, 120)][int]$SyncTimeoutMinutes = 10,
    [ValidateRange(1, 100)][int]$SyncFailuresBeforeNotice = 4,
    [ValidateRange(5, 600)][int]$CheckSeconds = 30,
    [ValidateRange(1, 100)][int]$MaxRestarts = 5,
    [ValidateRange(1, 1440)][int]$RestartWindowMinutes = 10,
    [ValidateRange(1, 1440)][int]$BackoffMinutes = 10,
    [ValidateRange(0, 600)][int]$StartDelaySeconds = 20,
    [string]$NotifyChannel = 'reminders',
    [switch]$LockScreen,
    [switch]$Discord
)

# ================================ THE SESSION COMMAND LINE ================================
# The one place to edit when Claude Code's flags change; --channels is a research preview,
# added only with -Discord.
#   First start:  claude <NewSessionArgs> <SessionArgs>     ({id} is a fresh GUID)
#   Restart:      claude <ResumeArgs> <SessionArgs>         ({id} is the saved GUID)
$ClaudeCommand = 'claude'
$SessionArgs = @('--remote-control', $SessionName)
if ($Discord) { $SessionArgs += @('--channels', 'plugin:discord@claude-plugins-official') }
$SessionArgs += @('--chrome', '--permission-mode', $PermissionMode)
$NewSessionArgs = @('--session-id', '{id}')
$ResumeArgs = @('--resume', '{id}')
$SessionMarker = '--remote-control'   # another process whose command line has this is a remote-control session
# ==========================================================================================

$ErrorActionPreference = 'Continue'
$WindowTitle = 'AgentFrame session'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
Set-Location $vault
$env:AF_ROOT = $vault
$logDir = Join-Path $vault 'system\logs'
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }
$log = Join-Path $logDir 'start-agent.log'
$idFile = Join-Path $logDir 'start-agent-session-id.txt'
$pidFile = Join-Path $logDir 'start-agent-session.txt'
$notifyScript = Join-Path $vault 'system\tools\notify.py'

function Write-Log([string]$Message) {
    $line = '{0:yyyy-MM-dd HH:mm:ss} {1}' -f (Get-Date), $Message
    Write-Host $line
    try {
        if ((Test-Path $log) -and ((Get-Item $log).Length -gt 1MB)) { Move-Item -Path $log -Destination "$log.1" -Force }
        Add-Content -Path $log -Value $line -Encoding UTF8
    } catch { }
}

function Write-SmallFile([string]$Path, [string]$Text) {
    [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Send-Status([string]$Title, [string]$Body) {
    # One message to his phone through notify.py; a failure is logged, never fatal.
    $py = Get-Command python -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $py) { Write-Log 'status message not sent: python is not on the PATH'; return }
    $file = Join-Path $logDir 'start-agent-status.md'
    Write-SmallFile $file $Body
    $notifyArgs = @($notifyScript, $file, '--title', $Title)
    if ($NotifyChannel) { $notifyArgs += @('--channel', $NotifyChannel) }
    $out = @(& $py.Source @notifyArgs 2>&1 | ForEach-Object { "$_" } | Where-Object { $_.Trim() })
    $last = ''
    if ($out.Count -gt 0) { $last = $out[$out.Count - 1] }
    Write-Log ("status message '{0}': notify exit {1}: {2}" -f $Title, $LASTEXITCODE, $last)
}

function Format-CmdArg([string]$Value) {
    if ($Value -match '[\s"&|<>^()]') { return '"' + ($Value -replace '"', '""') + '"' }
    return $Value
}

function Start-Chrome {
    if (Get-Process -Name chrome -ErrorAction SilentlyContinue) { return }
    $candidates = @(
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path -LiteralPath $c) {
            Start-Process -FilePath $c | Out-Null
            Write-Log 'chrome started'
            Start-Sleep -Seconds 5
            return
        }
    }
    Write-Log 'chrome not found; the session starts without the browser'
}

# --- Session identity ---------------------------------------------------------------------------

function Get-SavedSessionId {
    if (-not (Test-Path $idFile)) { return $null }
    $text = "$(Get-Content -Path $idFile -Raw -ErrorAction SilentlyContinue)".Trim()
    $guid = [guid]::Empty
    if ([guid]::TryParse($text, [ref]$guid)) { return $guid.ToString() }
    return $null
}

function New-SessionId {
    # Written to a temporary file, then renamed over the id file, so a crash never leaves half an id.
    $id = [guid]::NewGuid().ToString()
    $tmp = "$idFile.tmp"
    Write-SmallFile $tmp $id
    if (Test-Path $idFile) { [System.IO.File]::Replace($tmp, $idFile, $null) } else { [System.IO.File]::Move($tmp, $idFile) }
    return $id
}

function Test-SessionTranscript([string]$Id) {
    # Claude Code keeps each conversation as .claude\projects\<folder>\<id>.jsonl in the user folder.
    $pattern = Join-Path $env:USERPROFILE ".claude\projects\*\$Id.jsonl"
    return [bool](Test-Path -Path $pattern)
}

# --- Session process ----------------------------------------------------------------------------

function Save-SessionProcess($Process) {
    Write-SmallFile $pidFile ("{0} {1}" -f $Process.Id, $Process.StartTime.ToUniversalTime().Ticks)
}

function Find-OwnSession {
    # Adopts only the process this script last started: same pid and same start time.
    if (-not (Test-Path $pidFile)) { return $null }
    $parts = "$(Get-Content -Path $pidFile -Raw -ErrorAction SilentlyContinue)".Trim() -split '\s+'
    if ($parts.Count -ne 2) { return $null }
    $p = Get-Process -Id ([int]$parts[0]) -ErrorAction SilentlyContinue
    if (-not $p) { return $null }
    try {
        $diff = [math]::Abs($p.StartTime.ToUniversalTime().Ticks - [long]$parts[1])
        if ($diff -lt [TimeSpan]::TicksPerSecond) { $null = $p.Handle; return $p }
    } catch { }
    return $null
}

function Write-ForeignSessions($Own) {
    # Other remote-control sessions (a hand start, perhaps elevated) are left alone; logged once.
    $ownIds = @()
    if ($Own) {
        $ownIds += $Own.Id
        $ownIds += @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($Own.Id)" -ErrorAction SilentlyContinue | ForEach-Object { $_.ProcessId })
    }
    $others = @(Get-CimInstance Win32_Process -Filter "CommandLine LIKE '%remote-control%'" -ErrorAction SilentlyContinue |
        Where-Object { $ownIds -notcontains $_.ProcessId -and $_.ProcessId -ne $PID -and $_.CommandLine -match [regex]::Escape($SessionMarker) -and $_.CommandLine -match 'claude' -and $_.Name -notmatch '^powershell' })
    if ($others.Count -gt 0) {
        $list = ($others | ForEach-Object { "$($_.Name) pid $($_.ProcessId)" }) -join ', '
        Write-Log "another remote-control session is running and is left alone: $list. If it also holds the Discord channel, both answer the bot."
    }
}

function Start-Session([bool]$Resume, [string]$Id) {
    Start-Chrome
    $argv = @()
    if ($Resume) { $argv += $ResumeArgs } else { $argv += $NewSessionArgs }
    $argv = @($argv | ForEach-Object { $_ -replace '\{id\}', $Id })
    $argv += $SessionArgs
    $line = (@($ClaudeCommand) + $argv | ForEach-Object { Format-CmdArg $_ }) -join ' '
    # cmd /s /c "...": the window closes when claude exits, so the watchdog sees the exit.
    $cmdLine = '/d /s /c "title ' + $WindowTitle + ' & ' + $line + '"'
    $p = Start-Process -FilePath $env:ComSpec -ArgumentList $cmdLine -WorkingDirectory $vault -PassThru
    $null = $p.Handle   # keeps ExitCode readable after the process ends
    try { Save-SessionProcess $p } catch { Write-Log "could not record the session pid: $($_.Exception.Message)" }
    Write-Log "session started (pid $($p.Id)): $line"
    return $p
}

function Start-SessionByState {
    # The saved id, or a new one when there is none. Its transcript decides the flag: found means
    # --resume, not found means --session-id with the same id. A start that fails keeps the id.
    $id = Get-SavedSessionId
    if (-not $id) {
        $id = New-SessionId
        Write-Log "new session id $id"
    }
    if (Test-SessionTranscript $id) { return Start-Session $true $id }
    Write-Log "no transcript yet for session id $id; starting it with --session-id"
    return Start-Session $false $id
}

# --- Sync ---------------------------------------------------------------------------------------

function Invoke-Sync {
    $af = Get-Command af -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $af) {
        if (-not $script:AfMissingLogged) { Write-Log 'sync skipped: af is not on the PATH'; $script:AfMissingLogged = $true }
        return
    }
    $script:AfMissingLogged = $false
    $outFile = Join-Path $logDir 'start-agent-sync.out'
    $errFile = Join-Path $logDir 'start-agent-sync.err'
    $started = Get-Date
    $code = 'not started'
    try {
        $p = Start-Process -FilePath $af.Source -ArgumentList 'sync' -WorkingDirectory $vault -NoNewWindow -PassThru `
            -RedirectStandardOutput $outFile -RedirectStandardError $errFile
        $null = $p.Handle
        if ($p.WaitForExit($SyncTimeoutMinutes * 60000)) {
            $code = $p.ExitCode
        } else {
            # af may have started git or other children; end the whole tree, not only af.
            $kill = @(& taskkill.exe /T /F /PID $p.Id 2>&1 | ForEach-Object { "$_" } | Where-Object { $_.Trim() })
            Write-Log ("sync timed out; ended its process tree (taskkill exit {0}): {1}" -f $LASTEXITCODE, ($kill -join ' | '))
            $code = "timeout after $SyncTimeoutMinutes min"
        }
    } catch {
        $code = "could not start: $($_.Exception.Message)"
    }
    $secs = [int]((Get-Date) - $started).TotalSeconds
    $errLines = @()
    $outLines = @()
    if (Test-Path $errFile) { $errLines = @(Get-Content -Path $errFile -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() }) }
    if (Test-Path $outFile) { $outLines = @(Get-Content -Path $outFile -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() }) }
    $all = @($outLines) + @($errLines)
    $reason = ''
    if ($errLines.Count -gt 0) { $reason = $errLines[$errLines.Count - 1] } elseif ($outLines.Count -gt 0) { $reason = $outLines[$outLines.Count - 1] }

    # Exit 0: healthy; its last line is "Done.".
    if ("$code" -eq '0') {
        Write-Log "sync ok (${secs}s): $reason"
        if ($script:SyncFailStreak -ge $SyncFailuresBeforeNotice) { Write-Log 'sync healthy again' }
        if ($script:ConflictKey) { Write-Log 'sync conflict resolved' }
        $script:SyncFailStreak = 0
        $script:ConflictKey = ''
        return
    }

    # Exit 3: a chat must resolve the files named on SYNC_CONFLICT lines (library/process/sync.md).
    # One message per distinct set of conflicts, not one per tick.
    if ("$code" -eq '3') {
        $conflicts = @($all | Where-Object { $_ -match '^\s*SYNC_CONFLICT\s' } | ForEach-Object { $_.Trim() } | Sort-Object -Unique)
        $script:SyncFailStreak = 0
        Write-Log ("sync conflict (exit 3, {0}s): {1} file(s): {2}" -f $secs, $conflicts.Count, ($conflicts -join '; '))
        $key = $conflicts -join "`n"
        if ($key -ne $script:ConflictKey) {
            $script:ConflictKey = $key
            Send-Status 'AgentFrame sync conflict' ("On $env:COMPUTERNAME, ``af sync`` stopped on a conflict a chat must resolve (library/process/sync.md):`n`n" + ($conflicts -join "`n") + "`n`nNo more messages until the conflicts change.")
        }
        return
    }

    # Exit 1 because the private layer has no hub remote: this machine has not run `af sync init`
    # yet. Logged once per run, no message; the session keeps running. The full phrase matters: a
    # run can also print "Public code: no hub remote ... Skipped." and then fail for another reason.
    if ("$code" -eq '1' -and (($all -join ' ') -match 'the private layer has no hub remote')) {
        if (-not $script:NotSetUpLogged) {
            Write-Log "sync not set up yet; run af sync init <hub-folder> once on this machine: $reason"
            $script:NotSetUpLogged = $true
        }
        return
    }

    # Anything else (hub unreachable, another sync holding the lock, problems it printed, a timeout):
    # logged every time, one message after -SyncFailuresBeforeNotice failures in a row.
    $script:SyncFailStreak++
    Write-Log ("sync failed (exit {0}, {1}s, {2} in a row): {3}" -f $code, $secs, $script:SyncFailStreak, $reason)
    if ($script:SyncFailStreak -eq $SyncFailuresBeforeNotice) {
        Send-Status 'AgentFrame sync keeps failing' ("On $env:COMPUTERNAME, ``af sync`` has failed $($script:SyncFailStreak) times in a row, every $SyncMinutes minutes. Last: exit $code, $reason`n`nNo more messages until a sync succeeds. Log: system/logs/start-agent.log")
    }
}

# --- Start --------------------------------------------------------------------------------------

$created = $false
$script:Mutex = New-Object System.Threading.Mutex($true, 'Local\AgentFrameStartAgent', [ref]$created)
if (-not $created) { Write-Log 'another start-agent is already running; this copy exits'; exit 0 }

Write-Log "start-agent starting in $vault"
if (-not (Get-Command $ClaudeCommand -ErrorAction SilentlyContinue)) {
    Write-Log "'$ClaudeCommand' is not on the PATH; nothing started"
    Send-Status 'AgentFrame session not started' "On $env:COMPUTERNAME, '$ClaudeCommand' is not on the PATH, so the always-on session did not start."
    exit 1
}
if ($Discord) {
    Write-Log 'Discord bridge on'
    if (-not (Get-Command bun -ErrorAction SilentlyContinue)) { Write-Log 'warning: bun is not on the PATH; the Discord channel needs it' }
} else {
    Write-Log 'Discord bridge off; the session is reached through Remote Control only'
}

if ($StartDelaySeconds -gt 0) { Start-Sleep -Seconds $StartDelaySeconds }   # let the network settle after logon

$script:SyncFailStreak = 0
$script:ConflictKey = ''
$script:NotSetUpLogged = $false
$script:AfMissingLogged = $false
Invoke-Sync
$nextSync = (Get-Date).AddMinutes($SyncMinutes)

$session = Find-OwnSession
if ($session) {
    Write-Log "session started earlier by this script is still running (pid $($session.Id)); watching it"
    $sessionStarted = $session.StartTime
} else {
    $session = Start-SessionByState
    $sessionStarted = Get-Date
}
Write-ForeignSessions $session
if ($LockScreen) { Start-Sleep -Seconds 10; & rundll32.exe 'user32.dll,LockWorkStation'; Write-Log 'screen locked' }

# --- Watch --------------------------------------------------------------------------------------

$restartTimes = New-Object System.Collections.ArrayList
$backoffUntil = $null
$alerted = $false

while ($true) {
    try {
        $now = Get-Date
        if ($session -and $session.HasExited) {
            $code = 'unknown'
            try { if ($null -ne $session.ExitCode) { $code = $session.ExitCode } } catch { }
            $ran = [int]($now - $sessionStarted).TotalSeconds
            Write-Log "session ended (exit $code after ${ran}s)"
            if ($ran -ge ($RestartWindowMinutes * 60)) { $alerted = $false }
            $session = $null
        }

        if (-not $session -and -not ($backoffUntil -and $now -lt $backoffUntil)) {
            $cutoff = $now.AddMinutes(-$RestartWindowMinutes)
            foreach ($t in @($restartTimes)) { if ($t -lt $cutoff) { $restartTimes.Remove($t) } }
            if ($restartTimes.Count -ge $MaxRestarts) {
                $backoffUntil = $now.AddMinutes($BackoffMinutes)
                $restartTimes.Clear()
                Write-Log ("backing off: {0} restarts in {1} minutes; next try at {2:HH:mm}" -f $MaxRestarts, $RestartWindowMinutes, $backoffUntil)
                if (-not $alerted) {
                    $alerted = $true
                    Send-Status 'AgentFrame session keeps stopping' ("On $env:COMPUTERNAME the always-on session stopped $MaxRestarts times in $RestartWindowMinutes minutes. The watchdog waits $BackoffMinutes minutes between tries and sends no more messages until the session stays up. Log: system/logs/start-agent.log")
                }
            } else {
                $backoffUntil = $null
                [void]$restartTimes.Add($now)
                $session = Start-SessionByState
                $sessionStarted = Get-Date
            }
        }

        if ($now -ge $nextSync) {
            Invoke-Sync
            $nextSync = (Get-Date).AddMinutes($SyncMinutes)
        }
    } catch {
        Write-Log "watchdog error: $($_.Exception.Message)"
    }
    Start-Sleep -Seconds $CheckSeconds
}
