param([Parameter(Mandatory=$true)][string]$Destination, [string]$Sdk)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if (!$Sdk) { $Sdk=Join-Path $root '.tools\ffmpeg\windows-x64' }
if (!(Test-Path "$sdk\KOKODA-DEPENDENCY.json")) { throw 'Run python scripts/prepare-ffmpeg.py first' }
$target=Join-Path $Destination 'ffmpeg'
New-Item -ItemType Directory -Force $target | Out-Null
foreach ($name in @('avcodec-62.dll','avutil-60.dll','swresample-6.dll')) {
    Copy-Item "$sdk\bin\$name" $target -Force
}
Copy-Item "$sdk\LICENSE.txt" $target -Force
Copy-Item "$sdk\KOKODA-DEPENDENCY.json" $target -Force
