# 密码连接

[English](../en/password-authentication.md)

正常使用只需主机地址和连接密码。Windows 打开 `KokodaHost.exe`，点击「设置连接密码」并输入两次，选择共享范围后开始共享；Mac 输入地址和密码连接。不再需要配对文件夹，两端必须一起更新。

密码包含 8–128 个 Unicode 标量字符，不含控制字符，保留空格；建议使用较长且不重复的密码。修改前先停止共享。保存新密码会替换本地注册记录，下次启动只接受新密码，不保留之前的恢复票据。密码允许访问主机配置的共享范围，指定窗口仍为只读。知道密码的人都可以认证，这不是逐设备批准或撤销。

## 认证与传输

`crates/transport/src/password.rs` 集成上游 `opaque-ke` 的 OPAQUE 实现，没有自行实现 PAKE 密码学原语。固定套件使用 Ristretto255、TripleDH/SHA-512 和 Argon2id（锁定库的默认参数：19 MiB、两轮、并行度一）。Windows 所有者设置密码时在本机完成注册，没有向未认证网络开放的注册或重置密码接口。

每次接受连接都生成新的自签名证书，建立禁用早期数据和会话票据的 TLS 1.3 连接。认证前客户端校验 TLS 握手签名，但不把证书本身当作可信主机身份。OPAQUE 的认证上下文包含 `kokoda-password-1`，以及标签为 `EXPORTER-Kokoda-OPAQUE-v1` 的 32 字节 TLS exporter，从而阻止把认证过程转接到另一条 TLS 连接。客户端校验服务器的 OPAQUE 响应，主机校验最终客户端证明后发送成功标记，此后才开放正常应用协议。该端点不会回退到令牌认证。

应用数据由 TLS 流量密钥加密，OPAQUE 认证这条具体的 TLS 连接。认证帧在分配内容缓冲区前检查 4 KiB 长度上限，套接字读写超时为 10 秒。主机在滚动一分钟内限制每个来源 IP 五次、全局二十次连接尝试，成功连接也计入；超额连接在 TLS 处理前丢弃。这限制猜测和内存占用，但不代表能抵抗网络拒绝服务。当前主机一次处理一个会话。

## 保存与进程边界

Windows 将 OPAQUE 服务端设置和注册记录保存在 `%LOCALAPPDATA%\Kokoda\password.dat`，使用当前 Windows 用户的 DPAPI 保护，不会把明文密码写入 `settings.json`。记录损坏或无法读取时不会擅自替换，所有者可以在停止共享时重新设置密码。DPAPI 无法防御已经以同一用户身份运行的恶意代码。新连接不需要证书文件，证书私钥仅保留在进程内存。

Mac 使用 `NSSecureTextField`，在程序运行期间将密码保留在内存以供重连，尚未提供持久化「记住密码」功能。通过继承的 stdin 管道，以长度受限的 JSON 前导消息交给 `kokoda-bridge`，不使用命令行参数或环境变量。Rust 临时秘密缓冲区使用 `zeroize`；不据此保证所有系统或 Swift 界面副本均可擦除。认证失败不会自动重试。

显式旧入口 `--serve-development ... IDENTITY` 和 bridge 的配对目录参数仅保留给既有开发夹具。GUI 只启动 `--serve-password ... password.dat`，不会选择旧路径。旧身份/配对目录不会被新 GUI 使用，也不会自动删除。密码记录带版本；未来不兼容的套件变更必须升级记录及认证版本，并提供明确的重置/迁移路径。

## 依赖状态与验证边界

锁定官方 `opaque-ke 5.0.0-pre.0`（预发布版）、`voprf 0.6.0-pre.2` 和 `crypto-bigint 0.7.11`，避开旧版 crypto-bigint 的 [RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387) 依赖问题。上游历史审计不代表该版本或 Kokoda 集成已独立审计。本软件仍为开发版，正式发布前需要依赖/安全审查和更广泛设备覆盖。

来源：[OPAQUE 实现](https://github.com/facebook/opaque-ke)、[RFC 9807](https://www.rfc-editor.org/rfc/rfc9807.html)、[TLS 1.3 exporter](https://www.rfc-editor.org/rfc/rfc8446.html#section-7.5)。实际执行的测试见[验证记录](validation.md)。
