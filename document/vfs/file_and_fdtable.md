# File 与 FDTable

`File` 表示一次打开的文件会话, `FDTable` 表示进程可见的文件描述符空间. 这两个对象共同实现 POSIX open file description 的核心语义.

## 当前状态

- `FDTable` 内部保存 fd 到 `Arc<dyn File>` 的映射, 并为每个 fd 保存 fd flags.
- `File` trait 由普通文件, 管道, stdio, 字符设备文件和块设备文件实现.
- 普通文件 `RegFile` 持有 `Dentry`, 通过 dentry 的 inode 读写.
- `dup` 系列复制 fd slot, 但共享同一个 `Arc<dyn File>`.
- `O_CLOEXEC` 会转换为 fd flag, exec 前由 fd table 清理.

## 目标

- fd 分配和打开文件对象解耦.
- 支持最小可用 fd, dup, close-on-exec 等常见 POSIX 行为.
- 支持异构文件类型共存在同一个 fd table.
- 让普通文件 offset 属于 open session, 不是 inode.

## 非目标

- 不在文档中维护完整 fd table 方法清单.
- 不描述每个系统调用的参数检查分支.
- 不把 socket 生命周期纳入本页, socket fd 有额外网络层映射.

## 模块边界

- `fd_table.rs` 只管理 fd slot 和 fd flags.
- `file.rs` 定义打开会话能力.
- `impls/reg_file.rs` 桥接普通文件和 inode.
- `impls/pipe_file.rs` 管理流式管道缓冲区.
- `impls/char_dev_file.rs` 和 `impls/blk_dev_file.rs` 把设备 inode 转成驱动访问.
- `impls/stdio_file.rs` 绑定标准输入输出.

## 关键流程

### open 后安装 fd

```text
Dentry
  -> File implementation
  -> Arc dyn File
  -> FDTable alloc
  -> fd
```

fd table 不关心 file 的具体类型. 类型差异都隐藏在 `File` trait object 后面.

### dup

```text
fd 3 -> Arc File A
fd 4 -> Arc File A
```

dup 后两个 fd 指向同一个 `File`, 因此普通文件的 offset 共享. 这是和再次 open 同一路径的主要区别.

### close

close 只清空 fd slot. 如果这是最后一个 `Arc<dyn File>`, 对象才会 drop. 对普通文件而言, 数据同步仍由具体文件系统和 `sync` 路径负责, close 本身不是完整 fsync.

### exec

exec 前 fd table 会关闭带 close-on-exec flag 的 fd. open flags 和 fd flags 分开保存, 因为 dup 共享文件状态 flags, 但 fd flags 属于单个 descriptor.

## 并发和生命周期约束

- `FDTable` 用锁保护 slot 向量, `File` 自身必须 `Send + Sync`.
- fd table 操作的锁只保护映射关系, 不保护文件内容.
- `RegFile` 的 offset 是会话状态, 共享同一个 `RegFile` 的 fd 会共享 offset.
- 管道和设备文件有自己的同步约束, 不能假设它们支持 seek.

## 已知限制

- fd table 是简单向量结构, 适合当前最大 fd 数, 没有复杂稀疏 fd 管理.
- close 不自动清理所有外部子系统映射, 特殊 fd 需要调用方配合.
- 非阻塞语义和 poll/select 相关能力仍由各 file 实现和系统调用层逐步补齐.

## 源码索引

- `os/src/vfs/fd_table.rs`: fd slot, fd flags, dup, close, take_all.
- `os/src/vfs/file.rs`: 打开文件会话接口.
- `os/src/vfs/impls/reg_file.rs`: 普通文件 offset 和 inode 转发.
- `os/src/vfs/impls/pipe_file.rs`: 管道 file.
- `os/src/vfs/impls/stdio_file.rs`: stdin/stdout/stderr.
- `os/src/vfs/impls/char_dev_file.rs`: 字符设备文件.
- `os/src/vfs/impls/blk_dev_file.rs`: 块设备文件.
