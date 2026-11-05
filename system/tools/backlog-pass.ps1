# The weekly backlog pass: raises a loop naming the rows af backlog ready lists, for main to
# dispatch to a reviewed builder. backlog_pass.py owns the run; this passes the arguments through.
# -DryRun prints the loop it would raise.

param(
    [int]$MaxRows = 3,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $vault
$env:AF_ROOT = $vault
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }
$env:PYTHONIOENCODING = 'utf-8'

$pass = @((Join-Path $PSScriptRoot 'backlog_pass.py'), '--max-rows', "$MaxRows")
if ($DryRun) { $pass += '--dry-run' }

$ErrorActionPreference = 'Continue'
& python @pass
exit $LASTEXITCODE
