# 网络测试与排查

## 基础检查

```bash
cd os
cargo fmt
cargo check
```

## 功能矩阵

- 启动后接口枚举能看到 `lo`。
- 有真实 VirtIO NIC 时接口枚举能看到 `ethN`。
- loopback-only 场景能使用 `127.0.0.1/8`。
- `socket/bind/listen/accept/connect/send/recv/sendto/recvfrom` 基础路径可用。
- `ppoll/select` 能观察 TCP accept、TCP recv 和 UDP recv 可读事件。
- netperf/netserver 结果与 `netperf.md` 中记录的 EINTR 现象一致。

## 常见问题

- 如果 syscall 层出现 `SOCKET_SET`、`NET_IFACE` 或 `smoltcp::socket` import，说明边界回退了。
- 如果有真实 NIC 时 127.0.0.1 流量尝试走设备发送，检查 `NetTxToken` 的 loopback frame 判定和 stack loopback link。
- 如果 UDP poll/select 不醒，检查 `udp_dispatch_drain_locked()` 是否在 poll 路径执行，并确认 per-fd queue 是否收到 datagram。
