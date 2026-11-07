# Registers the Task Scheduler job "AgentFrame loop watch": loop-watch.ps1 once a day at 07:50,
# after the 07:45 bills watch and ahead of the 08:30 loop push, only while the operator is logged
# on, from the vault root; a missed run starts at the next logon. Rerunning replaces it. Remove with:
#   Unregister-ScheduledTask -TaskName 'AgentFrame loop watch' -Confirm:$false

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $PSScriptRoot 'loop-watch.ps1'
if (-not (Test-Path $script)) { throw "loop-watch.ps1 not found beside this script: $script" }

$taskName = 'AgentFrame loop watch'
$user = "$env:USERDOMAIN\$env:USERNAME"

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
    -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -Daily -At '07:50'
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Hours 7) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Daily check of due loops against his email, into a report main reviews' -Force | Out-Null

# The stub report starts the two-day staleness clock, so a task that dies on its first run is seen.
$env:AF_ROOT = $vault
& python (Join-Path $PSScriptRoot 'loop_watch.py') --init-report
if ($LASTEXITCODE -ne 0) { throw "the task is registered, but the stub report could not be written" }

$info = Get-ScheduledTaskInfo -TaskName $taskName
Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Next run: $($info.NextRunTime)"
