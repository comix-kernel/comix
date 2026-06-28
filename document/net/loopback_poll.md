# Loopback 与 poll

Loopback 和 poll 是当前网络栈能稳定跑本机 TCP/UDP 测试的关键边界。loopback 负责 127/8 流量回灌, poll 负责推进 smoltcp 和唤醒等待者。

## 当前状态

- `lo` 是显式接口, 默认配置会确保它存在。
- 无真实 NIC 时, active runtime 使用 `LoopbackNetDevice` 和 `127.0.0.1/8`。
- 有真实 NIC 时, 当前单 runtime 下仍通过 `NetTxToken` 检测 127/8 frame 并放入 `loopback_link`。
- 网络 poll 可由 I/O 路径直接调用, 也可由 timer/中断路径请求工作队列执行。

## 目标

- loopback 不依赖 `NullNetDevice` 的副作用。
- poll/select 看到的是已 dispatch 的 TCP/UDP socket 状态。
- 硬中断只做轻量通知, 协议推进在可控上下文中完成。

## 非目标

- 当前不提供独立 loopback smoltcp runtime。
- bounded drain 不是通用网络调度策略。
- 不在本文描述 pollfd 的每个事件位分支。

## Loopback 数据流

1. socket write/sendto 调用 smoltcp 发送。
2. `NetTxToken` 检查以太网帧中的 IPv4/ARP 地址。
3. 如果源或目的地址属于 127/8, 帧进入 `NetworkStack::loopback_link`。
4. 下一次 `NetDeviceAdapter::receive()` 优先从 loopback queue 取帧。
5. `NetworkStack::poll()` 驱动 smoltcp 接收并更新 socket 状态。

## Poll 流程

`NetworkStack::poll()` 执行:

- smoltcp interface poll。
- loopback extra poll。
- UDP per-port dispatch 到 per-fd queue。
- TCP pending close reaping。
- poll/select waiter wakeup。

`poll_network_and_dispatch()` 是 syscall I/O 层使用的门面。`request_network_poll()` 用原子 pending 位把中断/timer 侧请求合并到 work queue, 避免重复排队。

## 与 syscall I/O 的关系

- `read/write` 遇到 socket `WouldBlock` 时, 会先 poll 网络再 yield。
- `ppoll/poll/select` 在等待循环中推进网络状态。
- socket 文件的 `readable/writable` 查询只看当前可观察状态, 不复制用户缓冲区。
- 状态变化统一调用 `wake_poll_waiters()`。

## 并发和生命周期约束

- loopback queue 受 `NetworkStack` 内部锁保护。
- work queue pending 位避免多个中断重复安排网络 poll。
- poll 唤醒不能假设所有 waiter 都还持有原 fd, fd table 需重新验证。
- bounded drain 有固定次数上限, 避免写路径无限自旋。

## 已知限制

- 多接口 runtime 到来前, loopback 和真实 NIC 仍共享 active smoltcp interface。
- 对 127/8 的检测基于当前帧解析逻辑, 不是完整路由表。
- netperf 中和 `EINTR` 相关的现象更多来自 signal/syscall restart, 见 `netperf.md`。

## 源码索引

- `os/src/net/config.rs`: loopback interface 保证和默认配置。
- `os/src/device/net/loopback.rs`: 显式 loopback device。
- `os/src/net/stack/adapter.rs`: `NetTxToken` loopback 判定和 frame 回灌。
- `os/src/net/stack/mod.rs`: `poll()`, `poll_until_empty()`, loopback queue。
- `os/src/net/socket.rs`: `poll_network_and_dispatch()`, `request_network_poll()`。
- `os/src/kernel/syscall/io.rs`: poll/select 等待和唤醒。
