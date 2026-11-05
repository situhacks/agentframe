# Registers the Task Scheduler job "AgentFrame backlog pass": backlog-pass.ps1 every Sunday at
# 20:00, two hours after the weekly consolidation, only while the operator is logged on, from
# the vault root. Rerunning replaces it.

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'backlog-pass.ps1'
if (-not (Test-Path $script)) { throw "backlog-pass.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame backlog pass'
$user = "$env:USERDOMAIN\$env:USERNAME"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
    -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Weekly -DaysOfWeek Sunday -At '20:00'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Minutes 5) `
    -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Weekly loop naming the backlog rows ready for a reviewed builder' -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
