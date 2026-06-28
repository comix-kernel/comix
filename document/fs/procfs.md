# ProcFS

ProcFS 把进程和系统运行时状态暴露为 `/proc` 文件树. 它是动态伪文件系统, 文件内容通常在读取时生成.

## 当前状态

- 源码位于 `os/src/fs/proc/`.
- `ProcFS::init_tree` 创建固定根条目, 如 `meminfo`, `uptime`, `cpuinfo`, `mounts`, `psmem`, `self`.
- 进程相关路径由 proc inode/generator 动态提供.
- 文件内容由 generator 生成, 不落盘.
- 部分动态 inode 使用非缓存策略, 避免进程退出后路径陈旧.

## 目标

- 为用户态工具提供 Linux 风格 `/proc` 入口.
- 暴露进程, 内存, CPU, mount 等调试信息.
- 让动态内容以 VFS inode 方式接入, 不绕过路径和 fd 模型.

## 非目标

- 不实现完整 Linux procfs.
- 不保证每个字段和 Linux 完全一致.
- 不把 generator 的输出格式细节写入设计文档.

## 模块边界

- `proc.rs`: 文件系统实例和根树初始化.
- `inode.rs`: proc inode 类型, 动态文件, 动态 symlink, 目录行为.
- `generators/`: 具体内容生成器.
- `generators/process/`: 进程相关文件内容.

## 关键流程

### read dynamic file

```text
vfs lookup /proc/...
  -> ProcInode
  -> generator snapshot
  -> File read path returns bytes
```

generator 应尽量生成一个一致的快照, 避免读取过程中依赖长期锁.

### /proc/self

`/proc/self` 是动态 symlink, 每次解析时根据当前任务 pid 指向对应进程目录.

## 并发和生命周期约束

- 进程状态会变化, generator 需要容忍目标进程退出.
- 动态进程路径不应无条件缓存 dentry.
- 读取 proc 文件不应长期阻塞全局调度或任务锁.

## 已知限制

- 当前 procfs 以只读信息为主.
- Linux 工具依赖的某些 `/proc` 文件和字段尚未实现.
- `/proc/mounts` 反映当前 VFS mount table 的可见状态, 不是完整 namespace 视图.

## 源码索引

- `os/src/fs/proc/proc.rs`: `ProcFS` 和根树初始化.
- `os/src/fs/proc/inode.rs`: proc inode 类型.
- `os/src/fs/proc/generators/`: 系统级动态文件.
- `os/src/fs/proc/generators/process/`: 进程级动态文件.
- `os/src/fs/tests/proc/`: procfs 测试.
