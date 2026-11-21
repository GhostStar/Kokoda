Kokoda 开发版客户端 — Windows / Linux

Windows：完整解压 ZIP，双击 Kokoda.exe。保留旁边的 DLL、插件和
kokoda-bridge，后者由界面在后台启动，不需要命令行或配对文件夹。

Linux：安装匹配架构与发行版的 .deb，然后从应用菜单打开 Kokoda。
便携 tar 依赖系统 Qt 6 和 FFmpeg，安装 BUILD.txt 列出的依赖后运行 bin/Kokoda。

在 Windows 的 KokodaHost 中开始共享，客户端填写地址（通常为 47832
端口）和连接口令。口令只保留在内存中用于重连，不写入设置文件。
列表只显示主机允许共享的窗口。

双击应用打开远程窗口；同一连接可以再打开其他应用。浮动窗口跟随所属
应用，独立窗口分别展示。画质是全局设置，切换无需重新打开。
使用本地输入法，以及 Ctrl+C/X/V 复制、剪切、粘贴文字和图片。
目前不提供文件传输和音频。

客户端关闭、最小化、最大化不操作远端应用的窗口按钮。可在“窗口”菜单
重新打开本地关闭的窗口。每个窗口上方的“缩放画面”保持比例；切换为
“调整远端大小”才会改变主机窗口尺寸。Wayland 最大化尺寸由合成器决定，
此时保持比例的画面周围可能留空。

这是开发版，并非完成签名的正式发行版。已测环境和限制见源码仓库的
文档 docs/zh-CN/desktop-clients.md、validation.md。依赖许可见 LICENSE、
THIRD_PARTY.md 和 licenses。硬件解码依赖驱动，诊断时可设置环境变量
KOKODA_SOFTWARE_DECODE=1 强制使用软件解码。
