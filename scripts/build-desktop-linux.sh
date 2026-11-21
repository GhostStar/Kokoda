#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
build=${KOKODA_DESKTOP_BUILD:-"$root/target/desktop-linux"}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$root/target"}
cd "$root"
python3 tests/check-locales.py
"$root/scripts/cargo.sh" build -p kokoda-client --release --locked
cmake -S apps/desktop -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build "$build" --parallel "${KOKODA_BUILD_JOBS:-4}"
cp "$CARGO_TARGET_DIR/release/libkokoda_client.so" "$CARGO_TARGET_DIR/release/kokoda-bridge" "$build/"
ctest --test-dir "$build" --output-on-failure
printf '%s\n' "$build/Kokoda"
