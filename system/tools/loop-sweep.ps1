# The loop sweep, as Task Scheduler runs it (register-loop-sweep.ps1). Report-only by default: it
# appends to workspace/life/systems/loop-sweep-report.md and hands overdue loops to the 08:30 push
# (it sends nothing itself), and changes no loop; the closes it would make are in the report.
# -Live also closes the loops an email proves done. By hand:
#   powershell -NoProfile -File system\tools\loop-sweep.ps1 -DryRun   # print what a run would write; write nothing

param(
    [switch]$DryRun,
    [switch]$Live,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest
)

$ErrorActionPreference = 'Stop'

foreach ($a in $Rest) {
    if ($a -eq '--dry-run') { $DryRun = $true } elseif ($a -eq '--live') { $Live = $true } else {
        throw "unknown argument '$a' (use -DryRun or -Live)"
    }
}
if ($DryRun -and $Live) { throw '-DryRun writes nothing, so it does not go with -Live.' }

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $vault
if (-not $env:AF_ROOT) { $env:AF_ROOT = $vault }
$env:PYTHONIOENCODING = 'utf-8'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$pyArgs = @((Join-Path $PSScriptRoot 'loop_sweep.py'))
if ($DryRun) { $pyArgs += '--dry-run' }
if ($Live) { $pyArgs += '--live' }
& python @pyArgs
exit $LASTEXITCODE
