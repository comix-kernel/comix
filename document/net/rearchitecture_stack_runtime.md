# 协议栈运行时重构说明

本文档覆盖 `net::stack`。该层是 `smoltcp` 的唯一宿主，负责统一持有协议栈状态、socket 集合、设备适配器和 poll 推进逻辑。

## 当前代码

- `os/src/net/socket.rs` 定义 `SOCKET_SET`、`NET_IFACE`、`FD_SOCKET_MAP`、`NetIfaceWrapper`。
- `os/src/net/interface.rs` 定义 `SmoltcpInterface` 和 `NetDeviceAdapter`。
- `poll_network_interfaces()`、`poll_network_and_dispatch()`、`poll_until_empty()` 分散承担协议栈推进。
- UDP per-port dispatcher 与 TCP pending close 也在 `socket.rs` 内部。

## 目标对象

建议引入：

- `NetworkStack`：网络协议栈对外门面。
- `StackRuntime`：单接口或多接口的 smoltcp runtime。
- `StackSocketId`：网络子系统内部 socket id，不直接等同于 smoltcp handle。
- `StackPollResult`：poll 后对 VFS/poll waiters 发布的事件摘要。

`NetworkStack` 内部独占持有：

- `smoltcp::iface::Interface`
- `smoltcp::iface::SocketSet`
- `NetDeviceAdapter`
- UDP dispatch 表
- TCP pending close 表
- waiters 唤醒所需事件摘要

## 对外 API

第一阶段 API 可以保持小而稳定：

```rust
impl NetworkStack {
    pub fn create_socket(&self, kind: SocketKind) -> Result<StackSocketId, NetError>;
    pub fn bind(&self, id: StackSocketId, endpoint: NetEndpoint) -> Result<(), NetError>;
    pub fn listen(&self, id: StackSocketId, backlog: usize) -> Result<(), NetError>;
    pub fn accept(&self, id: StackSocketId) -> Result<AcceptedSocket, NetError>;
    pub fn connect(&self, id: StackSocketId, remote: NetEndpoint, local: NetEndpoint) -> Result<(), NetError>;
    pub fn send(&self, id: StackSocketId, data: &[u8]) -> Result<usize, NetError>;
    pub fn recv(&self, id: StackSocketId, data: &mut [u8]) -> Result<usize, NetError>;
    pub fn poll(&self) -> StackPollResult;
}
```

这里的 `NetEndpoint`、`NetError` 应是网络子系统自有类型，避免 syscall 层直接依赖 smoltcp 类型。

## 必做改造

1. 把 `SmoltcpInterface` 下沉到 `net::stack`，不再从接口层公开返回。
2. 把 `NetDeviceAdapter` 移到 `net::stack` 内部。
3. 把 `SOCKET_SET` 和 `NET_IFACE` 收进 `NetworkStack`，旧全局符号改成兼容包装。
4. 把 UDP dispatch、TCP pending close、poll waiters 唤醒统一放入 `NetworkStack::poll()`。
5. 所有 socket 操作完成后如需推进协议栈，只调用 `NetworkStack::poll()`。

## 锁顺序

推荐固定为：

`NetworkStack -> InterfaceRuntime -> SocketSet -> SocketFile local state`

禁止：

- syscall 层先锁 `SocketFile` 后抓裸 `SOCKET_SET`。
- 中断路径直接抓 `SocketSet` 并执行 smoltcp poll。
- 持有网络栈锁时复制用户缓冲区。

## 验收点

- `smoltcp` import 集中在 `net::stack` 和少量兼容转换模块。
- `SOCKET_SET` 不再作为裸全局暴露给 syscall。
- `NetworkStack::poll()` 是唯一协议栈推进入口。
- UDP 可读事件、TCP close reaping、waiter wakeup 都在 poll 结束阶段集中发布。

## 当前执行状态

- `SOCKET_SET` 与 `NET_IFACE` 已移入 `net::stack`，`socket.rs` 仅作为 runtime 实现模块以 `pub(crate)` 方式引用。
- `NetworkStack` 已覆盖 socket 创建、TCP connect/listen/state/close/endpoints、UDP dispatch/attach、poll、SocketFile read/write/readable/writable/drop/sendto/recvfrom。
- UDP dispatch、TCP pending close reaping 和 poll waiter 唤醒仍在统一 poll 路径中执行。
- loopback 兼容队列已从 `NetDeviceAdapter` 字段移入 `net::stack` runtime 的 loopback link。

保留的兼容点：

- `NetIfaceWrapper` 仍定义在 `socket.rs`，作为旧单接口 runtime 的实现细节；后续可整体搬入 `stack.rs` 或 `stack/runtime.rs`。
- `SocketHandle` 仍等同于 smoltcp handle 包装，暂未引入多 runtime 安全的 socket id。
