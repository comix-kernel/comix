# 协议栈运行时

`NetworkStack` 是 AF_INET 协议栈运行时的唯一宿主。它持有 smoltcp socket set, active interface runtime, loopback link, UDP dispatcher 和 TCP 延迟关闭队列。

## 当前状态

- `NetworkStack::init_network()` 安装当前 active smoltcp interface。
- TCP/UDP socket 创建都通过 `NetworkStack` 向 socket set 添加 smoltcp socket。
- `SocketFile` 的 read/write/readable/writable/drop 都回调 `NetworkStack`。
- UDP bind 使用 per-port smoltcp socket 加 per-fd queue 的分发模型。
- loopback frame 通过 `NetTxToken` 回灌到 `loopback_link`。

## 目标

- 不把 smoltcp `SocketSet` 暴露给 syscall 层。
- 统一协议推进入口, 让 poll/select 和 I/O 路径看到一致状态。
- 把 TCP listener pool, UDP per-port dispatch 和 close reaping 集中管理。

## 非目标

- 不支持多个 active `Interface` 同时运行。
- 不在 `SocketFile` 中保存 smoltcp socket 本体。
- 不在中断上下文直接执行完整 smoltcp poll。

## 内部状态

- `socket_set`: smoltcp sockets。
- `net_iface`: active `NetIfaceWrapper`, 拥有 `NetDeviceAdapter` 和 smoltcp `Interface`。
- `loopback_link`: 127/8 发送帧的内部回灌队列。
- `udp_ports`: UDP local port 到共享 smoltcp UDP socket 和 fd weak list 的映射。
- `pending_tcp_close`: 等待 graceful close 后回收的 TCP handle。

## Poll 流程

`NetworkStack::poll()` 是主推进路径:

1. 对 active interface 执行 smoltcp poll。
2. 如果 loopback queue 产生新帧, 做有限额外 poll。
3. 从共享 UDP socket drain datagram, 投递到匹配 fd 的 per-fd queue。
4. 回收 pending TCP close。
5. 如状态变化, 唤醒 poll/select waiters。

写入 loopback 目标后, `socket_write()` 和 `socket_sendto()` 会执行 bounded drain, 让本机测试不必等外部中断。

## UDP per-port dispatch

smoltcp UDP demux 按 port 工作, 但 Linux 兼容场景允许多个 fd 绑定同一端口。当前实现为:

- 每个 local port 使用一个共享 smoltcp UDP socket。
- 每个 `SocketFile` 保存自己的 local/remote endpoint 和 rx queue。
- poll 时从共享 socket drain datagram, 按 endpoint 投递到对应 fd 队列。

## TCP listener 和 close

`SocketFile` 保存 listener 状态, backlog 和 listen queue。`NetworkStack` 负责从队列中取出 established child socket, 维护备用 listener, 并在 `SocketFile::drop()` 时关闭或移除 TCP handle。

## 并发和生命周期约束

- 调用者应通过 `NetworkStack` 方法访问 runtime 状态。
- drop 路径必须处理 listener queue 中的子 handle。
- UDP 共享 socket 的生命周期不能因某个 fd drop 而提前释放。
- poll 唤醒和 socket 可读/可写查询必须避免持锁后复制用户缓冲区。

## 已知限制

- 单 runtime 架构限制多接口路由。
- bounded loopback drain 是兼容路径, 不应扩展成通用调度机制。
- TCP buffer 大小和 listener pool 限制以源码常量为准。

## 源码索引

- `os/src/net/stack/mod.rs`: `NetworkStack`, poll, TCP/UDP runtime。
- `os/src/net/stack/adapter.rs`: smoltcp `Device` adapter 和 loopback frame 回灌。
- `os/src/net/socket.rs`: `SocketFile` 对 `NetworkStack` 的门面调用。
- `os/src/kernel/syscall/io.rs`: poll/select waiter 和网络 poll 协作。
