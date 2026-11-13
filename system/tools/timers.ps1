# Shows, registers or removes AgentFrame's scheduled jobs on this machine. The register-*.ps1
# scripts it calls live in this folder and stay the owners of each job's schedule.
#   .\timers.ps1               # show which of the jobs are registered here
#   .\timers.ps1 -Register     # register them all on this machine, and remove the retired ones
#   .\timers.ps1 -Unregister   # remove them all from this machine, the retired ones too
# The batch task runs the loop watch, bills watch, loop sweep and loop push as the 10:00 run's
# morning jobs, so the four tasks that used to run them are retired and removed here.

[CmdletBinding()]
param(
    [switch]$Register,
    [switch]$Unregister
)

$ErrorActionPreference = 'Stop'

# Each job: the script that registers it and the task name that script uses.
$timers = @(
    @{ Script = 'register-watch-batch.ps1'; Task = 'AgentFrame batch' },
    @{ Script = 'register-weekly-consolidation.ps1'; Task = 'AgentFrame consolidation' },
    @{ Script = 'register-backlog-pass.ps1'; Task = 'AgentFrame backlog pass' }
)

# Task names the batch task replaced. A leftover one would run its job a second time.
$retired = @('AgentFrame loop watch', 'AgentFrame bills watch', 'AgentFrame loop sweep', 'AgentFrame reminders')

if ($Register -and $Unregister) { throw 'Use -Register or -Unregister, not both.' }

$tools = $PSScriptRoot
$failed = 0

function Remove-Retired {
    foreach ($task in $retired) {
        if (Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue) {
            try {
                Unregister-ScheduledTask -TaskName $task -Confirm:$false
                Write-Output "[removed]    $task (retired)"
            } catch {
                Write-Output "[failed]     $task : $($_.Exception.Message)"
                $script:failed++
            }
        }
    }
}

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
        if ($info -and $t.Script -eq 'register-watch-batch.ps1') {
            $live = @($info.Actions | Where-Object { $_.Arguments -match '(^|\s)-SweepLive(\s|$)' }).Count -gt 0
            $mode = if ($live) { ', live sweep: closes loops on proof' } else { ', report-only sweep' }
        }
        if ($info) { Write-Output "[here]       $task ($($info.State)$mode)" } else { Write-Output "[absent]     $task" }
    }
}

if ($Register -or $Unregister) {
    Remove-Retired
} else {
    foreach ($task in $retired) {
        $info = Get-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue
        if ($info) { Write-Output "[retired, still here] $task ($($info.State)); run timers.ps1 -Register or -Unregister to remove it" }
    }
}

if ($failed -gt 0) { Write-Output "$failed job(s) need a look."; exit 1 }
