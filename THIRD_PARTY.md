# Third-party source

crates/transport/src/bytes_codec.rs is copied from RustDesk hbb_common at revision fae47e024dd83cef6aaecde0aa7d11731df11649, src/bytes_codec.rs, with a source attribution header added. Copyright belongs to the upstream contributors. Distributed under AGPL-3.0, included in LICENSE.

Source https://github.com/rustdesk/hbb_common/blob/fae47e024dd83cef6aaecde0aa7d11731df11649/src/bytes_codec.rs

This module provides length-prefixed byte framing only. It does not implement RustDesk rendezvous, authentication, encryption, or client interoperability.

## FFmpeg shared libraries

The Windows package dynamically links FFmpeg libavcodec/libavutil and their
swresample dependency, using the FFmpeg-Builds LGPL shared distribution. The
original LICENSE.txt and pinned version/download/source metadata accompany the
DLLs in the package's `ffmpeg` directory. No NVIDIA, AMD or Intel driver DLL is
redistributed. The former direct NVENC API implementation and header were removed.

- FFmpeg: https://ffmpeg.org/ ; https://github.com/FFmpeg/FFmpeg
- Binary build project and build recipes: https://github.com/FFmpeg-Builds/FFmpeg-Builds
- Exact binary URL, SHA256 and source revision: scripts/prepare-ffmpeg.py
- LGPL license copy: ffmpeg/LICENSE.txt in the Windows package

The DLLs remain separate and replaceable with ABI-compatible builds. Kokoda's
bridge uses FFmpeg's public APIs; it does not copy RustDesk's hwcodec source.

## Password authentication

OPAQUE is provided by the official `opaque-ke = 5.0.0-pre.0` crate, with
`voprf = 0.6.0-pre.2`, pinned in Cargo.lock. Both are used under their MIT
license option; original notices are in `packaging/licenses` and included in
both distributions. Sources: https://github.com/facebook/opaque-ke and
https://github.com/facebook/voprf . These are prerelease dependencies; an
upstream historical audit is not an audit of these versions or this integration.
See docs/en/password-authentication.md for the suite and validation boundary.

密码认证使用上述官方 OPAQUE／VOPRF 预发布库，选择 MIT 许可，原始声明随包分发。
上游历史审计不代表这些版本或本项目集成已通过独立审计。详见
docs/zh-CN/password-authentication.md。

## Scheduling and adaptive playback design reference

RustDesk 1.5.3 and master revision `817ba32b25c4930cd0b8ca7a89aa59b9d4cc94e2`
were reviewed for asynchronous input, per-display decoding, frame-consumption
feedback and rate adaptation. Relevant upstream files are
[src/client/io_loop.rs](https://github.com/rustdesk/rustdesk/blob/1.5.3/src/client/io_loop.rs),
[src/server/video_service.rs](https://github.com/rustdesk/rustdesk/blob/1.5.3/src/server/video_service.rs)
and [src/server/video_qos.rs](https://github.com/rustdesk/rustdesk/blob/1.5.3/src/server/video_qos.rs).
Upstream is AGPL-3.0. Kokoda's bounded input pipeline, window-credit protocol and
`crates/host/src/flow.rs` are independently implemented for Kokoda; no source from
those modules is copied or linked, and no RustDesk interoperability is added.

调度设计参考 RustDesk 上述版本的异步输入、独立解码、消费反馈与自适应思路。
上游许可为 AGPL-3.0；Kokoda 按自身窗口授权及协议独立实现，未复制或链接上述模块。

## Windows / Linux desktop clients

The Qt Widgets frontend links Qt Core, Gui and Widgets dynamically. Windows
builds use the unmodified Qt 6.8.3 MSVC 2022 x64 SDK; Linux builds use distribution
Qt 6 (minimum 6.4). Qt is Copyright The Qt Company Ltd. and its contributors;
these libraries are used under LGPL-3.0. License texts, including GPL-3.0 and the
Qt exception, are in `packaging/licenses/qt` and included in both client packages.
Libraries remain separate and replaceable with ABI-compatible builds. Qt plugins
retain their embedded licensing metadata. Qt source and its bundled third-party
notices: https://download.qt.io/archive/qt/6.8/6.8.3/submodules/qtbase-everywhere-src-6.8.3.tar.xz
; https://github.com/qt/qtbase/tree/v6.8.3/LICENSES . Linux system packages supply
their matching source and notices through the distribution package manager.

The shared asynchronous decoder uses FFmpeg's public libavcodec, libavutil and
libswscale APIs; no RustDesk decoder source is copied. Windows uses the same
pinned LGPL FFmpeg SDK as the host, with the additional swscale DLL. Linux uses
system FFmpeg (6.1 on the Ubuntu 24.04 CI target; 8.0 in the local ARM64 VM). Decoder probes use D3D11VA on
Windows and VAAPI on Linux; no vendor driver is redistributed. `BUILD.txt` in
packages records the Qt version or Linux dependency requirements.

Windows/Linux 客户端动态链接 Qt Core、Gui、Widgets，按 LGPL-3.0 使用，
未修改上游 Qt 源码。许可证随包分发，动态库可替换；上面的官方源码包含
Qt 内部第三方声明。Linux 通过发行版提供匹配的源码与声明。解码器调用
FFmpeg 公共 API，未复制 RustDesk 解码源码；Windows 额外携带 swscale
动态库，Linux 使用系统库，不打包显卡驱动。


## LAN discovery

Kokoda uses `mdns-sd = 0.22.0` under its MIT option for mDNS/DNS-SD
advertisement and browsing, through `crates/discovery`. Source:
https://github.com/keepsimple1/mdns-sd ; version and transitive dependencies
are pinned in Cargo.lock. Original MIT notices for mdns-sd and its newly added
network/channel dependencies accompany the applications in `packaging/licenses`.
Discovery is an untrusted address hint; it does not replace Kokoda password authentication.
