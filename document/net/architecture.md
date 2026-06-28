# 网络架构

网络架构的核心是把控制面, 数据面和用户 ABI 分开。设备驱动提供帧收发, 接口层保存配置, `NetworkStack` 拥有协议栈运行时, syscall 层只做用户边界。

## 当前状态

```text
NetDevice
  |
  v
register_net_device()
  |
  v
NetworkInterface registry
  |
  v
NetworkStack
  |
  v
SocketFile / UnixSocketFile
  |
  v
network syscall
```

AF_INET 走 `NetworkStack` 和 smoltcp。AF_UNIX 走 `UnixSocketFile` 内部队列和绑定表, 是同一个 syscall family 下的本地 IPC transport。

## 目标

- 设备层和 syscall 层都不直接持有 smoltcp socket set。
- `SocketFile` 保存 fd 局部状态, 协议栈状态由 `NetworkStack` 持有。
- `NetworkInterface` 是控制面对象, 不承担 `Driver` 职责, 中断兼容由 `NetDriverHandle` 处理。
- loopback 流量在当前单 runtime 下通过内部 frame queue 回灌。

## 非目标

- 不实现完整路由表和多接口 runtime 调度。
- 不在网络核心中放架构特定逻辑。
- 不把测试兼容路径提升为长期抽象, 如 bounded loopback drain。

## 模块边界

- `os/src/device/net/`: `NetDevice` 数据面。
- `os/src/net/interface.rs`: 接口 registry, IP/gateway 配置, smoltcp interface 工厂。
- `os/src/net/stack/mod.rs`: socket set, active interface, UDP dispatcher, TCP close 回收。
- `os/src/net/socket.rs`: AF_INET socket 文件和公开门面。
- `os/src/net/unix_socket.rs`: AF_UNIX 本地 socket。
- `os/src/kernel/syscall/network/`: 用户 ABI 和 fd table。

## 关键流程

1. 设备驱动创建 `Arc<dyn NetDevice>`。
2. `register_net_device()` 创建 `NetworkInterface`, 加入 registry, 注册 `NetDriverHandle`。
3. `NetworkConfigManager::init_default_interface()` 选择 active interface, 设置 IP/gateway, 初始化 `NetworkStack`。
4. `socket()` 为 AF_INET 创建 smoltcp handle 和 `SocketFile`, 为 AF_UNIX 创建 `UnixSocketFile`。
5. connect/send/recv/poll 等 syscall 通过文件对象或 `NetworkStack` 门面推进协议状态。

## 并发和生命周期约束

- `NetworkStack` 内部状态用 `SpinLock` 分区保护。
- `SocketFile::drop()` 必须释放或关闭对应 stack handle, 防止 fd 生命周期结束后 socket set 泄漏。
- UDP per-port socket 可能被多个 fd 共享, per-fd 接收队列在 `SocketFile` 中隔离。
- 网络中断不直接推进完整协议栈, 而是唤醒 poll waiters 或请求工作队列 poll。

## 已知限制

- 单 active smoltcp runtime 限制了多 NIC 路由能力。
- `SocketHandle` 仍包装 smoltcp handle, 是迁移中的兼容类型。
- IPv6 支持受构建 feature 和 smoltcp 路径限制。

## 源码索引

- `os/src/net/mod.rs`: `register_net_device()`。
- `os/src/net/interface.rs`: `NetworkInterface`, `NetworkInterfaceManager`, `NetDriverHandle`。
- `os/src/net/config.rs`: 默认接口配置。
- `os/src/net/stack/mod.rs`: `NetworkStack`。
- `os/src/net/socket.rs`: `SocketFile`。
- `os/src/net/unix_socket.rs`: AF_UNIX transport。
