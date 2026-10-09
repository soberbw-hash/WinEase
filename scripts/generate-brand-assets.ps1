$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $projectRoot 'docs/branding/icon-master.png'
$expectedHash = '92979F558C078217D6BAE22AA7A4EE01C73E24204109BB47B19AAA8E80724655'
function Get-SourceHash {
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [BitConverter]::ToString($hasher.ComputeHash([System.IO.File]::ReadAllBytes($sourcePath))).Replace('-', '')
    } finally {
        $hasher.Dispose()
    }
}
if ((Get-SourceHash) -ne $expectedHash) {
    throw 'Brand source changed. Verify the user-provided original before generating assets.'
}
Push-Location $projectRoot
try {
    & npx tauri icon $sourcePath --output src-tauri/icons
    if ($LASTEXITCODE -ne 0) { throw 'Icon generation failed.' }
    Copy-Item -LiteralPath 'src-tauri/icons/128x128@2x.png' -Destination 'public/brand-icon.png' -Force
    Copy-Item -LiteralPath 'src-tauri/icons/32x32.png' -Destination 'public/favicon.png' -Force
    Copy-Item -LiteralPath 'src-tauri/icons/icon.ico' -Destination 'public/favicon.ico' -Force
    if ((Get-SourceHash) -ne $expectedHash) {
        throw 'Icon generation must not modify the original image.'
    }
} finally {
    Pop-Location
}
