# Registers the Task Scheduler job "AgentFrame night mute": night_mute.ps1 daily at 23:30, only while
# the operator is logged on. It watches until 06:00 and mutes the default speakers once he has been idle
# 20 minutes (every active output device); it never unmutes. A start the machine slept through runs when it wakes, and the script
# exits at once if that is outside the window. Rerunning replaces it. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame night mute' -Confirm:$false
param([string]$At = '23:30', [int]$IdleMinutes = 20)

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'night_mute.ps1'
if (-not (Test-Path $script)) { throw "night_mute.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame night mute'
$user = "$env:USERDOMAIN\$env:USERNAME"
$argument = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`" -Start $At -IdleMinutes $IdleMinutes"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $argument -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Daily -At $At
# A start boundary with no time-zone offset keeps the start at local time across daylight saving changes.
$trigger.StartBoundary = (Get-Date $At).ToString('s')
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 8) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description "Mutes the speakers at night once he has been idle $IdleMinutes minutes; never unmutes" -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, daily at $At, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
