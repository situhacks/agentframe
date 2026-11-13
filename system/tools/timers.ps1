# Shows, registers or removes AgentFrame's scheduled jobs on this machine. The register-*.ps1
# scripts it calls live in this folder and stay the owners of each job's schedule.
#   .\timers.ps1               # show which of the jobs are registered here
#   .\timers.ps1 -Register     # register them all on this machine
#   .\timers.ps1 -Unregister   # remove them all from this machine

[CmdletBinding()]
param(
    [switch]$Register,
    [switch]$Unregister
)

$ErrorActionPreference = 'Stop'

# Each job: the script that registers it and the task name that script uses.
$timers = @(
    @{ Script = 'register-loop-watch.ps1'; Task = 'AgentFrame loop watch' },
    @{ Script = 'register-loop-sweep.ps1'; Task = 'AgentFrame loop sweep' },
    @{ Script = 'register-bills-watch.ps1'; Task = 'AgentFrame bills watch' },
    @{ Script = 'register-daily-reminders.ps1'; Task = 'AgentFrame reminders' },
    @{ Script = 'register-weekly-consolidation.ps1'; Task = 'AgentFrame consolidation' },
    @{ Script = 'register-backlog-pass.ps1'; Task = 'AgentFrame backlog pass' }
)

if ($Register -and $Unregister) { throw 'Use -Register or -Unregister, not both.' }

$tools = $PSScriptRoot
$failed = 0

foreach ($t in $timers) {
    $task = $t.Task
    $path = Join-Path $tools $t.Script
    if ($Register) {
        if (-not (Test-Path $path)) { Write-Output "[missing]    $task : $($t.Script) not found in $tools"; $failed++; continue }
        try {
            & $path | ForEach-Object { Write-Output "             $_" }
            Write-Output "[registered] $task"
        } catch {
            Write-Output "[failed]     $task : $($_.Exception.Message)"
            $failed++
        }
    } elseif ($Unregister) {
        if (Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue) {
            try {
                Unregister-ScheduledTask -TaskName $task -Confirm:$false
                Write-Output "[removed]    $task"
            } catch {
                Write-Output "[failed]     $task : $($_.Exception.Message)"
                $failed++
            }
        } else {
            Write-Output "[absent]     $task"
        }
    } else {
        $info = Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue
        $mode = ''
        if ($info -and $t.Script -eq 'register-loop-sweep.ps1') {
            $live = @($info.Actions | Where-Object { $_.Arguments -match '(^|\s)-Live(\s|$)' }).Count -gt 0
            $mode = if ($live) { ', live: closes loops on proof' } else { ', report-only' }
        }
        if ($info) { Write-Output "[here]       $task ($($info.State)$mode)" } else { Write-Output "[absent]     $task" }
    }
}

if ($failed -gt 0) { Write-Output "$failed job(s) need a look."; exit 1 }
