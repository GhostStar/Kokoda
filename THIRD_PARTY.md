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
