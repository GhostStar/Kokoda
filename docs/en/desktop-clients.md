# Windows and Linux clients

[简体中文](../zh-CN/desktop-clients.md)

Windows and Linux now have a native Qt 6 Widgets development client. Both speak
Kokoda's password-authenticated protocol through `kokoda-bridge`. The bridge is
an automatically managed child process, not a separate user-facing program.
macOS retains AppKit and VideoToolbox. None of these endpoints implement RustDesk
interoperability.

## Common interaction contract

- Enter a host address and password, browse only the host-authorized catalog,
  and open multiple applications over one connection. A host still admits one
  client at a time. Passwords stay in memory for reconnects, never in settings
  or process arguments.
- Global quality changes apply to every open application without reopening.
  Match-viewer resolution uses backing pixels, including local display scaling;
  turning it off sends a zero viewport to restore the preset limit. Host policy
  still caps encoded dimensions and bitrate.
- Independent windows receive native frames/taskbar entries. Owned floating
  windows use native tool-window ownership; transient menus are undecorated.
  Discovery never broadens the host-authorized window group.
- Native close/minimize/maximize are local operations. Close hides the viewer
  and its owned floating descendants, preserving independent siblings. Window →
  Reopen Windows restores them. End Viewing releases an application session;
  disconnect releases all sessions. Neither closes remote applications.
- The selector above each decorated canvas chooses **Scale view** or **Resize
  host**. Scale view preserves the image's aspect ratio. Resize host explicitly
  changes remote width/height, with a debounced, generation-checked request.
  Read-only, blocked and fixed-size windows disable that option, not local close.
- Composition uses the local OS input method. Only committed text reaches the
  host. The candidate anchor follows the reported caret or last click. Focus
  loss releases pressed keys/buttons and cancels unfinished composition.
- Ctrl+C/X/V transfers text and PNG images through the host's contextual clipboard
  API. Files/URLs-as-files are rejected. Clipboard polling runs only for an active
  authorized viewer, establishes a baseline on focus changes, and does not replace
  a more recent local clipboard change with a late remote response.
- Reconnect clears pending input/clipboard, blanks disconnected surfaces and
  authenticates again before resuming. Local size/mode/dismissal are retained for
  matching members. Authentication failures are not retried. The connection menu
  offers manual reconnect and automatic reconnect control.

## Implementation boundaries

`apps/desktop/src` shares the Qt screens, session scheduling and interaction
handlers between Windows and Linux. It loads `kokoda-client` through the same C
ABI used on Mac for quality profiles and lifecycle bookkeeping. The frontend
still owns request scheduling/recovery integration; that code has not all moved
into the Rust core yet. Changes affecting observable behavior require Mac and Qt
contract coverage, not just a shared widget implementation.

`platforms/common/client` owns asynchronous FFmpeg decoding. Windows probes
D3D11VA; Linux probes VAAPI. Software decoding remains available, including the
`KOKODA_SOFTWARE_DECODE=1` diagnostic override. GPU output currently transfers to
CPU RGBA for Qt presentation; this is not a zero-copy GPU renderer. VM test
success is not certification of discrete-GPU playback performance.

Each viewer has ordered decoding, a maximum of three queued jobs and per-frame
consumption feedback. Two decoders may execute concurrently; encoded jobs share
a 96 MiB budget. The IPC reader has separate bounded delivery/backlog limits.
RGBA conversion uses FFmpeg-owned aligned/padded buffers before copying to Qt.
Hidden/minimized viewers hold at most two frame credits instead of growing queues.

`platforms/windows/client` applies native ownership/taskbar flags and constrains
interactive local resizing through `WM_SIZING`. `platforms/linux/client` supplies
transient ownership while Qt communicates with X11/Wayland. The compositor may
choose popup placement, activation and maximized dimensions. On Wayland,
maximized viewers retain proportional images but may have margins; submitting a
contradictory surface size is a protocol error, so Kokoda respects the compositor.

The Window menu and canvas-toolbar context menu offer 50–200% local zoom (within
OS minimum window dimensions, in the restored state). Each toolbar reports decoded
presentation FPS and frame dimensions. OS-native IME events and caret mapping are covered by tests;
real Windows Chinese IMEs and Linux IBus/Fcitx candidate UIs still need manual
validation. Audio, microphone, file transfer and macOS hosting remain out of scope.

## Build and package

Windows: MSVC 2022 x64 development shell, Rust MSVC target, Python, CMake, Ninja,
Qt 6.8.3 `msvc2022_64`, and the pinned FFmpeg SDK:

```powershell
python scripts/prepare-ffmpeg.py
./scripts/build-desktop-windows.ps1 -QtRoot C:/Qt/6.8.3/msvc2022_64
./scripts/package-desktop-windows.ps1 -BinaryDirectory target/desktop-windows -QtRoot C:/Qt/6.8.3/msvc2022_64 -FfmpegSdk .tools/ffmpeg/windows-x64
```

The ZIP contains a GUI `Kokoda.exe`, bridge, Rust core, replaceable Qt/FFmpeg DLLs,
platform plugins and bilingual instructions. Qt deployment handles its MSVC
runtime. Keep the extracted directory intact. This is distinct from the host
package, whose GUI is `KokodaHost.exe`.

Linux: Rust, Python 3, CMake, Ninja, a C++17 compiler, Qt 6.4+ development packages,
FFmpeg development packages (`libavcodec`, `libavutil`, `libswscale`), pkg-config,
and dpkg-dev for packaging. Install `qt6-wayland` for Wayland sessions.

```sh
scripts/build-desktop-linux.sh
scripts/package-desktop-linux.sh
```

Builds are native to the architecture/distribution. The `.deb` derives actual
library/version dependencies from the binaries; the tar uses those same system
libraries. A package built on Ubuntu 26.04 is not promised to run on 24.04. CI
builds Linux amd64 on Ubuntu 24.04 and Windows x64. Package filenames identify
architecture; `BUILD.txt` records dependencies.

## Verification

`kokoda-desktop-tests` covers shared core loading, translations, composition,
local/remote sizing, focus release, read-only handling, ownership, reconnection
preferences and repeated small non-aligned H.264 frames. CTest uses Qt offscreen;
run under Xvfb or a Wayland compositor to test real window-system behavior.

An opt-in `live` case requires `KOKODA_TEST_HOST` and `KOKODA_TEST_SECRET` (path to
a temporary JSON password file). Use only the synthetic roles fixture with its
animation command enabled. Optional `KOKODA_TEST_FIXTURE_DIR` enables observed
text-input/paste and cross-system clipboard checks. Tests fail on invalid setup;
they never operate arbitrary user application windows. No plaintext credentials
or remote titles/content are emitted by the test driver. See [validation](validation.md).
