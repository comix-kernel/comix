# 协议栈运行时

`os/src/net/stack.rs` 是 smoltcp runtime 的归属层。

## NetworkStack

`NetworkStack` 对外提供稳定状态对象：

- 创建 TCP/UDP socket。
- TCP connect/listen/state/close/endpoints。
- UDP dispatch 和 per-port attach。
- SocketFile read/write/readable/writable/drop/sendto/recvfrom。
- poll 和 bounded loopback drain。

调用者不应直接访问 smoltcp socket set。

## 内部状态

`net::stack` 持有：

- `socket_set`：smoltcp socket set。
- `net_iface`：当前 active interface runtime。
- `loopback_link`：当前单 runtime 下的 loopback frame queue。
- `udp_ports`：UDP per-port dispatcher。
- `pending_tcp_close`：等待 graceful close 回收的 TCP socket。

这些状态都是 `NetworkStack` 字段，不作为裸全局符号暴露给 syscall 层或 socket 层。

## Poll 顺序

`NetworkStack::poll()` 推进协议栈，并集中处理：

- smoltcp interface poll。
- bounded loopback extra poll。
- UDP datagram dispatch。
- TCP graceful close reaping。
- poll/select waiter wakeup。

锁顺序保持为：

```text
NetworkStack -> interface runtime -> SocketSet -> SocketFile local state
```
