# 开发运行链路

[English](../en/development.md)

第一批实现将已验证的 Windows 服务端／macOS 客户端链路迁入 Kokoda 新工作区，目前为开发版。产品与共享权限文档仍是正式发版的目标要求。

## 已接入的模块

- `kokoda-platform-windows`：WGC 捕获、FFmpeg NVENC／AMF／QSV 与 Media Foundation 回退、物理像素坐标、输入以及文本／图片剪贴板适配。
- `kokoda-host`：应用窗口组、各应用会话、视频／控制调度、恢复和连接清理。迁移的运行时由 `windows-runtime` 功能启用，主机可执行程序负责组装。
- `kokoda-protocol`：开发协议命令、路由／输入校验与统一画质档位。`kokoda-transport`：验证 TLS、离线传递的固定证书／令牌与有界分帧。两端使用 Kokoda 自有 `dev-1` 协议。
- `kokoda-client`：已测试的共享窗口生命周期、供 Mac 调用的 C 接口、全局画质动作和网络桥接程序。原生客户端仍负责部分调度及恢复衔接，后续逐步迁入共享核心。
- `platforms/macos`：原生窗口呈现、VideoToolbox、输入法和剪贴板图片处理。`apps/macos`：启动器及开发设置界面。`apps/desktop/locales`：中英文文案源，通过 `python3 scripts/generate-locales.py` 生成原生资源。

## 尚待实现的边界

开发主机通过 `--serve-development 地址 身份目录` 默认提供空的只读目录。加 `--sharing-policy 文件` 启用[指定窗口查看](restricted-sharing.md)，或显式加 `--allow-all-windows` 对全部应用窗口启用完整开发控制。配对仍采用手动传递证书和随机令牌，正式产品授权体系尚未完成。

限时配对码、设备批准／撤销、受限窗口交互控制、策略热更新、绑定窗口代次的发现列表、明确的主机忙响应和服务端设置界面尚待实现。全部窗口模式仍沿用原型应用归属规则；受限查看按主机规则过滤窗口组并禁用输入/剪贴板。应用归属规则本身不能作为隐私隔离边界。Windows/Linux 图形开发客户端现已接入，macOS 服务端尚待实现，目前不宣称达到首版发布条件。

媒体接口还需继续完善：当前 Windows 工作线程使用原生持有帧类型，Mac 使用原生解码器。继续保持 Kokoda 自有端点协议，禁止将窗口采集回退扩大为桌面采集。

## 构建

复用 PATH 中的 Rust 工具链。`scripts/cargo.sh` 也支持 `KOKODA_TOOLCHAIN_BIN`、`CARGO_HOME` 和 `CARGO_TARGET_DIR`，不会全局安装工具链。

```sh
./scripts/cargo.sh test --workspace --locked
python3 tests/check-locales.py
./scripts/build-macos.sh
```

Mac 产物为 `dist/Kokoda.app`，使用本地开发签名，尚未经过发行公证。要求 macOS 13 及以上和 Xcode 命令行工具。

Windows 安装 Rust MSVC 目标与 Visual Studio C++ 构建工具后，准备固定版本的 LGPL FFmpeg SDK：

```powershell
python scripts/prepare-ffmpeg.py
powershell -ExecutionPolicy Bypass -File scripts/build-windows.ps1 -Release
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -BinaryDirectory target/x86_64-pc-windows-msvc/release
```

可用 `-ToolRoot` 复用已有隔离工具链，`-TargetDir` 指定输出，`-FfmpegSdk` 指向已准备的对应 SDK。脚本先运行工作区测试，再构建 x86_64 MSVC 程序，并将可替换的 FFmpeg DLL 放在主机程序旁。显卡驱动由本机提供。参见[第三方来源](../../THIRD_PARTY.md)。

隔离测试使用 `kokoda-windows-host.exe --init 路径` 创建**新身份目录**。仅将 `certificate.der` 和 `pairing-token` 复制到客户端配对目录，`private-key.der` 留在主机。用上面的显式参数启动开发服务，将地址、端口及配对目录传给旧版命令行 bridge 测试入口。Mac 图形界面仅支持密码连接，不能连接这个旧版测试端点。测试禁止复用或提交个人配对凭据。

## 验证

Rust 测试在三端 CI 运行，Windows CI 先准备固定版本 FFmpeg SDK 再编译原生适配层。`tests/check-locales.py` 检查文案覆盖、格式参数和生成资源一致性。原生回归通过 `scripts/run-macos-test.sh 测试名` 运行，使用临时测试程序及隔离窗口。网络测试窗口位于 `tests/fixtures`，`tests/e2e` 接收显式地址、测试凭据和结果文件路径。

硬件加速可用性必须与编译／虚拟机测试分开记录。正式首版仍需三端原生输入法、发行版兼容及实机显卡验证。[本轮验证记录](validation.md)

## Windows 主机 GUI

双击 `KokodaHost.exe`，设置连接密码、选择共享范围后开始共享。Mac 输入地址和密码即可；两端需一起更新，正常使用不再需要配对文件。凭据保存、认证方式、迁移及边界见[密码连接](password-authentication.md)。

原生入口位于 `apps/host-settings/windows`，打开时保持停止，提供规则编辑/预览及局域网地址，并通过 Windows Job Object 管理子主机。关闭或结束 GUI 会停止该主机，最小化继续运行。更改范围或密码前先停止。原生回归 `tests/platform/windows/HostGui.Tests.ps1` 在已登录 Windows 桌面用隔离测试目录运行。

## Windows / Linux 图形客户端

Windows/Linux 图形开发版现已实现，详见[桌面客户端](desktop-clients.md)，其中包含构建、打包和交互边界。Kokoda.exe／Linux Kokoda 应用用于连接 Windows 主机；KokodaHost.exe 管理共享。客户端自动启动 bridge。正式发版仍需三端原生输入法、发行版兼容和真实显卡验证，macOS 服务端尚未实现。
