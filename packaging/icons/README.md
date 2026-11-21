# Kokoda application icons · 应用图标

The selected artwork is [the supplied unframed teal mark](../../docs/branding/kokoda-logo-unframed.png). This source is copied unchanged from the user's reference. The earlier framed design and other branding drafts are not build inputs.

当前使用[用户提供的青绿色无外框图形](../../docs/branding/kokoda-logo-unframed.png)，源文件为参考图的原样副本。此前带大外框版本及其他品牌草案不参与构建。

Run `./scripts/generate-icons.sh` on macOS. `prepare-icon.swift` uses Core Graphics to locate visible alpha bounds (alpha > 8), retain a three-source-pixel edge margin, and fit the unchanged artwork proportionally into an 896 px square centered on a transparent 1024 px canvas. It adds no enclosing frame or white tile. This removes the source image's excess empty canvas while preserving colors, overlap and transparency. The OS can still apply its own icon appearance treatment.

在 macOS 运行 `./scripts/generate-icons.sh`。`prepare-icon.swift` 使用 Core Graphics 查找可见透明度边界（alpha > 8），保留 3 个源像素的边缘余量，将图形等比放入 896 像素范围，并居中到 1024 像素透明画布。不添加外框或白色底板，只消除原图多余空白，保留原始颜色、叠放关系及透明度。系统仍可能应用自身的图标外观处理。

Apple's `sips` and `iconutil` generate the size variants and ICNS; Python's standard library packages PNG payloads into ICO without modifying pixels. Commit the source, preprocessing script and generated outputs together. Windows builds use the committed ICO and SDK resource compiler; `RC` can override its path.

Apple 的 `sips` 和 `iconutil` 生成各尺寸与 ICNS，Python 标准库只将 PNG 封装成 ICO，不修改像素。源图、预处理脚本及生成结果一起提交。Windows 构建使用已提交的 ICO 与 SDK 资源编译器，可通过 `RC` 指定编译器路径。

| Asset / 资源 | Use / 用途 |
|---|---|
| `kokoda-master.png` | Normalized transparent 1024 px master / 归一化的 1024 像素透明母版 |
| `Kokoda.icns` | Mac application bundle, Finder and Dock; 16–512 pt at 1×/2× / Mac 应用、Finder 与 Dock，16–512 点的 1×／2×资源 |
| `Kokoda.ico` | Windows GUI executable resource 1, native window and taskbar; 16–256 px / Windows GUI EXE 资源 1、原生窗口与任务栏，16–256 像素 |
| `kokoda.png` | 512 px transparent PNG for future Linux desktop packaging / 为后续 Linux 桌面打包准备的 512 像素透明 PNG |

Remote application viewers retain the remote application's title and identity; this logo represents Kokoda's own application. Linux desktop installation is not implemented by adding the PNG. After replacing a running Mac bundle, quit and reopen Kokoda to load the new resources; the build does not terminate an existing session.

远端应用查看窗口继续保留远端标题和身份，该图标代表 Kokoda 自身。提供 PNG 不代表已实现 Linux 桌面安装。替换正在运行的 Mac 应用后需退出并重新打开，才能加载新资源；构建不会终止现有会话。
