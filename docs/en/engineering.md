# Architecture, layout and verification

[简体中文](../zh-CN/engineering.md)

[Development runtime and remaining boundaries](development.md)

A trusted connection carries independent application sessions. Each session owns its root window, authorized members and input target. Stable protocol identifiers and negotiated capabilities are shared. Each platform owns native capture or presentation, IME, focus and clipboard.

apps/desktop holds shared screens and workflows. apps/{macos,windows,linux} holds platform runners. apps/host-settings/{windows,macos} integrates local host management. crates/{client,app-model,transport} holds shared client, model and transport code. crates/host owns shared host lifecycle and authorization orchestration; crates/{windows-host,macos-host} composes it with native implementations in platforms. The first Windows/macOS runtime is implemented; see the development guide for the current boundaries.

tests/contracts defines shared observable behavior. tests/fixtures contains isolated test windows, and tests/e2e covers real connections. Exercise two apps, popup ownership, focus changes, forbidden targets, quality changes during disconnect and revoked resume tickets. Record host, client, display system and actual encoder. packaging and scripts contain distribution and build tooling.

## Shared interactions and platform media

Each frontend sends user actions to kokoda-client, which owns the connection epoch, visible catalog, open viewers and global quality. Execute open, focus, close and quality effects on the native UI thread. Clear pending viewers on host rejection and discard old surfaces on revoke or disconnect. Run cargo test --workspace --locked to verify the shared behavior.

| Platform | Planned host capture | Planned client playback |
|---|---|---|
| Windows | WGC bound to the authorized HWND, D3D11 textures and hardware encoding where available | FFmpeg D3D11VA probing with software fallback; Qt presentation |
| macOS | Phase two: ScreenCaptureKit filtered to the authorized window | VideoToolbox decode and native window presentation |
| Linux | Client only in phase one | FFmpeg with runtime VA-API probing and software fallback, toolkit presentation under Wayland/X11 |

kokoda-media defines capture and player boundaries and selects a format supported at both ends. Frames carry window generation, stream epoch and sequence. Change epoch on encoder reset, discard obsolete frames and request a keyframe when predictive references are lost. Bound queues and keep capture fallback within the authorized window. Windows capture/encoding and macOS playback have been migrated; the common media adapter contract and release hardware validation remain in progress.

Kokoda clients and hosts use their own protocol for one-to-one connections, window authorization, window-group lifecycle and media capability negotiation. Reference or reuse RustDesk transport, codec and platform implementations through Kokoda interfaces. Evaluate dependency scope, platform support and licensing, record the upstream version and source, and validate integration with Kokoda end-to-end tests. The current shared controller and media interfaces are independent implementations; hbb_common framing is retained and FFmpeg provides vendor codec integration; other components remain subject to evaluation. [Codec and message handling](https://github.com/rustdesk/rustdesk/blob/d40e1da715fa1694ff4712f394b81a6df6bcd612/libs/scrap/src/common/codec.rs) · [Platform backends](https://github.com/rustdesk/rustdesk/tree/d40e1da715fa1694ff4712f394b81a6df6bcd612/libs/scrap/src)

[Product code structure](code-structure.md)

[Per-window video, transparency and dragging](window-video.md).

[Window roles and remote operations](window-management.md).

[Application icon sources and generation](../../packaging/icons/README.md).

The Windows/Linux GUI adapter is implemented with Qt Widgets. Its shared frontend and platform differences are specified in [desktop clients](desktop-clients.md).
