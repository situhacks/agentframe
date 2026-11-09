# The loop sweep, as Task Scheduler runs it (register-loop-sweep.ps1): closes the loops evidence
# settles, merges and de-duplicates, re-dates idle to-dos, hands overdue money and deadline loops
# to the 08:30 push (it sends nothing itself), regenerates the Discord #open board, and appends to
# workspace/life/systems/loop-sweep-report.md. Run by hand the same way:
#   powershell -NoProfile -File system\tools\loop-sweep.ps1 -DryRun   # plan and print; change nothing

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

$pyArgs = @((Join-Path $PSScriptRoot 'loop_sweep.py'))
if ($DryRun) { $pyArgs += '--dry-run' }
& python @pyArgs
exit $LASTEXITCODE
