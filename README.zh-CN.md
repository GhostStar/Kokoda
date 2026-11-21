# Kokoda

[English](README.md)

把另一台电脑上的应用窗口放在本机桌面，与本地应用并排使用。Kokoda 面向可信局域网内一台客户端连接一台主机的场景。

首版计划支持 macOS、Windows、Linux 客户端连接 Windows 主机。第二阶段增加 macOS 主机，三个客户端都可连接。本仓库包含产品设计、共享核心和首批开发运行链路。

- [产品功能与路线图](docs/zh-CN/product.md)
- [客户端与主机界面](docs/zh-CN/interface.md) · [交互原型](docs/zh-CN/prototype.html)（在本地浏览器打开）
- [主机共享权限](docs/zh-CN/sharing.md)
- [架构、目录与测试计划](docs/zh-CN/engineering.md)
- [贡献者与 AI 开发准则](AGENTS.md)

许可证为 [AGPL-3.0-only](LICENSE)。

[首版实现方案与开发切片](docs/zh-CN/implementation.md)

[产品代码结构](docs/zh-CN/code-structure.md)

首批 Windows 服务端／macOS 客户端开发链路已接入，参见[构建说明与实现边界](docs/zh-CN/development.md)。Windows/Linux 图形开发客户端也已接入，参见[桌面客户端构建与使用](docs/zh-CN/desktop-clients.md)。正式授权、原生输入法及硬件验收仍在进行。

[指定窗口查看配置](docs/zh-CN/restricted-sharing.md)

使用 Windows 主机地址与连接密码连接，无需传递配对文件夹。参见[密码设置与安全边界](docs/zh-CN/password-authentication.md)。
