# Platform adapters · 平台适配

`windows` is a Cargo package providing WGC capture, FFmpeg/Media Foundation encoding, window facts, input and clipboard. It depends on shared models and media types, without a transport dependency. The host owns group/session orchestration in `crates/host/src/windows`.

`windows` 已是 Cargo 包，提供 WGC 采集、FFmpeg／Media Foundation 编码、窗口事实、输入和剪贴板，依赖共享模型及媒体类型，不依赖传输层。窗口组／会话调度位于 `crates/host/src/windows`。

`macos/Sources` contains the native Swift viewer, IME, clipboard image conversion and VideoToolbox decoder. The native runner builds these sources with the client C ABI. Windows/Linux playback and macOS hosting remain reserved.

`macos/Sources` 提供 Swift 原生窗口、输入法、剪贴板图片转换及 VideoToolbox 解码器，由原生启动器与客户端 C 接口共同构建。Windows／Linux 播放及 macOS 主机能力仍预留。
