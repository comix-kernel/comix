# 网络模块概览

网络模块把 `NetDevice`, 接口控制面, smoltcp runtime, VFS socket 文件和网络 syscall 连接起来。当前仍是单 active runtime 设计, 但边界已经集中到 `NetworkStack` 门面。

## 当前状态

- 真实网卡通过 `register_net_device()` 注册为 `NetworkInterface`。
- 默认配置确保 `lo` 存在, loopback-only 场景使用 `127.0.0.1/8` 且没有默认网关。
- AF_INET TCP/UDP socket 由 smoltcp 驱动, 状态集中在 `NetworkStack`。
- AF_UNIX socket 是内核本地 IPC transport, 不进入 smoltcp。
- `SocketFile` 和 `UnixSocketFile` 都实现 VFS `File`, 由 syscall 层通过 fd table 暴露给用户态。

## 目标

- 设备层只提供收发帧能力, 不理解 socket 或 syscall。
- 接口层保存控制面配置, 不直接管理协议栈 socket set。
- 协议栈运行时由 `NetworkStack` 统一持有和推进。
- syscall 层只处理用户 ABI, fd table, sockaddr 和 errno。

## 非目标

- 当前不是多 active interface runtime。
- 不在文档列出每个 socket syscall 的参数和错误分支。
- 不把 AF_UNIX 混入 smoltcp 数据路径。

## 文档导航

- [整体架构](architecture.md)
- [设备与接口](device_and_interface.md)
- [协议栈运行时](stack_runtime.md)
- [Socket 与 syscall](socket_syscall.md)
- [Loopback 与 poll](loopback_poll.md)
- [测试与排查](testing.md)
- [网络实现指南](network_implementation_guide.md)
- [netperf / netserver 测试说明](netperf.md)

## 源码索引

- `os/src/device/net/`: 网卡设备抽象和 VirtIO/loopback/null 设备。
- `os/src/net/mod.rs`: 网络模块入口和 `register_net_device()`。
- `os/src/net/interface.rs`: `NetworkInterface`, registry, 中断兼容桥。
- `os/src/net/config.rs`: 默认接口配置和 loopback 初始化。
- `os/src/net/stack/mod.rs`: `NetworkStack` 和 smoltcp runtime。
- `os/src/net/stack/adapter.rs`: `NetDeviceAdapter` 和 loopback frame 回灌。
- `os/src/net/socket.rs`: AF_INET `SocketFile`, fd/socket mapping, poll 门面。
- `os/src/net/unix_socket.rs`: AF_UNIX socket。
- `os/src/kernel/syscall/network/`: socket syscall ABI 层。
