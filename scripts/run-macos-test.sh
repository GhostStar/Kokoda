#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
suite=${1:?Usage: run-macos-test.sh SUITE [arguments]}
shift
case "$suite" in
    PasswordLiveTests|PopupLiveTests|WindowManagementLiveTests)
        [ "$#" -eq 3 ] || { echo "Usage: $0 PasswordLiveTests BRIDGE ADDRESS SECRET_JSON" >&2; exit 2; } ;;
    ClipboardTests|ClipboardImageTests)
        [ "$#" -eq 4 ] || { echo "Usage: $0 $suite BRIDGE ADDRESS PAIRING RESULT_FILE" >&2; exit 2; } ;;
    VideoTests)
        [ "$#" -eq 2 ] || [ "$#" -eq 7 ] || { echo "Usage: $0 VideoTests CAPTURE_JSONL OUTPUT_PNG [X Y R G B]" >&2; exit 2; } ;;
esac
[ -f "tests/platform/macos/$suite.swift" ] || { echo "Unknown test suite: $suite" >&2; exit 2; }
./scripts/cargo.sh build -p kokoda-client --lib --locked
mkdir -p .cache/swift
build=$(mktemp -d ".cache/test.XXXXXXXX")
trap 'rm -rf "$build"' EXIT
{
    echo 'import AppKit'; echo 'import VideoToolbox'; echo 'import ImageIO'; echo 'import CoreImage'
    # Ignore crash-restoration UI only for this test process, without changing
    # persistent defaults or the suite's positional command-line arguments.
    echo 'UserDefaults.standard.setVolatileDomain(UserDefaults.standard.volatileDomain(forName:UserDefaults.argumentDomain).merging(["ApplePersistenceIgnoreState":true,"NSQuitAlwaysKeepsWindows":false]){_,new in new},forName:UserDefaults.argumentDomain)'
    cat "tests/platform/macos/$suite.swift"
} > "$build/main.swift"
app="$build/Suite.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if [ "$suite" = PasswordLiveTests ] || [ "$suite" = PopupLiveTests ] || [ "$suite" = WindowManagementLiveTests ]; then
    # Reuse the caller's stable executable identity for per-program network
    # permissions instead of creating a new executable in every random bundle.
    bridge_dir=$(cd "$(dirname "$1")" && pwd)
    ln -s "$bridge_dir/$(basename "$1")" "$app/Contents/MacOS/kokoda-bridge"
fi
cp -R apps/macos/Resources/. "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>suite</string>
<key>CFBundleIdentifier</key><string>local.kokoda.tests</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
swiftc -import-objc-header crates/client/include/KokodaCore.h -L "${CARGO_TARGET_DIR:-$PWD/target}/debug" -lkokoda_client -framework Security -framework SystemConfiguration -module-cache-path .cache/swift platforms/macos/Sources/*.swift apps/macos/Sources/*.swift "$build/main.swift" -o "$app/Contents/MacOS/suite"
if [ -n "${KOKODA_TEST_LANGUAGE:-}" ]; then
    "$app/Contents/MacOS/suite" "$@" -AppleLanguages "($KOKODA_TEST_LANGUAGE)"
else
    "$app/Contents/MacOS/suite" "$@"
fi
