param(
 [Parameter(Mandatory=$true)][string]$QtRoot,
 [string]$FfmpegSdk, [string]$BuildDirectory, [string]$TargetDirectory
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if (!$FfmpegSdk) {$FfmpegSdk=Join-Path $root '.tools\ffmpeg\windows-x64'}
if (!$BuildDirectory) {$BuildDirectory=Join-Path $root 'target\desktop-windows'}
if (!$TargetDirectory) {$TargetDirectory=Join-Path $root 'target'}
$env:CARGO_TARGET_DIR=$TargetDirectory
Push-Location $root
try {
 python tests/check-locales.py
 if ($LASTEXITCODE) {throw 'Locale validation failed'}
 cargo build -p kokoda-client --release --locked --target x86_64-pc-windows-msvc
 if ($LASTEXITCODE) {throw 'Rust client build failed'}
 cmake -S apps/desktop -B $BuildDirectory -G Ninja -DCMAKE_BUILD_TYPE=Release "-DCMAKE_PREFIX_PATH=$QtRoot" "-DKOKODA_FFMPEG_ROOT=$FfmpegSdk"
 if ($LASTEXITCODE) {throw 'CMake configuration failed'}
 cmake --build $BuildDirectory --parallel 4
 if ($LASTEXITCODE) {throw 'Desktop client build failed'}
 foreach($name in @('kokoda_client.dll','kokoda-bridge.exe')) {Copy-Item "$TargetDirectory\x86_64-pc-windows-msvc\release\$name" $BuildDirectory -Force}
 foreach($name in @('avcodec-62.dll','avutil-60.dll','swresample-6.dll','swscale-9.dll')) {Copy-Item "$FfmpegSdk\bin\$name" $BuildDirectory -Force}
 & "$QtRoot\bin\windeployqt.exe" --release --compiler-runtime --no-translations "$BuildDirectory\Kokoda.exe" "$BuildDirectory\kokoda-desktop-tests.exe"
 if ($LASTEXITCODE) {throw 'Qt deployment failed'}
 New-Item -ItemType Directory -Force "$BuildDirectory\platforms" | Out-Null
 Copy-Item "$QtRoot\plugins\platforms\qoffscreen.dll" "$BuildDirectory\platforms" -Force
 ctest --test-dir $BuildDirectory --output-on-failure
 if ($LASTEXITCODE) {throw 'Desktop tests failed'}
 Write-Output "$BuildDirectory\Kokoda.exe"
} finally {Pop-Location}
