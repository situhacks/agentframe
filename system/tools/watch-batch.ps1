# The batch run, as Task Scheduler runs it (register-watch-batch.ps1) at 10:00, 15:00 and 19:00:
# sweeps expired watch items, checks every due item in workspace/life/watch.md, raises a loop and
# sends a phone message for each one that is met, and writes system/logs/watch-batch/<date-time>.md.
# The first run from 09:55 each day also runs the bills watch, the loop watch, the loop sweep and the
# loop push, once. -SweepLive lets the morning loop sweep close loops on proof. By hand:
#   powershell -NoProfile -File system\tools\watch-batch.ps1 -DryRun   # check, print the report; write and send nothing

param(
    [switch]$DryRun,
    [switch]$SweepLive,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest
)

$ErrorActionPreference = 'Stop'

foreach ($a in $Rest) {
    if ($a -eq '--dry-run') { $DryRun = $true } elseif ($a -eq '--sweep-live') { $SweepLive = $true } else {
        throw "unknown argument '$a' (use -DryRun or -SweepLive)"
    }
}

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $vault
if (-not $env:AF_ROOT) { $env:AF_ROOT = $vault }
$env:PYTHONIOENCODING = 'utf-8'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$pyArgs = @((Join-Path $PSScriptRoot 'watch_batch.py'))
if ($DryRun) { $pyArgs += '--dry-run' }
if ($SweepLive) { $pyArgs += '--sweep-live' }
& python @pyArgs
exit $LASTEXITCODE
