# 协议栈运行时

`os/src/net/stack.rs` 是 smoltcp runtime 的归属层。

## NetworkStack

`NetworkStack` 对外提供稳定门面：

- 创建 TCP/UDP socket。
- TCP connect/listen/state/close/endpoints。
- UDP dispatch 和 per-port attach。
- SocketFile read/write/readable/writable/drop/sendto/recvfrom。
- poll 和 bounded loopback drain。

调用者不应直接访问 smoltcp socket set。

## 内部状态

`net::stack` 持有：

- `SOCKET_SET`：smoltcp socket set。
- `NET_IFACE`：当前 active interface runtime。
- loopback link：当前单 runtime 下的 loopback frame queue。

这些符号仅限 `pub(crate)` 兼容实现使用，不对 syscall 层开放。

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
