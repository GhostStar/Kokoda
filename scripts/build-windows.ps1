param([switch]$Release, [string]$ToolRoot, [string]$TargetDir, [string]$FfmpegSdk)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$project = Split-Path $PSScriptRoot -Parent
if ($ToolRoot) {
    $env:RUSTUP_HOME = "$ToolRoot\rustup"
    $env:CARGO_HOME = "$ToolRoot\cargo"
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
if (!$TargetDir) { $TargetDir = Join-Path $project 'target' }
$env:CARGO_TARGET_DIR = $TargetDir
if (!$FfmpegSdk) { $FfmpegSdk = Join-Path $project '.tools\ffmpeg\windows-x64' }
$env:KOKODA_FFMPEG_INCLUDE = Join-Path $FfmpegSdk 'include'
if (!(Test-Path "$env:KOKODA_FFMPEG_INCLUDE\libavcodec\avcodec.h")) {
    throw 'Run python scripts/prepare-ffmpeg.py or pass -FfmpegSdk with an existing pinned SDK.'
}
Push-Location $project
try {
    & cargo test --workspace --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed' }
    $buildOptions = @()
    if ($Release) { $buildOptions += '--release' }
    $profile = if ($Release) { 'release' } else { 'debug' }
    & cargo build -p kokoda-windows-host -p kokoda-host-settings-windows -p kokoda-client --locked --target x86_64-pc-windows-msvc @buildOptions
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    & "$PSScriptRoot\stage-ffmpeg.ps1" -Sdk $FfmpegSdk -Destination "$TargetDir\x86_64-pc-windows-msvc\$profile"
} finally { Pop-Location }
