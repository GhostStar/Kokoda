# Verification · 验证

- `cargo test --workspace --locked`: shared model, native C ABI, routing, transport, admission and Windows native regressions. Windows requires the pinned FFmpeg headers.
- `python3 tests/check-locales.py`: bilingual catalog and generated resource parity.
- `scripts/run-macos-test.sh InputTests`, `GroupTests`, `SessionTests`, `MultiSessionTests`, `LocalizationTests`: isolated AppKit regressions, using sources in `tests/platform/macos`.
- `tests/e2e`: real TLS/stream drivers. Supply an explicitly created test identity and run `tests/fixtures` in the Windows interactive desktop. Clipboard suites also accept bridge/address/pairing/result paths. They use a named local test clipboard; remote fixture clipboard contents are temporary.

共享、协议及 Windows 原生测试通过 Cargo 运行；Windows 编译需要固定版本 FFmpeg 头文件。双语资源由脚本校验。AppKit 回归位于 `tests/platform/macos`，网络回归位于 `tests/e2e`，隔离窗口位于 `tests/fixtures`。剪贴板测试使用命名的本地测试剪贴板；远端测试内容为临时数据。

Stop only processes created by the test. Remove temporary firewall rules, credentials and one-off launch tasks. Never select or send input to a user's real conversation window. Record GPU testing separately from VM results.

仅停止本次测试创建的进程；删除临时防火墙规则、凭据和启动任务。禁止选择用户的真实聊天窗口发送输入；显卡实测与虚拟机结果分开记录。

Windows-only loopback regression: `tests/e2e/verify-windows-runtime.ps1 -Bridge PATH -Pairing PATH -Results PATH`. Start the six named fixtures in the interactive session first. Add `-StressRapidSwitch` to reproduce the known rapid-focus input-loss defect; see the [validation record](../docs/en/validation.md). Default mode waits for observed commits and does not certify that stress case.

Windows 回环回归使用上述脚本及六个命名的交互会话测试窗口。`-StressRapidSwitch` 复现已知快速焦点切换丢字问题，见[验证记录](../docs/zh-CN/validation.md)；默认模式等待实际文字到达，不能据此宣称压力场景通过。

Restricted viewing / 受限查看：`cargo test -p kokoda-host` covers policy semantics; `scripts/run-macos-test.sh SharingTests` checks native read-only interactions. Use `tests/fixtures/sharing-windows.ps1` with `tests/e2e/verify-sharing.py` for real catalog/capture denial and title/owner revocation; `--push` tests push streams, `--empty` tests default denial. See [configuration](../docs/en/restricted-sharing.md) / [配置](../docs/zh-CN/restricted-sharing.md).

Windows host GUI / Windows 主机界面：`tests/platform/windows/HostGui.Tests.ps1` runs in an interactive signed-in Windows desktop with explicit binary/result directories. It exercises real native controls, loopback TLS, selected/all scopes and child-process cleanup, with isolated credentials. 该脚本操作真实原生控件，验证回环 TLS、共享范围与退出清理，使用隔离凭据。

Input/video flow / 输入与视频调度：`scripts/run-macos-test.sh PipelineTests` verifies ordered pipelining, barriers and background IPC parsing; `AsyncFrameTests` covers independent decode and control delivery. Run `PopupLiveTests BRIDGE ADDRESS SECRET_JSON` against `tests/fixtures/popup-video-windows.ps1` for native tracking and reconnect. `python3 tests/e2e/verify-playback-flow.py --client BRIDGE --address ADDRESS --secret SECRET_JSON` checks per-window credit isolation and rate adaptation against the same password-authenticated fixture. Use temporary credentials and remove the fixture/host afterward.

`PipelineTests` 验证有序输入流水线、屏障及后台 IPC 解析，`AsyncFrameTests` 覆盖独立解码与控制响应。`PopupLiveTests` 使用上述动画窗口检查原生拖动和重连；`verify-playback-flow.py` 检查每窗口消费额度隔离和速率调整，注入的慢解码指标不作为硬件性能数据。全部使用临时凭据，完成后清理测试窗口及主机。

Window management / 窗口管理：`WindowManagementTests` verifies native roles, actual owners, modality, resize revisions and operation routing. `WindowManagementLiveTests BRIDGE ADDRESS SECRET_JSON` uses `tests/fixtures/window-roles-windows.ps1`; the fixture accepts `-StateFile` and `-CommandFile` paths beside the temporary secret (`roles-state.json`, `roles-command.txt`). Start a second `-Mode Unrelated` fixture as a sibling process to test exclusion; the root fixture starts its own descendant on command. `tests/e2e/verify-window-management.py --client BRIDGE --address ADDRESS --secret SECRET_JSON --command COMMAND_FILE --state STATE_FILE` verifies host geometry, native menus, lifecycle and process membership. Add `--read-only` against a title rule allowing only `Kokoda Roles Root` to verify mutation denial and unauthorized descendant exclusion.

`WindowManagementTests` 验证原生窗口角色、实际 owner、模态限制、尺寸修订及操作路由；真实测试使用上述角色夹具及临时密码。同级启动 `-Mode Unrelated` 夹具用于验证无关同程序实例不会加入，后代进程由根夹具按指令创建。`--read-only` 使用仅允许 `Kokoda Roles Root` 的标题规则，验证所有窗口修改被主机拒绝，以及进程跟随不会扩大共享范围。测试结束写入 `cleanup:stop`，关闭独立启动的无关夹具，并清理主机与临时凭据。
