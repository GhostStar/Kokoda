# 指定窗口查看（开发功能）

[English](../en/restricted-sharing.md)

主机现可通过本地 JSON 规则过滤客户端目录和采集窗口。第一阶段权限实现**仅供查看**：主机拒绝输入、文本/图片剪贴板及剪贴板轮询；Mac 客户端在窗口标题显示只读，也不会为远端操作读取本地剪贴板。完整开发控制仍可通过显式 `--allow-all-windows` 使用。

这不代表产品授权体系完成。设备批准/撤销、安全的输入/剪贴板接收方校验仍待实现。

发现和选择响应通过 `read_only` 告知客户端访问能力；拒绝交互请求时返回 `sharing_read_only`。各客户端应据此显示操作状态，最终权限仍由主机执行。

## 在 Windows 配置

创建新规则文件，不覆盖已有文件：

```powershell
kokoda-windows-host.exe --init-policy sharing.json
```

初始内容为 `{"mode":"selected","rules":[]}`，不共享任何窗口。示例：

```json
{
  "mode": "selected",
  "rules": [
    {"executable": "C:\\Apps\\Example.exe", "title_contains": "工作"},
    {"title_contains": "Kokoda Public Root"}
  ]
}
```

程序条件要求已存在的绝对路径，由主机解析后比较。标题子串使用 Unicode NFC 规范化及小写比较，保留空格。同一规则内的条件同时满足，不同规则任一命中即可。空条件/规则、未知 JSON 字段和无效路径使启动失败。文件最大 64 KiB，最多 128 条规则，接受有无 BOM 的 UTF-8。

先在主机本地预览，再从已登录用户桌面启动：

```powershell
kokoda-windows-host.exe --preview-policy sharing.json
kokoda-windows-host.exe --serve-development 0.0.0.0:47831 identity --sharing-policy sharing.json
```

身份及配对步骤见[开发指南](development.md)。不提供共享参数时，`--serve-development ADDRESS IDENTITY` 返回空的只读目录。需要原有全部应用窗口的输入和剪贴板功能时，显式指定 `--allow-all-windows`；JSON 加载器拒绝 `mode: all`，避免配置文件无提示启用完整访问。

## 执行方式和边界

共享规则引擎只接收主机观测的事实。弹窗只能沿核验后的 owner 链、在同一进程实例内继承权限（PID 加进程创建时间）。独立窗口即使同 PID 也须自行匹配。无法读取进程信息时拒绝放行；可执行路径不传给客户端，未授权 owner/前台窗口 ID 从输出元数据中移除。

选择、窗口组、采集线程及画面发送前都会重新核验权限。根窗口标题变化、弹窗解除归属可撤销访问，失去权限的待发送数据被丢弃。观察到拒绝后会作废该会话的恢复凭据，再次共享须重新选择。已传出的数据无法召回。标题条件不能隔离刻意伪造匹配标题的应用，也无法区分允许窗口内部绘制的私密内容。

规则在主机进程生命周期内不可变，**编辑后必须重启主机**；重启断开连接并作废所有恢复凭据，尚未实现规则热更新。受限模式下根窗口隐藏或消失会拒绝访问。采集线程在工作及有界队列等待期间检查权限；目录和画面检查不能与其他应用的 UI 变化构成原子事务。

## 验证

`kokoda-host` 测试默认拒绝、无效规则、组合条件、规范化标题、owner 链、独立同进程窗口、进程复用及撤销。`SharingTests` 检查原生客户端提示及输入/输入法/剪贴板抑制。Windows 测试窗口配合 `tests/e2e/verify-sharing.py` 验证真实 TLS 路径；`--empty` 验证默认目录，`--push` 验证推送。仅对隔离的 `sharing-windows.ps1` 测试窗口运行，显式指定状态及命令文件路径。

图形化配置可使用 [Windows 主机 GUI](development.md#windows-主机-gui)。
