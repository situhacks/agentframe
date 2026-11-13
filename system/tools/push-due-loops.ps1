# The daily reminder push, as the 10:00 batch run ends with it (watch_batch.py): due open
# loops to his phone through push_due_loops.py and notify.py, from the vault root.
# Nothing due: exits 0 without a word. Run by hand the same way:
#   powershell -NoProfile -File system\tools\push-due-loops.ps1 -DryRun   # print, send nothing

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

$pyArgs = @((Join-Path $PSScriptRoot 'push_due_loops.py'))
if ($DryRun) { $pyArgs += '--dry-run' }
& python @pyArgs
exit $LASTEXITCODE
