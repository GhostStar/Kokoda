# 产品代码结构

[English](../en/code-structure.md)

三端共享交互决策，采集、播放和系统集成按平台实现。首版维持一对一连接、连接口令与主机窗口过滤。[当前开发链路与剩余发布边界](development.md)

## 目录与职责

| 目录 | 职责 | 当前实现 |
|---|---|---|
| apps/desktop/src、locales | 共用页面及翻译 | 112 个双语文案键；共享视觉界面待实现 |
| apps/macos、windows、linux | 启动、菜单和原生 UI 集成 | Mac 开发运行器已接入；Windows／Linux 预留 |
| apps/host-settings | 本机管理界面 | 预留 |
| crates/app-model | 窗口事实、身份、输入、画质和格式 | 共享 Rust 模型；应用分组不能充当授权策略 |
| crates/client | 窗口生命周期、全局画质及 UI 动作 | 已测试的控制器、C 接口与网络桥接；调度／恢复继续迁移 |
| crates/host | 接入、会话及授权调度 | 接入槽位与迁入的 Windows 窗口组／会话运行时；正式授权待实现 |
| crates/protocol | Kokoda 消息、限制与校验 | 开发协议路由／输入校验和画质档位；完整类型化消息及版本协商待完善 |
| crates/transport | 加密、分帧、超时及背压 | 固定证书／令牌开发配对、有界分帧及分段 I/O 处理 |
| crates/media | 帧、采集／播放接口与格式交集 | 共享接口、大小限制和后端错误边界；适配层完整接入待完善 |
| platforms/windows | 窗口元数据、采集、编码、输入与剪贴板 | 原生 Cargo 包，不依赖传输层 |
| platforms/macos | 播放、窗口呈现、输入法及剪贴板图片 | 原生 Swift 源码，由 Mac 运行器编译 |
| platforms/linux | 原生播放及集成 | 预留 |
| crates/windows-host、macos-host | 主机可执行入口组装 | Windows 命令行已接入；macOS 预留 |
| tests/contracts、fixtures、platform、e2e | 共用样例、合成窗口、原生与联网回归 | 已接入测试；Rust 回归也随 crate 保存 |
| packaging、scripts | 分发与构建工具 | 开发构建／签名／SDK 脚本；发行安装器待实现 |
| docs/en、docs/zh-CN | 对齐的产品与工程文档 | 同步维护 |

未实现目录保留说明或 `.gitkeep`，有实现后才加入构建目标。Windows／Linux UI 工具包仍需通过输入法、多窗口和视频表面验证。

## 依赖与模块边界

`app-model` 不依赖客户端、服务端或平台。客户端及协议依赖共享模型；客户端组装协议、传输并向原生运行器导出 C 接口，不依赖原生编解码器。Windows 平台代码依赖模型与媒体层，不依赖传输或端点调度。host 的 Windows 模块组装原生适配、协议和传输，主机入口负责启动。两端核心不相互依赖。

Mac 将已确认的根窗口身份交给共享控制器，并执行其全局画质动作。待打开队列、本地窗口、请求调度和恢复衔接暂由原生运行器维护，继续逐项迁入共享模块；输入法、焦点及媒体资源保留在平台边界。C 接口限定 UI 线程调用，返回字符串须通过同一个 Rust 库释放。

正式授权在主机执行，以平台事实为依据，界面过滤不能替代授权。开发主机默认返回空的只读目录，由主机规则引擎过滤发现及采集；只有显式启动参数才能开放全部窗口及输入/剪贴板。当前窗口标识、应用分组和持有令牌配对属于迁移契约，尚未实现完整隐私边界。发布前必须完成受限交互控制、规则热更新、发现列表代际及请求关联。

RustDesk 分帧代码保留上游归属；FFmpeg 通过公开 API 提供厂商编码集成。参见[第三方来源](../../THIRD_PARTY.md)。项目不要求兼容 RustDesk 端点。

## 生命周期与线程

一个主机循环拥有 Admission，先销毁窗口组及输入／采集资源，再释放连接槽位，旧代际释放不能影响新连接。开发监听循环串行运行，明确的主机忙响应和设备批准接入流程尚待完成。Windows Session 0 启动服务会在监听前被拒绝。

采集／编码线程持有原生资源，输出队列有界，预测编码帧保持顺序。原生回调绑定连接代际，窗口拒绝旧 token／流代际，断线丢弃待发输入。目前恢复凭据可以恢复窗口而不重放旧输入与剪贴板操作；正式权限撤销还必须清理并重新验证这些资源。

## 验证

运行 `cargo test --workspace --locked` 和 `python3 tests/check-locales.py`。CI 在 Windows、macOS、Linux 测试，Windows 才编译运行其原生后端。Mac 原生测试及真实网络驱动详见[开发说明](development.md)。[验证记录](validation.md)

Windows 原生主机设置入口已实现在 `apps/host-settings/windows`，生成 `KokodaHost.exe`。它负责本地配置与子主机进程生命周期，授权仍由 `kokoda-host` 执行。
