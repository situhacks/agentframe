# The loop watch, as Task Scheduler runs it (register-loop-watch.ps1): checks the due loops against
# his email through the claude.ai Gmail connector and rewrites
# workspace/life/systems/loop-watch-report.md for main to review. It closes and edits no loop.
# Run by hand the same way:
#   powershell -NoProfile -File system\tools\loop-watch.ps1 -DryRun   # read Gmail, print, write nothing

param(
    [switch]$DryRun,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest
)

$ErrorActionPreference = 'Stop'

foreach ($a in $Rest) {
    if ($a -eq '--dry-run') { $DryRun = $true } else { throw "unknown argument '$a' (use -DryRun)" }
}

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $vault
if (-not $env:AF_ROOT) { $env:AF_ROOT = $vault }
$env:PYTHONIOENCODING = 'utf-8'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$pyArgs = @((Join-Path $PSScriptRoot 'loop_watch.py'))
if ($DryRun) { $pyArgs += '--dry-run' }
& python @pyArgs
exit $LASTEXITCODE
