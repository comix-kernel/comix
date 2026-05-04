# Socket 与 syscall 重构说明

本文档覆盖 `SocketFile`、fd 映射和 `kernel::syscall::network`。目标是让 syscall 层只处理用户 ABI，底层网络行为全部转发给 `NetworkStack`。

## 当前代码

- `os/src/kernel/syscall/network.rs` 直接 import `SOCKET_SET`、`SocketHandle`、`smoltcp::socket::{tcp, udp}`。
- `SocketFile` 在 `os/src/net/socket.rs` 中直接读取 `SOCKET_SET`。
- `FD_SOCKET_MAP` 使用 `(tid, fd) -> SocketHandle` 记录 fd 与 smoltcp handle 的关系。
- `set_network_interface_config()` 仍返回 `-1/-2/-3` 这类私有错误码。

## 目标边界

syscall 层只负责：

- 提取架构层传入的 syscall 参数。
- 使用 `SumGuard` 访问用户指针。
- sockaddr 与用户缓冲区的 ABI 编解码。
- fd table 查询和 `SocketFile` 创建。
- 将请求转发到 `NetworkStack`。
- 返回标准 Linux errno。

syscall 层禁止：

- 直接访问 `smoltcp::SocketSet`。
- 直接匹配 `tcp::State` 或 `udp::Socket`。
- 直接推进设备 poll。
- 直接读取或修改 `NET_IFACE`。

## SocketFile 职责

`SocketFile` 保留为 VFS 文件对象，但只保存逻辑状态：

- `StackSocketId`
- local endpoint
- remote endpoint
- flags/options
- listener backlog
- shutdown 状态
- UDP per-fd 接收队列

`SocketFile::read()`、`write()`、`readable()`、`writable()` 调用 `NetworkStack` 方法，不直接锁 `SocketSet`。

## 必做改造

1. 定义 `StackSocketId`，替代公开传递的 `SocketHandle`。
2. 将 `FD_SOCKET_MAP` 的 value 从 smoltcp handle 改为 `StackSocketId`；更理想的是让 fd 的 `SocketFile` 成为唯一映射源。
3. 将 `create_tcp_socket()`、`create_udp_socket()` 改为 `NetworkStack::create_socket()`。
4. 将 `tcp_connect()`、`socket_sendto()`、`udp_attach_fd_to_port()` 等函数改为 `NetworkStack` 方法。
5. `kernel::syscall::network.rs` 中的 `SOCKET_SET` 和 `smoltcp` import 必须消失。
6. 所有网络 syscall 统一使用 `uapi::errno`，不再返回私有负数。

## 阻塞与非阻塞语义

- 非阻塞 fd 遇到暂不可读/写返回 `EAGAIN`。
- 阻塞路径可以循环调用 `NetworkStack::poll()`、`yield_task()`、信号中断检查。
- poll/select 的可读可写判断必须依赖 `SocketFile` 的公开方法，而这些方法内部转发到 `NetworkStack` 查询。

## 验收点

- `socket()` 只创建 `SocketFile` 和网络栈 socket，不分配虚假 fd。
- `bind/listen/connect/accept/send/recv/sendto/recvfrom` 都经 `NetworkStack`。
- `getsockname/getpeername` 不直接读取 smoltcp socket。
- 用户指针访问不发生在持有网络栈内部锁期间。

## 当前执行状态

- `kernel::syscall::network` 已不再 import `SOCKET_SET`、`NET_IFACE` 或 `smoltcp::socket::{tcp, udp}`。
- `listen/accept/connect/shutdown/getsockname/getpeername` 通过 `NetworkStack` 查询或修改协议栈状态。
- `SocketFile::read/write/readable/writable/recvfrom/drop` 已改为调用 `NetworkStack` 文件级 API。
- `set_network_interface_config()` 已从私有 `-1/-2/-3/-4/-5` 返回值改为 `uapi::errno`。

保留的兼容点：

- `FD_SOCKET_MAP` 的 value 仍是 `SocketHandle`，尚未替换为真正独立的 `StackSocketId`。
- `socket.rs` 仍包含 socket runtime 的 smoltcp 具体操作，但这些操作已位于 `NetworkStack` 调用边界之后。
