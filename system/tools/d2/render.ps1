param(
    [Parameter(Mandatory)]
    [string]$InputPath,
    [Parameter(Mandatory)]
    [string]$OutputPath
)

$extension = [IO.Path]::GetExtension($OutputPath).ToLowerInvariant()
if ($extension -ne ".svg") {
    throw "This minimal D2 capability renders SVG only; received '$extension'. Use the owning export route when a final raster, PDF, or animation is actually required."
}

$binary = Join-Path $PSScriptRoot "bin/windows-amd64/d2.exe"
if (-not (Test-Path -LiteralPath $binary)) {
    throw "Vendored D2 binary is missing. Run system/tools/d2/update.ps1 before rendering."
}

$input = (Resolve-Path -LiteralPath $InputPath).Path
$output = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $output
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

& $binary $input $output
if ($LASTEXITCODE -ne 0) {
    throw "D2 failed with exit code $LASTEXITCODE."
}
if (-not (Test-Path -LiteralPath $output) -or (Get-Item -LiteralPath $output).Length -eq 0) {
    throw "D2 completed without creating a non-empty '$output'."
}

Get-Item -LiteralPath $output | Select-Object FullName, Length
