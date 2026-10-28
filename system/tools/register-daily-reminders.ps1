# Registers the Task Scheduler job "AgentFrame reminders": push-due-loops.ps1 every day at
# 08:30, only while the operator is logged on, from the vault root; a missed run starts at
# the next logon. Rerunning replaces it. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame reminders' -Confirm:$false

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'push-due-loops.ps1'
if (-not (Test-Path $script)) { throw "push-due-loops.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame reminders'
$user = "$env:USERDOMAIN\$env:USERNAME"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
    -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Daily -At '08:30'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Minutes 10) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Daily push of due open loops to Discord (af open due)' -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
