# SysV 共享内存

SysV shared memory 当前由全局 segment registry 和每个任务的 attachment table 共同管理。registry 管理 `shmid/key` 生命周期, task attachment table 管理某个进程地址空间里的映射关系。

## 当前状态

- `shmget` 创建或查找 segment。
- `shmat` 把 segment 的物理页映射进当前 `MemorySpace`。
- `shmdt` 从当前进程分离指定映射。
- `shmctl` 当前支持 `IPC_STAT` 和 `IPC_RMID`。
- exit 和 exec 都会分离当前进程持有的 shm attachments。
- clone/fork 会根据是否共享 VM 决定 attachment table 是共享还是复制, 非线程 clone 会增加 segment attach 计数。

## 目标

- 把 SysV shm 的命名, 权限和删除标记放在全局 registry。
- 把实际页映射放在 `MemorySpace`, 让共享数据面走页表而不是内核拷贝。
- 让 exit/exec cleanup 能统一回收映射并更新 attach 计数。

## 非目标

- 当前不支持 huge page shm。
- 当前不维护完整 Linux shm tunables 和 namespace。
- 不在本文列出所有 flag 和 errno 分支, 以 `uapi::ipc` 和 syscall 源码为准。

## Segment registry

全局 `SHM_REGISTRY` 保存两张索引:

- `by_id`: `shmid -> Arc<ShmSegment>`。
- `by_key`: `key -> shmid`, `IPC_PRIVATE` 不进入 key 索引。

`ShmSegment` 保存 segment 元数据, 物理页 frame 列表和受锁保护的运行时状态:

- `marked_removed`: 是否已被 `IPC_RMID` 标记删除。
- `attach_count`: 当前 attach 数。
- `atime/dtime/ctime/lpid`: SysV stat 所需的时间和最后操作 pid。

删除采用延迟语义: `IPC_RMID` 先移除 key 可见性并标记 removed, 如果仍有 attachment, segment 会等最后一次 detach 后再从 registry 移除。

## shmget 生命周期

`shmget` 的核心语义是"按 key 查找或创建":

1. 拒绝当前不支持的 huge page flag。
2. 非 `IPC_PRIVATE` key 先查 `by_key`。
3. 已存在且未删除时, 校验 `IPC_CREAT|IPC_EXCL`, size 和访问权限。
4. 不存在且未指定 `IPC_CREAT` 时返回 not found。
5. 创建新 segment, 分配页帧, 建立 `by_id` 和可选 `by_key` 索引。

## shmat 映射关系

`shmat` 把 registry 中的 segment 接入当前进程:

1. 查找 segment 并校验读写权限。
2. 根据 `shmaddr`, `SHM_RND`, `SHM_REMAP` 和空地址 hint 选择起始地址。
3. 计算覆盖 segment 页数的 `VpnRange`。
4. 根据 `SHM_RDONLY` 和 `SHM_EXEC` 构造页表权限。
5. 调用 `MemorySpace::insert_shared_area()` 把 segment frames 映射进当前地址空间。
6. 更新 segment attach 计数, 并把 `ShmAttachment` 写入当前 task 的 attachment table。

attachment table 以用户虚拟起始地址为 key, 保存地址, 长度和 segment 引用。它是后续 `shmdt`, exit 和 exec cleanup 的依据。

## shmdt 分离关系

`shmdt` 只接受当前进程已经 attach 的起始地址:

1. 地址必须页对齐。
2. 从 attachment table 移除对应 attachment。
3. 调用 `MemorySpace::munmap()` 解除虚拟地址映射。
4. 调用 `shm_detach_segment()` 更新 segment detach 时间, last pid 和 attach count。
5. 如果 segment 已被 `IPC_RMID` 标记且 attach count 归零, registry 移除 segment。

若 `munmap` 失败, syscall 会把 attachment 放回 table, 保持 task 元数据和地址空间一致。

## shmctl 控制关系

- `IPC_STAT`: 校验只读权限, 把 segment stat 写回用户缓冲区。
- `IPC_RMID`: 校验 owner 或 `IPC_OWNER` capability, 标记删除并从 key 索引摘除。

`IPC_RMID` 不强制拆除其他进程已映射的地址, 它只阻止后续按 key 查找并等待最后 detach 完成实际释放。

## exit/exec cleanup

`detach_all_shm()` 是统一清理入口:

1. 从 task 取出 `memory_space` 和 `shm_attachments`。
2. 用 `mem::take` 清空 attachment table, 避免长时间持有 task 锁。
3. 对每个 attachment 执行 `MemorySpace::munmap()`。
4. 对每个 segment 调用 `shm_detach_segment()` 更新 registry 状态。

exit 路径在关闭 fd 后调用它, exec 路径在切换到新地址空间前调用它。这样旧程序的 SysV shm 映射不会泄漏到新程序。

## clone/fork 关系

- `CLONE_VM` 共享地址空间时, attachment table 也随线程共享。
- 非线程 clone/fork 会复制 attachment table, 并为复制出的每个 attachment 增加 segment attach count。
- 新地址空间由 `clone_for_fork()` 复制, shared area 的物理页仍指向相同 segment frames。

## 并发和生命周期约束

- registry 由 `SpinLock<ShmRegistry>` 保护。
- 每个 segment 的 attach/removal 状态由 segment 内部锁保护。
- task attachment table 由 `SpinLock<BTreeMap<usize, ShmAttachment>>` 保护。
- cleanup 先取走 attachment 元数据再 munmap 和更新 registry, 避免 task -> memory_space -> registry 的长锁链。

## 已知限制

- 不支持 `SHM_HUGETLB`。
- `shmctl` 当前只覆盖 `IPC_STAT` 和 `IPC_RMID`。
- 没有完整 shm namespace, limits 和 accounting。
- 权限模型已有 owner/group/other 和 `IPC_OWNER` 检查, 但不是完整 Linux IPC namespace 语义。

## 源码索引

- `os/src/ipc/shared_memory.rs`: `ShmSegment`, registry, 权限和删除语义。
- `os/src/kernel/syscall/ipc.rs`: `shmget`, `shmat`, `shmdt`, `shmctl`。
- `os/src/kernel/task/mod.rs`: `detach_all_shm()` 和 exit cleanup。
- `os/src/kernel/syscall/task/exec_ops.rs`: exec 前 detach shm。
- `os/src/kernel/syscall/task/clone_ops.rs`: clone/fork 的 attachment table 复制和 attach count。
- `os/src/mm/memory_space/`: shared area 映射和 `munmap()`。
