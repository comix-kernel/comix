# netperf / netserver 测试说明

本页记录 netperf 场景和当前内核边界的关系。它不是网络实现指南, 也不要求修改用户态测试脚本。

## 当前状态

- 仓库内脚本 `data/netperf_testcode.sh` 会启动 `netserver`, 再运行 UDP/TCP stream 和 request-response 测试。
- 主要测试目标是 loopback TCP/UDP, socket syscall, poll/select 和 signal 交互。
- 测试可能在尾部看到 `Interrupted system call (errno 4)` 相关输出。

## 目标

- 用 netperf 覆盖本机网络数据面和 syscall 等待路径。
- 把已知 `EINTR` 输出和真实网络失败区分开。
- 保持脚本输出格式稳定, 便于回归比较。

## 非目标

- 不在文档中维护 netperf 参数大全。
- 不通过修改脚本掩盖内核 syscall restart 限制。
- 不把 netperf 结果当作完整网络兼容性证明。

## 关键流程

1. `netserver` 在 loopback 地址监听。
2. `netperf` 发起 TCP/UDP 流量。
3. socket syscall 进入 AF_INET `SocketFile`。
4. `NetworkStack::poll()` 推进 smoltcp, loopback queue 和 UDP dispatch。
5. poll/select waiter 被网络状态变化唤醒。
6. 子进程退出或信号到达时, syscall 可能返回 `EINTR`。

## 已知现象

`accept_connections: select failure: Interrupted system call (errno 4)` 可能出现。它表示 `netserver` 的 select 被信号打断并看到了 `EINTR`。

当前内核尚未完整实现 `SA_RESTART` 和 syscall restart, 因此该输出不一定表示网络栈失败。判断测试是否失败应同时看脚本是否完整跑完, 各测试段落是否输出 success, 以及是否存在真实连接/收发错误。

## 排查边界

- 如果只出现 `EINTR` 文案, 先查 signal 和 syscall restart, 不要直接改网络栈。
- 如果 TCP accept 或 recv 卡住, 查 `NetworkStack::poll()`, listener queue 和 `wake_poll_waiters()`。
- 如果 UDP 测试无数据, 查 UDP per-port dispatch 和 per-fd queue。
- 如果 loopback 发送后没有接收, 查 `NetTxToken` 127/8 判定和 `loopback_link` drain。

## 源码索引

- `os/src/net/stack/mod.rs`: loopback drain, TCP/UDP runtime, poll。
- `os/src/net/stack/adapter.rs`: loopback frame 回灌。
- `os/src/kernel/syscall/io.rs`: poll/select 和 `EINTR` 交互。
- `os/src/ipc/signal.rs`: `signal_interrupts_syscall()`。
- `os/src/kernel/syscall/network/`: socket syscall。
