param(
    [Parameter(Mandatory)]
    [ValidatePattern("^v?\d+\.\d+\.\d+$")]
    [string]$Version
)

$normalizedVersion = $Version.TrimStart("v")
$archiveName = "d2-v$normalizedVersion-windows-amd64.tar.gz"
$archiveUrl = "https://github.com/terrastruct/d2/releases/download/v$normalizedVersion/$archiveName"
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) "agentframe-d2-$normalizedVersion-$([guid]::NewGuid())"
$archive = Join-Path $temporaryRoot $archiveName
$extractRoot = Join-Path $temporaryRoot "extract"
$targetBinary = Join-Path $PSScriptRoot "bin/windows-amd64/d2.exe"
$targetLicense = Join-Path $PSScriptRoot "LICENSE.txt"

try {
    New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null
    Invoke-WebRequest -Uri $archiveUrl -OutFile $archive
    tar -xzf $archive -C $extractRoot

    $sourceBinary = Get-ChildItem -LiteralPath $extractRoot -Recurse -Filter "d2.exe" | Select-Object -First 1
    $sourceLicense = Get-ChildItem -LiteralPath $extractRoot -Recurse -Filter "LICENSE.txt" | Select-Object -First 1
    if (-not $sourceBinary -or -not $sourceLicense) {
        throw "Release archive did not contain the expected D2 binary and license."
    }

    New-Item -ItemType Directory -Path (Split-Path -Parent $targetBinary) -Force | Out-Null
    Copy-Item -LiteralPath $sourceBinary.FullName -Destination $targetBinary -Force
    Copy-Item -LiteralPath $sourceLicense.FullName -Destination $targetLicense -Force

    $manifest = [ordered]@{
        upstream = "https://github.com/terrastruct/d2"
        version = $normalizedVersion
        platform = "windows-amd64"
        archive_url = $archiveUrl
        archive_sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
    }
    $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $PSScriptRoot "d2-version.json") -Encoding utf8
    & $targetBinary version
}
finally {
    Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
}
