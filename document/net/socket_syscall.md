# Socket 与 syscall

## SocketFile

`SocketFile` 是 VFS 文件对象，保存 fd 相关逻辑状态：

- stack socket handle。
- listener backlog 和 accept 队列。
- local/remote endpoint。
- UDP per-fd receive queue。
- shutdown、flags、socket options。

`SocketFile::read()`、`write()`、`readable()`、`writable()`、`recvfrom()` 和 drop 路径通过 `NetworkStack` 方法进入协议栈。

## Syscall 边界

`os/src/kernel/syscall/network.rs` 负责：

- 解析 syscall 参数。
- 使用 `SumGuard` 访问用户指针。
- 编解码 sockaddr。
- 操作 fd table。
- 将网络行为转发给 `NetworkStack` 或 socket 文件对象。
- 返回 Linux errno。

syscall 层禁止：

- import `SOCKET_SET` 或 `NET_IFACE`。
- import `smoltcp::socket::{tcp, udp}`。
- 匹配 smoltcp TCP state。
- 持有协议栈锁时复制用户缓冲区。

## Errno

网络 syscall 应使用 `uapi::errno` 或 `FsError::to_errno()`。`set_network_interface_config()` 这类接口配置 syscall 不返回私有 `-1/-2/-3` 错误码。
