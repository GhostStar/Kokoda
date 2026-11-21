# Contributing · 开发准则

Read the product, engineering and sharing specifications before changing a boundary. Review each change from the perspective of all three clients. Keep permissions on the host and platform APIs in platform code.

修改跨组件行为前阅读产品、架构及共享权限文档。从三种客户端的角度评估改动。授权由主机执行，平台 API 留在平台代码中。

Update English and Simplified Chinese documentation and user-facing strings together. Use stable localization keys and preserve user-supplied titles and input.

同步更新中英文文档与界面资源，使用稳定的本地化键，保留用户窗口标题和输入原文。

Test with isolated windows and temporary credentials. Keep keys, pairing tokens, clipboard contents and private window titles out of commits and logs. Record what was tested on hardware and stop only test processes created for the run.

使用隔离测试窗口和临时凭据。提交与日志排除密钥、配对令牌、剪贴板内容及私人窗口标题。记录硬件实测范围，仅清理本次启动的测试进程。

Kokoda owns the client-to-host protocol and its window authorization semantics. Evaluate RustDesk transport, codec and platform components for reference or reuse through Kokoda interfaces. Record the source, version and license of reused code, and verify the complete Kokoda workflow.

Kokoda 自行定义客户端与服务端协议及窗口授权语义。评估参考或复用 RustDesk 的传输、编解码和平台组件，通过 Kokoda 接口接入。复用代码记录来源、版本与许可证，并验证完整 Kokoda 使用流程。
