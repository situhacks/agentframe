# Registers the Task Scheduler job "AgentFrame batch": watch-batch.ps1 daily at 10:00, 15:00 and
# 19:00, only while the operator is logged on, from the vault root. It replaces the loop watch, bills
# watch, loop sweep and reminders tasks: the 10:00 run also does their jobs, once a day, in order.
# A run the machine slept through starts at the next time it is available, and the morning jobs
# still run once if that is after 09:55. -SweepLive registers the run whose loop sweep also closes
# loops on proof. Rerunning replaces it; timers.ps1 shows the mode. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame batch' -Confirm:$false

param([switch]$SweepLive)

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'watch-batch.ps1'
if (-not (Test-Path $script)) { throw "watch-batch.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame batch'
$user = "$env:USERDOMAIN\$env:USERNAME"
$mode = if ($SweepLive) { 'live sweep' } else { 'report-only sweep' }
$argument = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`""
if ($SweepLive) { $argument += ' -SweepLive' }

$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $argument -WorkingDirectory $vault
$triggers = @(
    New-ScheduledTaskTrigger -Daily -At '10:00'
    New-ScheduledTaskTrigger -Daily -At '15:00'
    New-ScheduledTaskTrigger -Daily -At '19:00'
)
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 3) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $triggers -Principal $principal `
    -Settings $settings -Description "Three daily checks of the watch queue; the 10:00 run also does the morning jobs ($mode)" -Force | Out-Null

# The stub report starts the loop watch's staleness clock, so a task that dies on its first run is seen.
$env:AF_ROOT = $vault
& python (Join-Path $PSScriptRoot 'loop_watch.py') --init-report
if ($LASTEXITCODE -ne 0) { throw "the task is registered, but the loop watch's stub report could not be written" }

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' ($mode) for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
