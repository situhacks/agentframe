# Registers the Task Scheduler job "AgentFrame loop sweep": loop-sweep.ps1 once a day at 08:20,
# after the 07:45 bills watch and the 07:50 loop watch, whose report it reads, and ahead of the
# 08:30 loop push, so the push carries the overdue list. Report-only unless -Live: it writes the
# report and the overdue handoff and changes no loop. -Live registers the run that also closes
# loops on email proof. Only while the operator is logged on, from the vault root; a missed
# run starts at the next logon. Rerunning replaces it; laptop/timers.ps1 shows the mode. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame loop sweep' -Confirm:$false

param([switch]$Live)

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'loop-sweep.ps1'
if (-not (Test-Path $script)) { throw "loop-sweep.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame loop sweep'
$user = "$env:USERDOMAIN\$env:USERNAME"
$mode = if ($Live) { 'live' } else { 'report-only' }
$argument = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`""
if ($Live) { $argument += ' -Live' }

$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $argument -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Daily -At '08:20'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Minutes 30) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description "Daily sweep of the open loops ($mode): close on proof, report the rest" -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' ($mode) for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
