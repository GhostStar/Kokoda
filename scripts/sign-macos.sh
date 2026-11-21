#!/bin/sh
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app="${1:-$project_dir/dist/Kokoda.app}"
# Local development signature, not a Developer ID certificate or notarization.
codesign --force --sign - --identifier local.kokoda.development.bridge "$app/Contents/MacOS/kokoda-bridge"
codesign --force --sign - --identifier local.kokoda.development "$app"
codesign --verify --deep --strict "$app"
