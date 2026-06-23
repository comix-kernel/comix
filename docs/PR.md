# PR: 提升 OS Contest 测试兼容性与文件系统/syscall 语义

## 背景

本分支基于 `main`，围绕 OS Contest 官方测试镜像运行过程中的实际缺口，补齐了一批 Linux/POSIX 兼容语义。改动重点集中在 VFS、tmpfs/ext4、文件相关 syscall、部分网络 syscall，以及官方测试常见的 BusyBox/musl 行为。

分支范围：

- 当前分支：`ccy-dev01`
- 对比基线：`main`
- 提交数量：41
- 主要影响目录：`os/src/vfs`、`os/src/fs`、`os/src/kernel/syscall`、`os/src/net`

## 改动摘要

本分支主要完成以下内容：

- 支持 mknod 设备号编码/解码，并在 `stat` 结果中正确报告 `st_rdev`。
- 补齐 tmpfs/ext4 对字符设备、块设备、FIFO、socket、普通文件等特殊节点的元数据支持。
- 支持 named FIFO 打开语义、pipe size 查询和设置边界。
- 新增最小 Unix domain socket 支持，覆盖基础本地 socket 使用场景。
- 实现 `linkat`、tmpfs hard link、tmpfs rename，并修正跨设备 link、unlinkat removedir、rmdir link count 等语义。
- 收紧 `openat`、`faccessat`、`fchmodat`、`fchownat`、`statx/fstatat`、`utimensat` 等 syscall 的 flag 和空路径处理。
- 补齐 `O_NOFOLLOW`、`O_CLOEXEC`、`F_DUPFD` 下界、`getcwd` 小缓冲区返回 `ERANGE` 等 Linux 行为。
- 保留路径最后组件的 `.`、`..` 和尾部 `/` 语义，按 syscall 分别返回更接近 Linux 的错误码。
- 强化 `renameat2` 对空路径、特殊组件、尾部斜杠、目标类型校验的处理。
- 支持 `fchmodat(..., "", mode, AT_EMPTY_PATH)`，修复 BusyBox `cp` preserve permissions 阶段返回 `EINVAL` 的问题。

## 文件系统与 VFS

### 设备号与特殊文件

新增并使用 Linux 设备号编码/解码辅助函数，使 `mknodat` 创建的字符设备、块设备可以在 `stat` 中正确反映 `st_rdev`。

涉及内容：

- VFS 设备号 helper。
- `mknodat` 解码用户态传入的 `dev_t`。
- `stat`/`statx` 输出设备号。
- tmpfs/ext4 保存特殊 inode 的 rdev 和 file type。

### tmpfs 语义增强

tmpfs 增强了特殊文件和目录项操作支持：

- 精确解析 `mknod` 文件类型。
- 支持 FIFO、字符设备、块设备、socket、普通文件的创建元数据。
- 支持 hard link。
- 支持 rename。
- rmdir 时维护 link count。
- chmod/chown/timestamps 等元数据路径保持可写。

### ext4 语义增强

ext4 侧补齐：

- mknod 设备元数据。
- rename 目标类型校验。
- 目录覆盖非目录返回 `ENOTDIR`。
- 非目录覆盖目录返回 `EISDIR`。

### FIFO 与 pipe

新增 named FIFO 打开支持，并改进 pipe 相关行为：

- `open` FIFO 时返回 pipe file。
- `F_GETPIPE_SZ` 返回实际 pipe size。
- `F_SETPIPE_SZ` 对过小值进行最小值 clamp。

## syscall 兼容性

### 路径和空路径处理

本分支修复了一批空路径行为：

- `openat("")` 返回 `ENOENT`。
- `readlinkat("")` 返回 `ENOENT`。
- `faccessat("")` 返回 `ENOENT`。
- `fchmodat/fchownat("")` 在无 `AT_EMPTY_PATH` 时返回 `ENOENT`。
- `linkat` 源路径为空返回 `ENOENT`。
- `renameat2` old/new path 为空返回 `ENOENT`。

同时新增 `split_parent_preserving_basename()`，避免全局路径拆分提前吞掉最后组件 `.` 和 `..`。不同 syscall 对 final component 的语义不同，因此本分支没有粗暴修改全局 `split_path`，而是在 syscall 层分别处理。

### open/fcntl

改动包括：

- `openat` 支持 `O_NOFOLLOW`。
- `openat` 支持 `O_CLOEXEC`。
- `openat` 拒绝 `O_CREAT` 创建目录目标的错误路径。
- `fcntl(F_DUPFD)` 校验 lower bound。
- pipe size 查询和设置更贴近 Linux 行为。

### access/stat/chmod/chown/utimensat

补齐内容：

- `faccessat` flag 校验。
- `fstatat/statx` flag 收紧。
- `utimensat` 支持 `AT_EMPTY_PATH`。
- `fchmodat/fchownat` flag 校验。
- `fchmodat` 支持 `AT_EMPTY_PATH`，可通过 fd 对已打开文件 chmod。

### mknod

本分支修复：

