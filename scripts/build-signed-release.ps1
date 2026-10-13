param([string]$SigningKeyPath = (Join-Path $env:USERPROFILE 'Documents\WinEase 项目备份\更新签名密钥\winease.key'))
$ErrorActionPreference = 'Stop'
$winEaseRepo = Split-Path -Parent $PSScriptRoot
if (!(Test-Path -LiteralPath $SigningKeyPath)) { throw '找不到更新签名私钥。请按 docs/updating.md 配置。' }
$winEasePreviousKey = $env:TAURI_SIGNING_PRIVATE_KEY
$winEasePreviousPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
Push-Location $winEaseRepo
try {
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -LiteralPath $SigningKeyPath -Raw -Encoding UTF8
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
    npm run tauri -- build --bundles nsis
    if ($LASTEXITCODE -ne 0) { throw '签名构建失败。' }
    $winEaseVersion = (Get-Content package.json -Raw -Encoding UTF8 | ConvertFrom-Json).version
    $winEaseName = "WinEase_${winEaseVersion}_x64-setup.exe"
    $winEaseInstaller = Join-Path $winEaseRepo "src-tauri\target\release\bundle\nsis\$winEaseName"
    $winEaseSignature = "$winEaseInstaller.sig"
    if (!(Test-Path -LiteralPath $winEaseSignature)) { throw '安装包签名未生成。' }
    $winEaseManifest = @{
        version = $winEaseVersion
        notes = "WinEase $winEaseVersion"
        pub_date = [DateTime]::UtcNow.ToString('o')
        platforms = @{'windows-x86_64' = @{
            signature = (Get-Content -LiteralPath $winEaseSignature -Raw -Encoding UTF8).Trim()
            url = "https://github.com/soberbw-hash/WinEase/releases/download/v$winEaseVersion/$winEaseName"
        }}
    }
    $winEaseReleaseDir = Join-Path $winEaseRepo 'output\release'
    New-Item -ItemType Directory -Path $winEaseReleaseDir -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $winEaseReleaseDir 'latest.json'), ($winEaseManifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))
    Copy-Item -LiteralPath $winEaseInstaller,$winEaseSignature -Destination $winEaseReleaseDir -Force
    $winEaseHash = Get-FileHash -LiteralPath $winEaseInstaller -Algorithm SHA256
    "$($winEaseHash.Hash.ToLowerInvariant())  $winEaseName" | Set-Content -LiteralPath (Join-Path $winEaseReleaseDir 'SHA256SUMS.txt') -Encoding ascii
    Write-Output "已生成签名安装包及 latest.json：$winEaseReleaseDir（未发布）"
} finally {
    $env:TAURI_SIGNING_PRIVATE_KEY = $winEasePreviousKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $winEasePreviousPassword
    Pop-Location
}
