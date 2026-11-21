# Shared desktop UI · 共享桌面界面

`locales` contains the canonical bilingual resources. Run `python3 scripts/generate-locales.py` after editing them; CI verifies parity. The shared Rust controller and C ABI live in `crates/client`. macOS uses that ABI for confirmed viewer state and global quality. Shared visual screens and the remaining cross-platform connection workflows are still being implemented.

`locales` 存放统一的中英文文案，修改后运行 `python3 scripts/generate-locales.py`，CI 校验生成结果。共享 Rust 控制器及 C 接口位于 `crates/client`，Mac 已用于确认后的窗口状态与全局画质。共享视觉界面及其余跨平台连接流程仍在实现中。

Platform runners live in `apps/{macos,windows,linux}`, with native media/input adapters in `platforms`. [Current implementation](../../docs/en/development.md) · [当前实现](../../docs/zh-CN/development.md)
