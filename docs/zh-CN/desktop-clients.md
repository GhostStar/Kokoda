# Windows 和 Linux 客户端

[English](../en/desktop-clients.md)

Windows 和 Linux 现在提供原生 Qt 6 Widgets 开发版客户端，通过自动管理的
`kokoda-bridge` 子进程使用 Kokoda 口令认证协议。用户无需单独启动 bridge。
macOS 保留 AppKit 和 VideoToolbox；三端均不实现 RustDesk 协议互通。

## 共同操作规则

- 填写地址、口令，仅浏览主机授权的窗口，通过一条连接打开多个应用。主机
  同时只接纳一个客户端。口令仅留在内存供重连使用，不保存到设置或进程参数。
- 画质全局生效，切换不重开窗口。匹配窗口分辨率使用本地屏幕实际像素，包含
  显示缩放；关闭后发送零视口恢复档位上限。编码尺寸和码率仍受主机限制。
- 独立窗口有原生边框和独立任务栏入口；浮动窗口使用原生工具窗口及所属关系；
  临时菜单不加边框。自动发现不会扩大主机授权的窗口组。
- 客户端关闭、最小化、最大化只在本地处理。关闭隐藏此窗口和所属浮动后代，
  独立兄弟窗口保留；“窗口 → 重新打开窗口”可以恢复。“结束查看”释放一个
  应用会话，断开释放全部会话，均不关闭远端应用。
- 每个带边框画面上方的选择器提供“缩放画面”和“调整远端大小”。前者保持
  比例，后者明确改变远端宽高，请求经过防抖及窗口代次校验。只读、被模态阻挡
  或固定尺寸时禁用远端调整，但不禁用本地关闭。
- 使用本地系统输入法，只发送确认后的文字；候选位置跟随远端插入光标或最近
  点击。失焦释放按键、鼠标按钮并取消未完成的组词。
- Ctrl+C/X/V 通过主机的目标窗口剪贴板接口传输文字和 PNG 图片，不传文件。
  自动同步只针对当前活动的授权窗口，切换焦点时建立基线；迟到的远端结果
  不覆盖更新的本地剪贴板。
- 重连先清空待发输入/剪贴板、清除断线画面，重新认证后恢复应用。匹配窗口
  保留本地尺寸、模式和关闭状态。认证失败不自动重试；连接菜单提供手动重连
  及自动重连开关。

## 实现边界

`apps/desktop/src` 共享 Windows/Linux 的 Qt 界面、会话调度和交互逻辑，
通过与 Mac 相同的 C ABI 调用 `kokoda-client` 的画质档位和生命周期记录。
请求调度、恢复集成仍在前端，尚未全部迁入 Rust 核心。影响行为的修改需要
同时验证 Mac 和 Qt 合同，不能仅凭共用控件认定三端一致。

`platforms/common/client` 负责异步 FFmpeg 解码。Windows 探测 D3D11VA，
Linux 探测 VAAPI，也可用软件解码；`KOKODA_SOFTWARE_DECODE=1` 可强制软件
路径用于诊断。GPU 解码结果当前仍回读为 CPU RGBA 交给 Qt 显示，并非零拷贝
GPU 渲染。虚拟机通过不代表独显播放性能已经认证。

各窗口按顺序解码，最多排队三个任务并逐帧反馈消费进度。最多两个解码器并行，
编码数据共用 96 MiB 预算；IPC 接收另有排队和背压限制。RGBA 转换使用 FFmpeg
对齐且带填充的缓冲区，再复制到 Qt。隐藏/最小化保留最多两个帧额度，不无限积压。

`platforms/windows/client` 设置原生所属关系、任务栏属性，并通过 WM_SIZING
约束本地拖边比例。`platforms/linux/client` 提供窗口所属关系，由 Qt 对接
X11/Wayland。合成器可以决定弹窗位置、激活和最大化尺寸。Wayland 最大化时
画面保持比例但可能留边；提交矛盾的表面尺寸会触发协议错误，因此遵循合成器。

窗口菜单和画面工具栏右键菜单提供 50–200% 本地缩放（还原状态下，受系统最小
窗口尺寸限制），工具栏显示解码展示帧率及画面尺寸。测试覆盖输入法事件
及候选定位，但真实 Windows 中文输入法和 Linux IBus/Fcitx 候选界面仍需人工
实测。音频、麦克风、文件传输和 macOS 服务端不在本次范围内。

## 构建与打包

Windows 使用 MSVC 2022 x64 开发命令行、Rust MSVC 目标、Python、CMake、
Ninja、Qt 6.8.3 msvc2022_64 及固定版本 FFmpeg SDK：

```powershell
python scripts/prepare-ffmpeg.py
./scripts/build-desktop-windows.ps1 -QtRoot C:/Qt/6.8.3/msvc2022_64
./scripts/package-desktop-windows.ps1 -BinaryDirectory target/desktop-windows -QtRoot C:/Qt/6.8.3/msvc2022_64 -FfmpegSdk .tools/ffmpeg/windows-x64
```

ZIP 包含图形界面 Kokoda.exe、bridge、Rust 核心、可替换的 Qt/FFmpeg 动态库、
平台插件和中英文说明，Qt 部署工具处理 MSVC 运行库。完整解压使用，不要单独
移动 EXE。此客户端包与界面名为 KokodaHost.exe 的主机包相互独立。

Linux 需要 Rust、Python 3、CMake、Ninja、C++17 编译器、Qt 6.4+ 开发包、
FFmpeg 的 libavcodec/libavutil/libswscale 开发包、pkg-config，打包还需要
 dpkg-dev；Wayland 安装 qt6-wayland。

```sh
scripts/build-desktop-linux.sh
scripts/package-desktop-linux.sh
```

按本机架构及发行版构建。deb 从二进制推导实际库版本依赖，tar 使用相同系统库；
Ubuntu 26.04 构建不保证兼容 24.04。CI 构建 Ubuntu 24.04 amd64 和 Windows x64。
文件名标识架构，BUILD.txt 记录依赖。

## 验证

kokoda-desktop-tests 覆盖 Rust 核心加载、双语、组词、本地/远端尺寸、失焦释放、
只读、窗口所属关系、重连偏好，以及小尺寸非对齐 H.264 重复解码。CTest 默认
离屏运行；Xvfb 或 Wayland 合成器用于真实窗口系统测试。

实连 live 用例需设置 KOKODA_TEST_HOST、KOKODA_TEST_SECRET（临时 JSON 口令
文件路径），只连接已启用动画的合成 roles 测试窗口。可选的
KOKODA_TEST_FIXTURE_DIR 启用文字输入/粘贴结果及跨系统剪贴板检查。配置无效
会失败，不会操作用户任意应用。驱动不输出口令或远端标题/内容。
详见[验证记录](validation.md)。
