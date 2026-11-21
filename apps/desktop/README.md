# Shared desktop UI · 共享桌面界面

`src` implements the shared Qt 6 Widgets Windows/Linux client: connection UI,
application sessions, window behavior, input, clipboard and recovery scheduling.
It loads the Rust core C ABI in `crates/client` for lifecycle/quality and manages
`kokoda-bridge` as a background child. macOS retains its AppKit runner and uses
the same Rust ABI. Native codec/ownership integrations stay in `platforms`.

`src` 实现 Windows/Linux 共用的 Qt 6 Widgets 客户端，包括连接界面、应用
会话、窗口行为、输入、剪贴板及恢复调度。通过 crates/client 的 Rust C ABI
使用生命周期/画质能力，并自动管理后台 bridge。Mac 保留 AppKit 运行器和
同一 Rust ABI。原生解码和窗口所属关系适配留在 platforms。

`locales` contains canonical English/Chinese resources. Run
`python3 scripts/generate-locales.py` after editing; CI verifies native resource
parity. `locales` 是标准中英文资源，修改后执行生成脚本，CI 检查原生资源一致性。

[Build, interactions and limits](../../docs/en/desktop-clients.md) ·
[构建、交互及限制](../../docs/zh-CN/desktop-clients.md)
