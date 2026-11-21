param([Parameter(Mandatory=$true)][string]$HostExecutable, [string]$FfmpegBin)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$bin=if ($FfmpegBin) {$FfmpegBin} else {Join-Path $project '.tools\ffmpeg\windows-x64\bin'}
$stream=Join-Path ([System.IO.Path]::GetTempPath()) ("kokoda-ffmpeg-"+[guid]::NewGuid().ToString()+'.h264')
try {
    & $HostExecutable --probe-ffmpeg-software $stream
    if ($LASTEXITCODE -ne 0) { throw 'FFmpeg bridge probe failed' }
    & "$bin\ffmpeg.exe" -v error -xerror -i $stream -f null -
    if ($LASTEXITCODE -ne 0) { throw 'H264 decoding failed' }
    $result=& "$bin\ffprobe.exe" -v error -count_frames -select_streams v:0 -show_entries stream=width,height,nb_read_frames -of json $stream
    if ($LASTEXITCODE -ne 0) { throw 'H264 stream inspection failed' }
    $video=($result | ConvertFrom-Json).streams[0]
    if ($video.width -ne 1280 -or $video.height -ne 720 -or $video.nb_read_frames -ne '60') {
        throw "Incorrect encoded dimensions/frame count: $result"
    }
    Write-Output 'PASS: FFmpeg bridge encoded and drained 60 frames; all decoded at 1280x720'
} finally {
    Remove-Item -LiteralPath $stream -Force -ErrorAction SilentlyContinue
}
