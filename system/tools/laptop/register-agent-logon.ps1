# Registers the Task Scheduler job "AgentFrame agent": start-agent.ps1 at logon of the current user,
# interactive and not elevated, in a minimized window, from the vault root, with no time limit.
# Runs in a normal PowerShell: the job is the current user's, at limited run level.
# Rerunning replaces it. Options given here are passed on to start-agent.ps1.
#   .\register-agent-logon.ps1                                   # register; Remote Control only
#   .\register-agent-logon.ps1 -PermissionMode acceptEdits -LockScreen
#   .\register-agent-logon.ps1 -Discord                          # add the Discord bridge, once the plugin is paired
#   .\register-agent-logon.ps1 -Unregister                       # remove the job
# Removing the job stops nothing already running: end the watchdog, then close the session window.

[CmdletBinding()]
param(
    [switch]$Unregister,
    [ValidateNotNullOrEmpty()][string]$SessionName,
    [ValidateNotNullOrEmpty()][string]$PermissionMode,
    [ValidateRange(1, 1440)][int]$SyncMinutes,
    [ValidateNotNullOrEmpty()][string]$NotifyChannel,
    [switch]$LockScreen,
    [switch]$Discord
)

$ErrorActionPreference = 'Stop'

$taskName = 'AgentFrame agent'

if ($Unregister) {
    if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
        Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
        Write-Output "Removed '$taskName'."
    } else {
        Write-Output "'$taskName' is not registered; nothing to remove."
    }
    return
}

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$script = Join-Path $PSScriptRoot 'start-agent.ps1'
if (-not (Test-Path $script)) { throw "start-agent.ps1 not found beside this script: $script" }

$user = "$env:USERDOMAIN\$env:USERNAME"

# Only the options given are passed on, each as -Name "value"; an option left out leaves no trace,
# so start-agent.ps1 uses its own default.
$argLine = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Minimized -File `"$script`""
foreach ($name in @('SessionName', 'PermissionMode', 'SyncMinutes', 'NotifyChannel')) {
    if (-not $PSBoundParameters.ContainsKey($name)) { continue }
    $value = "$($PSBoundParameters[$name])".Trim()
    if (-not $value) { throw "-$name needs a value; leave it out to use start-agent.ps1's default" }
    if ($value -match '"') { throw "-$name may not contain a double quote" }
    $argLine += " -$name `"$value`""
}
if ($LockScreen) { $argLine += ' -LockScreen' }
if ($Discord) { $argLine += ' -Discord' }

$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $argLine -WorkingDirectory $vault
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew `
    -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 5) -Priority 4   # 4 is normal; the default 7 is below normal

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description 'Always-on AgentFrame session and its watchdog, started at logon' -Force | Out-Null

Write-Output "Registered '$taskName' for $user, working directory $vault"
Write-Output "Runs at your next logon: powershell.exe $argLine"
if ($Discord) {
    Write-Output 'Discord bridge: on. The Discord plugin must be installed and paired, or the session exits at once.'
} else {
    Write-Output 'Discord bridge: off; the session is reached through Remote Control only. Rerun with -Discord to add it.'
}
Write-Output "Start it now without signing out: Start-ScheduledTask -TaskName '$taskName'"
