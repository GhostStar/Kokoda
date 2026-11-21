#!/bin/sh
# Package the supplied unframed artwork with normalized visual bounds.
set -eu
cd "$(dirname "$0")/.."
source=docs/branding/kokoda-logo-unframed.png
mkdir -p packaging/icons .cache
build=$(mktemp -d .cache/icons.XXXXXXXX)
trap 'rm -rf "$build"' EXIT
mkdir -p .cache/swift
swift -module-cache-path .cache/swift scripts/prepare-icon.swift "$source" packaging/icons/kokoda-master.png
source=packaging/icons/kokoda-master.png
iconset="$build/Kokoda.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$source" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    doubled=$((size * 2))
    sips -z "$doubled" "$doubled" "$source" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o packaging/icons/Kokoda.icns
cp "$iconset/icon_512x512.png" packaging/icons/kokoda.png
for size in 16 20 24 32 40 48 64 96 128 256; do
    sips -z "$size" "$size" "$source" --out "$build/$size.png" >/dev/null
done
# ICO is a directory of PNG payloads; keep the native resampler's pixels intact.
python3 - "$build" <<'PY'
from pathlib import Path
import struct,sys
sizes=(16,20,24,32,40,48,64,96,128,256)
images=[(Path(sys.argv[1])/f'{s}.png').read_bytes() for s in sizes]
offset=6+16*len(sizes)
headers=[]
for size,data in zip(sizes,images):
    headers.append(struct.pack('<BBBBHHII',size%256,size%256,0,0,1,32,len(data),offset))
    offset+=len(data)
Path('packaging/icons/Kokoda.ico').write_bytes(struct.pack('<HHH',0,1,len(sizes))+b''.join(headers)+b''.join(images))
PY
printf '%s\n' 'Generated macOS ICNS, Windows ICO and Linux PNG from the selected artwork.'
