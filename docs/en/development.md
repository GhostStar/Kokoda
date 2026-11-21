# Development runtime

[简体中文](../zh-CN/development.md)

The first implementation slice migrates the verified Windows-host/macOS-client path into Kokoda's new workspace. It is a development build. The release requirements in the product and sharing specifications remain the target.

## What runs

- `kokoda-platform-windows`: WGC capture, FFmpeg NVENC/AMF/QSV with Media Foundation fallback, physical-pixel coordinate handling, input and text/image clipboard adapters.
- `kokoda-host`: application groups and per-application sessions, streaming/control scheduling, recovery and admission cleanup. The migrated runtime is behind `windows-runtime`; the executable composes it.
- `kokoda-protocol`: development command names, routing/input validation and common quality presets. `kokoda-transport`: verified TLS, out-of-band pinned identity/token and bounded framing. These endpoints speak Kokoda's `dev-1` protocol.
- `kokoda-client`: tested shared viewer lifecycle, a C ABI used by the macOS runner, global quality effects and a network bridge executable. The native runner still owns scheduling and recovery integration; those workflows will move into the shared core incrementally.
- `platforms/macos`: native presentation, VideoToolbox, IME and clipboard image handling. `apps/macos`: runner and development settings. `apps/desktop/locales`: canonical English/Chinese resources; generate native resources with `python3 scripts/generate-locales.py`.

## Boundaries still being implemented

The explicit legacy development host defaults to an empty read-only catalog with `--serve-development ADDRESS IDENTITY`. Add `--sharing-policy FILE` for [selected-window viewing](restricted-sharing.md), or explicitly add `--allow-all-windows` for full control of all application windows. This legacy fixture path uses a manually transferred certificate and random token. Normal GUI connections use [password authentication](password-authentication.md). The explicit flag is local consent for development access; it is not the product's restricted sharing mode.

Expiring pairing codes, per-device approval/revocation, interactive restricted sharing, live policy updates, generation-bound discovery, explicit busy responses are pending. All-window mode retains the prototype's application-membership rules. Restricted viewing filters these groups by host policy and disables input/clipboard. Do not treat application grouping alone as privacy isolation. Windows/Linux GUI clients and macOS hosting are pending. No first release is claimed.

The media interfaces are reserved for a fuller adapter contract. The working Windows worker uses its native owned-frame type; macOS playback uses its native decoder. Do not introduce RustDesk endpoint compatibility or broaden a capture fallback to the desktop.

## Build

Use an existing Rust toolchain on PATH. `scripts/cargo.sh` also accepts `KOKODA_TOOLCHAIN_BIN`, `CARGO_HOME` and `CARGO_TARGET_DIR`; it never installs a toolchain globally.

```sh
./scripts/cargo.sh test --workspace --locked
python3 tests/check-locales.py
./scripts/build-macos.sh
```

The macOS output is `dist/Kokoda.app`. Its local development signature is not a notarized distribution signature. macOS 13+ and Xcode command-line tools are required.

On Windows, install Rust's MSVC target and Visual Studio C++ build tools, then prepare the pinned LGPL FFmpeg SDK:

```powershell
python scripts/prepare-ffmpeg.py
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 -Release
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -BinaryDirectory target/x86_64-pc-windows-msvc/release
```

An existing isolated toolchain can be reused with `-ToolRoot`, the output directory with `-TargetDir`, and an existing matching FFmpeg SDK with `-FfmpegSdk`. The script runs workspace tests, builds x86_64 MSVC binaries and stages replaceable FFmpeg DLLs next to the host. Vendor GPU drivers are supplied by the machine. See [third-party provenance](../../THIRD_PARTY.md).

For an isolated test, generate a **new** identity with `kokoda-windows-host.exe --init PATH`. Copy only `certificate.der` and `pairing-token` into the client's pairing folder. Keep `private-key.der` on the host. Start the development server with the explicit flag above and pass its address/port and pairing directory to the legacy command-line bridge fixture. The Mac GUI accepts passwords only; it cannot connect to this legacy test endpoint. Never reuse or commit personal pairing data for tests.

## Validation

Rust tests run on all three CI platforms; Windows CI prepares the pinned FFmpeg SDK before compiling the native adapter. `tests/check-locales.py` checks translation coverage, format arguments and generated-resource parity. Native regressions run using `scripts/run-macos-test.sh SUITE` with temporary executables and isolated test windows. Network fixtures live in `tests/fixtures`; end-to-end drivers in `tests/e2e` accept explicit addresses, credentials and result paths.

Hardware acceleration availability must be reported separately from compilation and VM validation. First-release acceptance still requires Windows/Linux GUI clients and real GPU coverage. [Current validation record](validation.md)

## Windows host GUI

Double-click `KokodaHost.exe`. Set a connection password, select a sharing scope and start sharing. On Mac, enter the address and password. Both endpoints must be updated; normal operation no longer uses pairing files. See [password connections](password-authentication.md) for credential storage, authentication, migration and limitations.

The native runner lives in `apps/host-settings/windows`. It opens stopped, offers rule editing/preview and LAN endpoints, and manages its child host through a Windows job object. Closing or terminating the GUI stops that host; minimizing keeps it running. Stop before editing scope or password. Run the native regression `tests/platform/windows/HostGui.Tests.ps1` in a signed-in Windows desktop with isolated test/result directories.
