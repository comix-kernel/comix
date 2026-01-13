# netperf / netserver 测试说明（已知现象）

本页用于记录在 ComixOS 上运行 `netperf/netserver` 时的测试方法与当前已知现象，便于后续回归与排查。

## 如何运行

仓库内提供了脚本（不建议修改脚本本身）：

- `data/netperf_testcode.sh`

在系统内启动后执行：

```sh
./netperf_testcode.sh
```

该脚本会：

1. 后台启动 `netserver`（`-D` 守护模式），监听 `127.0.0.1:12865`
2. 依次运行 `netperf` 的 `UDP_STREAM/TCP_STREAM/UDP_RR/TCP_RR/TCP_CRR`
3. 结束时 `kill -9` 掉 `netserver` 进程

## 预期结果

脚本应当能够完整跑完并返回 shell，且各测试段落会打印 `end: success`。

## 已知现象：`accept_connections: select failure: Interrupted system call (errno 4)`

在脚本尾部（或某些测试段落结束后）可能看到如下输出：

```
accept_connections: select failure: Interrupted system call (errno 4)
```

说明：

- 该信息来自 `netserver`：其内部 `select()` 被信号打断，返回 `EINTR (errno=4)` 后打印该告警。
- **这不代表脚本失败**：脚本仍可能全部测试通过并正常返回 shell。
- 触发时机通常与 `netserver` 的守护/回收子进程逻辑相关（例如收到 `SIGCHLD` 等），而 `netserver` 对 `EINTR` 的处理方式是直接打印错误并退出/回到外层循环。

### 为什么暂时不在脚本层修复

用户侧脚本已固定（且多处依赖其输出格式），修改脚本容易引入额外差异，不利于对内核兼容性的验证。

### 后续若要消除该输出（不改脚本）的方向

需要在内核或用户态二进制中做其一：

1. **内核实现更完整的 `SA_RESTART` / syscall restart 语义**（让 `select/poll` 在特定信号到来后自动重启，尽量不向用户态暴露 `EINTR`）。
2. **修改/替换 `netserver`**：将 `select()` 返回 `EINTR` 视为可重试（不打印错误、直接继续）。

