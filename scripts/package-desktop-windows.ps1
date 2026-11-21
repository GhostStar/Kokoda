param(
 [Parameter(Mandatory=$true)][string]$BinaryDirectory,
 [Parameter(Mandatory=$true)][string]$QtRoot,
 [Parameter(Mandatory=$true)][string]$FfmpegSdk,
 [string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if (!$OutputDirectory) {$OutputDirectory=Join-Path $root 'dist'}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$stage=Join-Path $OutputDirectory ('.client-package-'+[Guid]::NewGuid().ToString('N'))
$name='Kokoda-Client-Windows-x64-development'
$bundle=Join-Path $stage $name
try {
 New-Item -ItemType Directory -Force $bundle | Out-Null
 foreach($file in @('Kokoda.exe','kokoda-bridge.exe','kokoda_client.dll','avcodec-62.dll','avutil-60.dll','swresample-6.dll','swscale-9.dll')) {Copy-Item "$BinaryDirectory\$file" $bundle}
 & "$QtRoot\bin\windeployqt.exe" --release --compiler-runtime --no-translations "$bundle\Kokoda.exe"
 if ($LASTEXITCODE) {throw 'Qt package deployment failed'}
 if ($env:VCToolsRedistDir) {
   $crt=Get-ChildItem "$env:VCToolsRedistDir\x64" -Directory -Filter 'Microsoft.VC*.CRT' | Select-Object -First 1
   if ($crt) {Copy-Item "$($crt.FullName)\*.dll" $bundle}
 }
 foreach($file in @('LICENSE','THIRD_PARTY.md')) {Copy-Item "$root\$file" $bundle}
 Copy-Item "$root\packaging\licenses" "$bundle\licenses" -Recurse
 foreach($file in @('README.en.txt','README.zh-CN.txt')) {Copy-Item "$root\packaging\desktop\$file" $bundle}
 New-Item -ItemType Directory -Force "$bundle\licenses\ffmpeg" | Out-Null
 Copy-Item "$FfmpegSdk\LICENSE.txt","$FfmpegSdk\KOKODA-DEPENDENCY.json" "$bundle\licenses\ffmpeg"
 $qtVersion=& "$QtRoot\bin\qmake.exe" -query QT_VERSION
 @("Qt: $qtVersion","Qt source: https://github.com/qt/qtbase/tree/v$qtVersion","FFmpeg: $((Get-Content "$FfmpegSdk\KOKODA-DEPENDENCY.json" -Raw|ConvertFrom-Json).version)",'Architecture: x86_64-pc-windows-msvc','Build: scripts/build-desktop-windows.ps1') | Set-Content "$bundle\BUILD.txt"
 Compress-Archive -Path $bundle -DestinationPath "$OutputDirectory\$name.zip" -Force
 Write-Output "$OutputDirectory\$name.zip"
} finally {if(Test-Path $stage){Remove-Item $stage -Recurse -Force}}
