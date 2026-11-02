# Registers the Task Scheduler job "AgentFrame bills watch": bills-watch.ps1 every day at 07:45,
# ahead of the 08:30 loop push, only while the operator is logged on, from the vault root; a
# missed run starts at the next logon. Rerunning replaces it. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame bills watch' -Confirm:$false

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'bills-watch.ps1'
if (-not (Test-Path $script)) { throw "bills-watch.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame bills watch'
$user = "$env:USERDOMAIN\$env:USERNAME"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
    -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Daily -At '07:45'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 3) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Daily read of card emails into bills.md and the Bills loops' -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
