# ✨ Feature / 新功能

## 🚀 描述 (Description)

本次拉取请求在 `new-main` 上落实网络模块重构，主要改进如下：

- 收口网络协议栈运行时：新增 `NetworkStack` 门面，将 smoltcp socket set、active interface runtime、loopback link 和 poll 推进统一放到 `os/src/net/stack.rs`。
- 清理 socket/syscall 边界：`SocketFile` 的读写、可读可写、recvfrom、sendto、drop 等路径改为经 `NetworkStack` 操作协议栈；网络 syscall 不再直接访问 `SOCKET_SET`、`NET_IFACE` 或 `smoltcp::socket::{tcp, udp}`。
- 拆分设备与接口职责：VirtIO net 初始化只注册 `NetDevice`，接口创建统一由网络子系统入口负责；`NetworkInterface` 不再直接实现 `Driver`，改由 `NetDriverHandle` 作为兼容桥。
- 显式建模 loopback：新增 `LoopbackNetDevice`，默认网络初始化确保 `lo` 存在；`NullNetDevice` 不再承担 loopback 语义。
- 更新网络文档：补齐 `document/net/` 面向读者的模块文档，并恢复/补充 `document/net/`、`document/arch/*/network_rearchitecture.md` 中的施工记录。
- 清理本地协作文件：`.gitignore` 忽略各处 `AGENTS.md` 和 `docs/superpowers/` 草稿目录；补充 unused 类告警忽略标签，避免重构期间被旧未使用告警阻塞。

本记录基于以下提交：

- `2d93e1d chore: 忽略本地代理与草稿文档`
- `16b161f docs: 更新网络重构文档`
- `11ca651 fix: 忽略未使用告警`
- `37cc37e fix: 收口网络协议栈运行时`
- `4ad26a9 fix: 拆分网络设备与接口`
- `e1281ae fix: 修正文档空行`

## 🔗 关联 Issue

暂无关联 Issue。
