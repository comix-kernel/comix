# 网络子系统实现指南

本文档描述 `new-main` 上网络模块的当前实现边界和维护方式。网络模块以 `NetDevice -> NetworkInterface -> NetworkStack -> SocketFile/syscall` 为主线，保留少量兼容壳体以维持现有接口稳定。

## 当前分层

- 设备层：`os/src/device/net/` 定义和实现网卡数据面，核心抽象是 `NetDevice`。
- 接口层：`os/src/net/interface.rs` 维护接口名称、MAC、IP、网关和接口枚举。
- 栈运行时：`os/src/net/stack.rs` 持有 smoltcp runtime、socket set、loopback link 和 poll 推进入口。
- Socket 层：`os/src/net/socket.rs` 保留 `SocketFile`、fd 映射和 UDP per-fd 队列，协议栈操作通过 `NetworkStack` 门面执行。
- Syscall 层：`os/src/kernel/syscall/network.rs` 只处理用户 ABI、fd table、sockaddr 编解码和 errno 映射。

## 关键规则

- 设备驱动只注册 `NetDevice`，不直接创建接口、配置 IP 或操作 socket。
- `SOCKET_SET`、`NET_IFACE`、smoltcp socket 类型不得暴露给 syscall 层。
- `NetworkStack::poll()` 是协议栈推进和 poll/select 唤醒的统一入口。
- `lo` 是显式 loopback 语义；不要用 `NullNetDevice` 伪装 loopback。
- 架构差异只放在 `document/arch/*` 和 `os/src/arch/*`，不要进入网络核心实现。

## 维护入口

- 总体架构见 `document/net/architecture.md`。
- 设备和接口见 `document/net/device_and_interface.md`。
- 栈运行时见 `document/net/stack_runtime.md`。
- Socket 和 syscall 见 `document/net/socket_syscall.md`。
- Loopback 与 poll 见 `document/net/loopback_poll.md`。
- 测试和排查见 `document/net/testing.md`、`document/net/netperf.md`。

## 回归要求

每次修改网络边界后至少执行：

```bash
cd os
cargo fmt
cargo check
```

涉及行为修改时，还需要覆盖：

- 接口枚举能看到 `lo`，有真实网卡时还能看到 `ethN`。
- `set_network_interface_config()` 能正确设置和查询 IP、mask、gateway。
- `socket/bind/listen/accept/connect/send/recv/sendto/recvfrom` 基础路径可用。
- `ppoll/select` 能观察 TCP accept、TCP recv、UDP recv 可读事件。
- netperf/netserver 已知 EINTR 行为不要误判为网络功能不可用。
