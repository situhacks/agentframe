# Registers the Task Scheduler job "AgentFrame consolidation": consolidate-weekly.ps1 every
# Sunday at 18:00, only while the operator is logged on, from the vault root. Rerunning replaces it.

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'consolidate-weekly.ps1'
if (-not (Test-Path $script)) { throw "consolidate-weekly.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame consolidation'
$user = "$env:USERDOMAIN\$env:USERNAME"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
    -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Weekly -DaysOfWeek Sunday -At '18:00'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Hours 2) `
    -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Weekly consolidation of workspace/life/ by the consolidator role' -Force | Out-Null

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
