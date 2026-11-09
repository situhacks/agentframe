# Registers the Task Scheduler job "AgentFrame loop sweep": loop-sweep.ps1 once a day at 08:20,
# after the 07:45 bills watch and the 07:50 loop watch, whose report it reads, and ahead of the
# 08:30 loop push, so the push carries the swept list. Only while the operator is logged on, from
# the vault root; a missed run starts at the next logon. Rerunning replaces it. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame loop sweep' -Confirm:$false

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'loop-sweep.ps1'
if (-not (Test-Path $script)) { throw "loop-sweep.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame loop sweep'
$user = "$env:USERDOMAIN\$env:USERNAME"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
    -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Daily -At '08:20'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Minutes 30) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Daily sweep of the open loops: close on evidence, remind on the rest' -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
