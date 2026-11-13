# The bills watch, as the 10:00 batch run starts it (watch_batch.py): reads his card emails
# through the claude.ai Gmail connector, updates workspace/life/systems/finances/bills.md, and
# raises, re-dates and closes the Bills loops that the morning loop push carries to his phone.
# Run by hand the same way:
#   powershell -NoProfile -File system\tools\bills-watch.ps1 -DryRun   # read Gmail, print, write nothing

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

$pyArgs = @((Join-Path $PSScriptRoot 'bills_watch.py'))
if ($DryRun) { $pyArgs += '--dry-run' }
& python @pyArgs
exit $LASTEXITCODE
