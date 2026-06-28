# 网络实现指南

本文给维护网络代码时的边界规则。细节 API 以源码和 rustdoc 为准。

## 当前分层

- 设备层: `os/src/device/net/` 定义和实现 `NetDevice`。
- 接口层: `os/src/net/interface.rs` 维护接口 registry 和控制面配置。
- 配置层: `os/src/net/config.rs` 初始化默认 IP, gateway 和 loopback。
- 栈运行时: `os/src/net/stack/mod.rs` 持有 smoltcp runtime 和 poll 推进。
- Socket 层: `os/src/net/socket.rs` 和 `os/src/net/unix_socket.rs` 暴露 VFS `File`。
- Syscall 层: `os/src/kernel/syscall/network/` 处理用户 ABI。

## 维护规则

- 设备驱动只注册 `NetDevice`, 不配置 IP 或操作 socket。
- syscall 层不要 import smoltcp socket 类型, 也不要直接访问 socket set。
- 新的 AF_INET 行为优先加到 `NetworkStack` 门面, 再由 `SocketFile` 或 syscall 调用。
- AF_UNIX 行为留在 `unix_socket.rs`, 不进入 smoltcp runtime。
- 用户指针复制只能在 syscall 边界完成。
- 状态变化后调用 `wake_poll_waiters()`, 但不要在硬中断中推进完整协议栈。

## 修改 checklist

- 改设备注册: 检查 `register_net_device()` 和 `NetworkInterfaceManager`。
- 改默认网络: 检查 `NetworkConfigManager::init_default_interface()` 和 loopback-only 分支。
- 改 TCP/UDP: 检查 `NetworkStack`, `SocketFile`, network syscall ops 是否保持边界。
- 改 poll: 检查 `io.rs`, `NetworkStack::poll()`, UDP dispatch 和 wakeup。
- 改 fd 生命周期: 检查 `SocketFile::drop()`, exit cleanup 和 fd/socket map。

## 回归建议

- `cd os && cargo fmt`
- `cd os && cargo check`
- 启动后确认接口枚举能看到 `lo`。
- 有真实 VirtIO NIC 时确认可见 `ethN`。
- 覆盖 `socket/bind/listen/accept/connect/send/recv/sendto/recvfrom` 基础路径。
- 覆盖 `poll/ppoll/select` 对 TCP accept, TCP recv, UDP recv 的可读观察。
- 对 netperf/netserver 输出按 `netperf.md` 判断, 不把已知 `EINTR` 文案误判为网络失效。

## 已知限制

- 单 active runtime 是最大结构限制。
- UDP per-port dispatcher 是兼容当前 smoltcp demux 的设计, 后续多 socket demux 可能重构。
- loopback bounded drain 是测试兼容路径, 不能替代真实调度和中断模型。

## 源码索引

- `os/src/net/mod.rs`
- `os/src/net/interface.rs`
- `os/src/net/config.rs`
- `os/src/net/stack/mod.rs`
- `os/src/net/stack/adapter.rs`
- `os/src/net/socket.rs`
- `os/src/net/unix_socket.rs`
- `os/src/kernel/syscall/network/`