- `mknod(path, 0644)` 默认创建普通文件。
- `mknod(S_IFDIR)` 返回 `EPERM`。
- mknod 目标路径尾部斜杠按 Linux 错误码处理。
- mknod 特殊文件类型和设备号在 tmpfs/ext4/stat 之间贯通。

### link/unlink/rename

新增/修复：

- 实现 `linkat`。
- tmpfs 支持 hard link。
- 跨设备 hard link 返回 `EXDEV`。
- `unlinkat(..., AT_REMOVEDIR)` 分流到 rmdir。
- tmpfs 支持 rename。
- `renameat2` 处理空路径、`.`/`..`、尾部斜杠、目标类型冲突。
- ext4 rename 校验目录和非目录覆盖类型。

### 尾部斜杠语义

本分支对不同 syscall 分别实现尾部 `/` 行为：

- `mkdirat("new/")`、`mkdirat("new//")` 可以成功。
- `mkdirat("file/")`、`mkdirat("dir/")`、`mkdirat("/")` 返回 `EEXIST`。
- `mknodat/symlinkat/linkat` 创建目标带尾部 `/` 时按 Linux 行为区分 `ENOENT`、`EEXIST`、`ENOTDIR`。
- `renameat2` 源路径和目标路径尾部 `/` 分别校验是否必须为目录。

## 网络

新增最小 Unix domain socket 支持：

- 新增 `os/src/net/unix_socket.rs`。
- 扩展 socket syscall 路径，支持基础 Unix socket 创建、地址绑定、连接、收发和选项处理。
- 补齐 socket 相关错误码和地址处理边界。

这部分主要用于提升 BusyBox、libc、网络测试中对本地 socket 的基础兼容性，不等同于完整 Linux Unix socket 实现。

## 主要提交列表

```text
f06cbeb vfs: add device number encoding helpers
2dead4e syscall: decode mknodat device numbers
f4fa8dd vfs: report device numbers in stat results
57561b1 tmpfs: parse mknod file types exactly
c82d7ef vfs: derive equality for file modes
ac8c926 ext4: support mknod device metadata
4e9aa7d vfs: support opening named fifos
3c80b50 vfs: invalidate global dentry cache on removal
393e80f net: add minimal unix domain sockets
87343be syscall: implement linkat
051de17 tmpfs: support hard links
08f3a8d tmpfs: support rename
57c42bc syscall: route unlinkat removedir to rmdir
c1a9ed1 vfs: return exdev for cross-device links
dccf4eb tmpfs: update link counts on rmdir
3598bbd syscall: validate chmod chown at flags
766a8b9 syscall: support utimensat empty path
35384fa syscall: tighten stat at path flags
facacc6 syscall: reject creat directory opens
b4def61 fcntl: return actual pipe size
8b1d325 pipe: clamp pipe size to minimum
e260f7f syscall: validate faccessat inputs
5965e7d syscall: preserve final path components
15dc8b2 syscall: reject empty open paths
084b58c syscall: support openat nofollow
d881e35 fcntl: validate dupfd lower bound
a262d98 syscall: honor openat cloexec
1590f6a syscall: reject empty readlink paths
0c95fc6 syscall: reject empty chmod chown paths
690ae3a syscall: reject empty access paths
acbf5d6 syscall: reject empty link sources
dd22c98 syscall: default mknod type to regular file
78c711c syscall: reject directory mknod with eperm
42e1b96 syscall: report erange for small getcwd buffers
0446089 syscall: reject empty rename paths
35fd318 syscall: preserve special rename components
e7e0e0c syscall: handle rename trailing slashes
563ac69 ext4: validate rename target types
035c64f syscall: allow mkdir trailing slashes
cba140e syscall: handle creation trailing slashes
0fda713 syscall: support fchmodat empty path
```

## 验证

已使用官方测试机镜像进行格式和编译检查：

```bash
docker run --rm -v "$PWD":/workspace -w /workspace/os \
  zhouzhouyi/os-contest:20260510 \
  bash -lc 'cargo fmt --check && cargo check'
```

结果：通过。

说明：

- 检查输出中仍有大量既有 warning，本 PR 未处理这些存量 warning。
- 本地宿主机上 `os/target` 和 `os/fs-riscv.img` 曾因测试镜像生成物归属为 root 导致直接 `cargo check` 权限失败，因此最终以指定 Docker 镜像内结果为准。

## 风险与边界

- Unix domain socket 是最小可用实现，不是完整 Linux 语义。
- 多数路径语义修复按当前测试和 Linux 探测行为实现，仍可能存在少量 errno 优先级差异。
- ext4 写路径仍可能受底层库写放大影响，性能问题不在本 PR 中完全解决。
- 本 PR 强化了大量 syscall flag 校验，若用户程序依赖旧的宽松行为，可能暴露新的错误返回。

## 后续建议

- 继续按官方评分失分项补齐 glibc-rv、musl-la 启动和基础测试输出。
- 针对 BusyBox 文件写入、复制、删除、文本处理类命令继续定位。
- 对 iperf TCP、reverse TCP、parallel TCP 做网络路径专项排查。
- 对大型 benchmark 和 LTP 分阶段补 syscall 覆盖，不建议一次性按全量 LTP 推进。
