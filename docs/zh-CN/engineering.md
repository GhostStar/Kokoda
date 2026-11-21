# 架构、目录与测试

[English](../en/engineering.md)

一个受信连接承载独立应用会话，每个会话管理根窗口、授权成员及输入目标。协议标识与能力协商跨平台共用，采集或呈现、输入法、焦点和剪贴板由平台适配。

apps/desktop 存放共用页面与流程；apps/{macos,windows,linux} 存放平台运行器；apps/host-settings/{windows,macos} 接入本机管理接口。crates/{client,app-model,transport} 分别负责共享客户端、模型和传输；crates/host 负责共享主机生命周期与授权调度，crates/{windows-host,macos-host} 将其与 platforms 中的原生实现组装。首批 Windows／macOS 开发链路已接入，当前实现边界详见开发说明。

tests/contracts 定义跨平台行为，tests/fixtures 存放隔离测试窗口，tests/e2e 覆盖真实连接。测试双应用、弹窗归属、焦点切换、禁止目标、断线时切画质及撤权后恢复。记录主机、客户端、显示系统与实际编码器。packaging 与 scripts 用于分发和构建。

## 共享交互与平台媒体

三端界面向 kokoda-client 提交用户意图，由它维护当前连接代际、可见窗口目录、打开的查看窗口和全局画质。平台在 UI 线程执行打开、聚焦、关闭和切换画质动作。服务端拒绝打开时清理本地待开窗口，撤权或断线时清除旧画面。可运行 cargo test --workspace --locked 验证当前共享逻辑。

| 平台 | 主机采集方案 | 客户端播放方案 |
|---|---|---|
| Windows | WGC 绑定已授权 HWND，优先 D3D11 纹理与硬件编码 | Media Foundation 解码到 D3D11，保留软件解码回退 |
| macOS | 第二阶段用 ScreenCaptureKit 过滤指定窗口 | VideoToolbox 解码并呈现到原生窗口 |
| Linux | 首版仅作为客户端 | FFmpeg 探测 VA-API 能力，失败回退软件解码，由桌面工具包在 Wayland/X11 呈现 |

kokoda-media 定义采集与播放接口，按双方实际能力选择格式。帧携带窗口代际、流代际和序号。后端重建时更新流代际，客户端丢弃旧帧；预测帧失去参考时请求关键帧恢复。队列有界，采集回退也必须局限于获准窗口。Windows 采集／编码和 Mac 播放已迁入；通用媒体适配契约与发版硬件验证仍在推进。

Kokoda 客户端与服务端使用自己的协议，围绕一对一连接、窗口授权、窗口组生命周期及媒体能力协商设计。参考或复用 RustDesk 成熟的传输、编解码和平台实现，组件通过 Kokoda 的接口接入。复用时评估依赖范围、平台支持和许可证，记录来源及版本，并以 Kokoda 两端的端到端测试验证。当前共享核心与媒体接口为独立实现，已保留 hbb_common 分帧代码，FFmpeg 负责厂商编码集成；其他组件按需要评估。[编解码与消息代码](https://github.com/rustdesk/rustdesk/blob/d40e1da715fa1694ff4712f394b81a6df6bcd612/libs/scrap/src/common/codec.rs) · [平台后端](https://github.com/rustdesk/rustdesk/tree/d40e1da715fa1694ff4712f394b81a6df6bcd612/libs/scrap/src)

[产品代码结构](code-structure.md)

[已接入的开发链路与剩余边界](development.md)

[每窗口视频、透明度与拖动实现](window-video.md)。

[窗口角色与远端操作](window-management.md)。

[应用图标来源与生成](../../packaging/icons/README.md)。
