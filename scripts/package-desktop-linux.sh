#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
build=${1:-"$root/target/desktop-linux"}
out=${2:-"$root/dist"}
mkdir -p "$out"
out=$(cd "$out" && pwd)
arch=$(dpkg --print-architecture)
name="Kokoda-Client-Linux-$arch-development"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
bundle="$stage/$name"
mkdir -p "$bundle/bin" "$bundle/lib/kokoda" "$bundle/share/applications" "$bundle/share/icons/hicolor/256x256/apps"
cp "$build/Kokoda" "$build/kokoda-bridge" "$build/libkokoda_client.so" "$bundle/lib/kokoda/"
cat > "$bundle/bin/Kokoda" <<'LAUNCHER'
#!/bin/sh
base=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$base/lib/kokoda/Kokoda" "$@"
LAUNCHER
chmod +x "$bundle/bin/Kokoda"
cp "$root/packaging/desktop/Kokoda.desktop" "$bundle/share/applications/"
cp "$root/packaging/icons/kokoda.png" "$bundle/share/icons/hicolor/256x256/apps/kokoda.png"
cp "$root/LICENSE" "$root/THIRD_PARTY.md" "$root/packaging/desktop/README."*.txt "$bundle/"
cp -R "$root/packaging/licenses" "$bundle/"
mkdir -p "$stage/debian"
printf 'Source: kokoda-client\nSection: net\nPriority: optional\nMaintainer: GhostStar <i@ghost.ac>\nStandards-Version: 4.6.2\n\nPackage: kokoda-client\nArchitecture: any\nDescription: Kokoda client\n' > "$stage/debian/control"
deps=$(cd "$stage" && dpkg-shlibdeps -O -e"$bundle/lib/kokoda/Kokoda" -e"$bundle/lib/kokoda/kokoda-bridge" -e"$bundle/lib/kokoda/libkokoda_client.so" | sed -n 's/^shlibs:Depends=//p')
# Qt loads its display backends dynamically. Keep both X11 and Wayland available.
deps="$deps, qt6-qpa-plugins, qt6-wayland"
printf 'Architecture: %s\nDependencies: %s\nBuild: scripts/build-desktop-linux.sh\n' "$arch" "$deps" > "$bundle/BUILD.txt"
pkg="$stage/deb"
mkdir -p "$pkg/opt" "$pkg/usr/bin" "$pkg/usr/share/applications" "$pkg/usr/share/icons/hicolor/256x256/apps" "$pkg/DEBIAN"
cp -R "$bundle" "$pkg/opt/kokoda-client"
cat > "$pkg/usr/bin/Kokoda" <<'INSTALLED'
#!/bin/sh
exec /opt/kokoda-client/lib/kokoda/Kokoda "$@"
INSTALLED
chmod +x "$pkg/usr/bin/Kokoda"
cp "$bundle/share/applications/Kokoda.desktop" "$pkg/usr/share/applications/"
cp "$root/packaging/icons/kokoda.png" "$pkg/usr/share/icons/hicolor/256x256/apps/kokoda.png"
printf 'Package: kokoda-client\nVersion: 0.1.0~dev1\nSection: net\nPriority: optional\nArchitecture: %s\nMaintainer: GhostStar <i@ghost.ac>\nDepends: %s\nDescription: Remote application windows on your local desktop\n' "$arch" "$deps" > "$pkg/DEBIAN/control"
dpkg-deb --root-owner-group --build "$pkg" "$out/$name.deb"
tar -C "$stage" -czf "$out/$name.tar.gz" "$name"
printf '%s\n' "$out/$name.deb" "$out/$name.tar.gz"
