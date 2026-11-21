param([Parameter(Mandatory=$true)][string]$BinaryDirectory,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if (!$OutputDirectory) {$OutputDirectory=Join-Path $root 'dist'}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$stage=Join-Path $OutputDirectory ('.package-'+[Guid]::NewGuid().ToString('N'))
$name='Kokoda-Windows-x64-development'
$bundle=Join-Path $stage $name
try {
 New-Item -ItemType Directory -Force "$bundle\ffmpeg" | Out-Null
 foreach($file in @('KokodaHost.exe','kokoda-windows-host.exe','kokoda-bridge.exe')) {Copy-Item (Join-Path $BinaryDirectory $file) $bundle}
 foreach($file in @('avcodec-62.dll','avutil-60.dll','swresample-6.dll','LICENSE.txt','KOKODA-DEPENDENCY.json')) {Copy-Item "$BinaryDirectory\ffmpeg\$file" "$bundle\ffmpeg"}
 foreach($file in @('README.en.txt','README.zh-CN.txt','sharing.example.json')) {Copy-Item "$root\packaging\windows\$file" $bundle}
 foreach($file in @('LICENSE','THIRD_PARTY.md')) {Copy-Item "$root\$file" $bundle}
 Copy-Item "$root\packaging\licenses" "$bundle\licenses" -Recurse
 # Only the explicitly staged files enter the archive, never a working identity directory.
 Compress-Archive -Path $bundle -DestinationPath "$OutputDirectory\$name.zip" -Force
 Write-Output "$OutputDirectory\$name.zip"
} finally {if(Test-Path $stage){Remove-Item $stage -Recurse -Force}}
