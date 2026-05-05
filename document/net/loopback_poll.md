# Loopback 与 poll

## Loopback 模型

`lo` 是显式接口，不再由 `NullNetDevice` 隐式承担。无真实 NIC 时，默认配置创建 `LoopbackNetDevice`，接口名为 `lo`，地址为 `127.0.0.1/8`。

当前仍是单 active smoltcp runtime，因此有真实 NIC 时，127/8 frame 通过 `net::stack` 内部 loopback link 消费；后续多 runtime 可以把 `lo` 升级为独立 smoltcp interface。

## Poll 模型

协议栈推进集中在 `NetworkStack::poll()`：

- smoltcp poll。
- loopback link bounded drain。
- UDP per-port dispatch 到 per-fd queue。
- TCP pending close reaping。
- poll/select waiter wakeup。

`poll_until_empty()` 仅作为 loopback/netperf 兼容包装保留，不应成为新的主路径。

## 中断协作

网络中断兼容桥只记录事件和唤醒 waiter，不直接维护另一份协议栈状态。需要处理收包时，由进程上下文中的 poll/read/write/connect 等路径推进 `NetworkStack`。
