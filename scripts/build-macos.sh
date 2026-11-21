#!/bin/sh
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"
./scripts/cargo.sh build -p kokoda-client --release --locked
mkdir -p "$project_dir/dist"
staging=$(mktemp -d "$project_dir/dist/.macos-build.XXXXXXXX")
trap 'rm -rf "$staging"' EXIT
app="$staging/Kokoda.app"
mkdir -p "$app/Contents/MacOS" "$project_dir/.cache/swift"
mkdir -p "$app/Contents/Resources"
cp -R apps/macos/Resources/. "$app/Contents/Resources/"
cp packaging/icons/Kokoda.icns "$app/Contents/Resources/"
cp -R packaging/licenses "$app/Contents/Resources/"
cp THIRD_PARTY.md LICENSE "$app/Contents/Resources/"
cp "${CARGO_TARGET_DIR:-$PWD/target}/release/kokoda-bridge" "$app/Contents/MacOS/"
swiftc -import-objc-header crates/client/include/KokodaCore.h -L "${CARGO_TARGET_DIR:-$PWD/target}/release" -lkokoda_client -framework Security -framework SystemConfiguration -target "$(uname -m)-apple-macos13.0" -module-cache-path "$project_dir/.cache/swift" -framework AppKit platforms/macos/Sources/*.swift apps/macos/Sources/*.swift apps/macos/main.swift -o "$app/Contents/MacOS/Kokoda"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>Kokoda</string>
<key>CFBundleIdentifier</key><string>local.kokoda.development</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleName</key><string>Kokoda</string>
<key>CFBundleIconFile</key><string>Kokoda.icns</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSBonjourServices</key><array><string>_kokoda._tcp</string></array>
<key>NSLocalNetworkUsageDescription</key><string>Discover nearby Kokoda hosts and connect to your selected host to stream authorized application windows and input.</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
# Seal the complete bundle, not just swiftc's automatically signed Mach-O.
./scripts/sign-macos.sh "$app"
# Never truncate the executable of a currently running client. Build and verify
# a new bundle, then replace the directory; the existing process may keep running.
destination="$project_dir/dist/Kokoda.app"
if [ -e "$destination" ]; then mv "$destination" "$staging/previous.app"; fi
if ! mv "$app" "$destination"; then
    if [ -e "$staging/previous.app" ]; then mv "$staging/previous.app" "$destination"; fi
    exit 1
fi
echo "$destination"
