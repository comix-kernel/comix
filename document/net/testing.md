# 网络测试与排查

本页只记录当前网络边界的验证点, 不作为完整测试计划。

## 基础检查

```bash
cd os
cargo fmt
cargo check
```

## 功能矩阵

- 启动后接口枚举能看到 `lo`。
- 有真实 VirtIO NIC 时接口枚举能看到 `ethN`。
- loopback-only 场景使用 `127.0.0.1/8`, 且没有错误默认网关。
- AF_INET TCP: socket, bind, listen, accept, connect, send, recv。
- AF_INET UDP: bind, sendto, recvfrom, connected UDP send/recv。
- AF_UNIX: socketpair, stream read/write, datagram queue, path/abstract bind。
- poll/select: TCP listener 可读, TCP recv 可读, UDP recv 可读。
- fd 生命周期: close/exit 后 socket handle 不应被复用 fd 命中。

## 排查路径

- 接口缺失: 查 `register_net_device()` 是否被驱动调用, `NETWORK_INTERFACE_MANAGER` 是否已有接口。
- loopback 不通: 查 `NetTxToken` 127/8 判定和 `NetworkStack::loopback_link` 是否有帧。
- UDP poll 不醒: 查 `NetworkStack::udp_dispatch_drain_locked()` 是否在 poll 路径执行, per-fd queue 是否收到 datagram。
- TCP accept 卡住: 查 listener queue, spare listener pool 和 `NetworkStack::poll()` 是否推进。
- `EINTR` 暴露: 查 signal pending 和 `signal_interrupts_syscall()`, 不要先假设网络栈丢包。
- syscalls 返回 `ENOTSOCK`: 查 fd table 中对象类型, 以及 `(tid, fd) -> SocketHandle` mapping 是否清理过早或遗漏。

## 已知现象

- netperf/netserver 的部分 `Interrupted system call` 输出是已知 signal/syscall restart 限制, 见 `netperf.md`。
- loopback 写路径存在 bounded drain, 因此本机测试可能比外部设备路径更快观察到状态变化。

## 源码索引

- `os/src/net/config.rs`: 默认接口和 loopback-only 配置。
- `os/src/net/stack/mod.rs`: poll, UDP dispatch, TCP close。
- `os/src/net/stack/adapter.rs`: loopback frame 回灌。
- `os/src/kernel/syscall/io.rs`: poll/select 和 WouldBlock 重试。
- `os/src/kernel/syscall/network/`: socket syscall。
