# Socket 与 syscall

Socket 层把网络 transport 暴露为 VFS `File`。AF_INET 使用 `SocketFile` 和 `NetworkStack`, AF_UNIX 使用 `UnixSocketFile` 的内核本地队列。

## 当前状态

- `socket(AF_INET, SOCK_STREAM/SOCK_DGRAM)` 创建 smoltcp TCP/UDP handle 和 `SocketFile`。
- `socket(AF_UNIX, ...)` 创建 `UnixSocketFile`。
- `socketpair()` 当前支持 AF_UNIX。
- `bind/listen/connect/accept/send/recv/sendto/recvfrom/getsockname/getpeername/shutdown` 分散在 `network/**` ops 文件。
- `setsockopt/getsockopt` 同时识别 AF_INET 和 AF_UNIX socket 文件。

## 目标

- syscall 层只处理 ABI: 用户指针, sockaddr 编解码, fd table 和 errno。
- socket 文件保存 fd 局部状态, 如 flags, endpoint, shutdown 和 per-fd queue。
- 协议栈行为通过 `NetworkStack` 或 `UnixSocketFile` 方法进入。

## 非目标

- 不在 syscall 层匹配 smoltcp TCP 内部状态。
- 不把用户传入的 sockaddr 或 buffer 指针保存到 socket 对象。
- 不在本文维护完整 errno 表。

## SocketFile 边界

`SocketFile` 保存:

- optional `SocketHandle`。
- listener backlog 和 listen queue。
- local/remote endpoint。
- UDP per-fd receive queue。
- shutdown 状态。
- status flags 和 socket options。

`SocketFile` 实现 VFS `File`, 但实际 read/write/readable/writable/drop 都委托给 `NetworkStack`。

## UnixSocketFile 边界

AF_UNIX 不使用 smoltcp。它保存:

- path 或 abstract address binding。
- stream connection buffer。
- datagram queue。
- listener pending queue。
- shutdown 状态和 socket options。

路径绑定会在 VFS 中创建 socket node, abstract binding 只在内核表中注册。

## Syscall 边界

`os/src/kernel/syscall/network/` 负责:

- 解析 domain/type/protocol 和 `SOCK_NONBLOCK`, `SOCK_CLOEXEC`。
- 从 fd table 取得文件对象并 downcast 到 `SocketFile` 或 `UnixSocketFile`。
- 复制用户 sockaddr, buffer 和 optval。
- 写回 fd, sockaddr 和 optlen。
- 将 `FsError`/`NetworkError` 转换成 Linux errno。

## I/O 和 poll 协作

通用 `read/write` 在 `io.rs` 中处理 `WouldBlock` 重试。对于网络 socket:

- 阻塞路径会先请求 `poll_network_and_dispatch()`。
- 让出 CPU 后检查可投递信号, 需要时返回 `EINTR`。
- `file_read_ready()` 和 `file_write_ready()` 使用 socket 文件的 `readable/writable`。
- 网络状态变化通过 `wake_poll_waiters()` 唤醒等待者。

## 并发和生命周期约束

- fd 生命周期结束时, AF_INET `SocketFile::drop()` 必须释放 stack handle。
- exit cleanup 关闭 fd 时会清理 `(tid, fd) -> SocketHandle` 映射。
- UDP per-port 共享 socket 用 weak fd list, 避免 fd drop 后悬挂强引用。
- AF_UNIX binding 在 `Drop` 中移除, stream peer shutdown 会唤醒等待者。

## 已知限制

- AF_INET 只覆盖当前测试所需 TCP/UDP 路径。
- AF_UNIX 是最小本地 socket 实现, 不是完整 Linux unix socket。
- 网络 syscall 的阻塞重试依赖当前 `io.rs` 的 yield/poll 模型, 尚非完整调度等待队列模型。

## 源码索引

- `os/src/net/socket.rs`: AF_INET `SocketFile`, fd/socket map, sockaddr_in helper。
- `os/src/net/unix_socket.rs`: AF_UNIX socket。
- `os/src/kernel/syscall/network/socket_ops.rs`: socket, bind, listen, accept。
- `os/src/kernel/syscall/network/connection_ops.rs`: connect, send, recv, shutdown。
- `os/src/kernel/syscall/network/addr_ops.rs`: sendto, recvfrom, getsockname, getpeername。
- `os/src/kernel/syscall/network/sockopt_ops.rs`: sockopt。
- `os/src/kernel/syscall/io.rs`: read/write/poll retry 边界。
