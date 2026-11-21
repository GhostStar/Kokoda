#!/bin/sh
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$project_dir/target}"
if [ -n "${KOKODA_TOOLCHAIN_BIN:-}" ]; then
    export PATH="$KOKODA_TOOLCHAIN_BIN:$PATH"
fi
if command -v cargo >/dev/null 2>&1; then
    exec cargo "$@"
fi
if [ -x "$project_dir/.tools/cargo/bin/rustup" ]; then
    export RUSTUP_HOME="$project_dir/.tools/rustup"
    export CARGO_HOME="$project_dir/.tools/cargo"
    exec "$CARGO_HOME/bin/rustup" run stable cargo "$@"
fi
echo 'Set KOKODA_TOOLCHAIN_BIN to an existing Rust toolchain bin directory, or install Rust locally.' >&2
exit 1
