# Loopback 与 poll 路径重构说明

本文档覆盖 loopback、RX/TX、poll/select 唤醒与中断协作。这里是当前网络实现最容易“补丁堆补丁”的区域，必须先固定模型。

## 当前代码

- `NetDeviceAdapter` 内部有 `loopback_queue`。
- `NetTxToken::consume()` 根据 `device.name() == "null-net"` 或 127 地址判断是否回灌队列。
- `NetIfaceWrapper::poll()` 为 loopback 做额外 bounded poll。
- `poll_until_empty()` 为本地回环和 netperf 场景主动 drain。
- `NetworkInterface::try_handle_interrupt()` 直接从设备收包并唤醒 poll waiters。

## 目标模型

- `lo` 是显式接口，不是 `NullNetDevice` 或 `NetDeviceAdapter` 的隐藏行为。
- 真实 NIC 的 RX/TX 只来自设备队列。
- loopback 的 RX/TX 走独立 loopback 后端或 `NetworkStack` 内部虚拟链路。
- 中断路径只标记事件并调度网络 poll，不直接维护另一份协议栈推进逻辑。
- `NetworkStack::poll()` 是唯一入口。

## Loopback 建模选择

优先方案：

- 新增 `LoopbackDevice`，实现与 `NetDevice` 相近的二层帧收发接口。
- 网络子系统启动时创建 `lo`，配置 `127.0.0.1/8`。
- loopback 发送帧进入自己的 RX 队列，再由 `NetworkStack::poll()` 消费。

备选方案：

- 不创建完整设备对象，只在 `NetworkStack` 内部维护 `LoopbackLink`。
- `lo` 仍必须作为 `NetIf` 出现在接口注册表中。

无论使用哪个方案，都不能继续把 loopback 写在普通 `NetDeviceAdapter` 的特判里。

## poll 改造

`NetworkStack::poll()` 应按固定顺序执行：

1. 采样当前时间。
2. 从真实设备和 loopback 后端接收帧。
3. 调用 smoltcp poll。
4. 处理 TCP close reap。
5. 处理 UDP dispatch。
6. 计算 socket 可读/可写变化。
7. 统一唤醒 poll/select waiters。

## 中断协作

- 设备中断处理不直接调用 smoltcp poll。
- 中断处理只记录对应设备或接口有 RX/TX 事件。
- 如果当前内核没有独立网络线程，可在中断返回前轻量唤醒 waiters，由进程上下文中的 poll 路径推进。
- 不得在中断上下文执行会分配内存的 UDP dispatch。

## 迁移步骤

1. 给当前额外 poll 逻辑加清晰注释，标记为兼容层。
2. 新增显式 `lo` 接口并让接口枚举能看到它。
3. 将 `NullNetDevice` 与 loopback 语义拆开。
4. 将 `loopback_queue` 从 `NetDeviceAdapter` 移到 loopback 后端。
5. 删除 `device.name() == "null-net"` 特判。
6. 将 `poll_until_empty()` 降级为测试/兼容包装，主路径只调用 `NetworkStack::poll()`。

## 验收点

- 没有真实网卡时，`lo` 仍能服务 TCP/UDP 127.0.0.1。
- 有真实网卡时，127.0.0.1 不经真实 NIC。
- UDP poll/select 可读事件不依赖 ad-hoc 额外轮询补丁。
- 中断路径和主动 poll 不形成双重状态源。

## 当前执行状态

- 新增 `LoopbackNetDevice`，无真实 NIC 时由默认配置创建显式 `lo`。
- 默认网络初始化会先确保 `lo` 存在，再选择真实 `ethN` 或 `lo` 作为当前单 active smoltcp runtime。
- `NetTxToken` 不再检查 `device.name() == "null-net"`；`NullNetDevice` 不再表示 loopback。
- 兼容 loopback link 已归 `net::stack` runtime，普通 `NetDeviceAdapter` 不再持有自己的 loopback 队列字段。

保留的兼容点：

- 当前仍是单 active interface runtime；有真实 NIC 时 `lo` 主要作为接口枚举和 127/8 runtime link 语义存在，还不是独立 smoltcp interface。
- `poll_until_empty()` 仍作为 loopback/netperf 兼容包装保留，主路径继续通过 `NetworkStack::poll()` 推进。
