# Inode 与 Dentry

`Inode` 和 `Dentry` 是 VFS 中最容易混淆的两个对象. 简单说, inode 表示"文件系统里的对象", dentry 表示"路径树里某个名字指向这个对象".

## 当前状态

- `Inode` 由具体文件系统实现, 比如 ext4 inode, tmpfs inode, proc inode, sysfs inode, VFAT inode.
- `Dentry` 由 VFS 创建和缓存, 保存 name, parent, children, inode 和 mount 关系.
- `DENTRY_CACHE` 保存 full path 到 `Weak<Dentry>` 的映射.
- `Inode::cacheable` 允许动态文件系统拒绝路径缓存.
- ext4 inode 使用 weak dentry 反向引用, 需要路径时从 dentry 计算 full path.

## 目标

- 让路径缓存不污染具体文件系统实现.
- 让一个 inode 可以被多个名字引用, 支持硬链接和 mount root 等关系.
- 让路径解析能跨 mount point 并保留合理的 `..` 行为.
- 让动态文件系统能避免陈旧 dentry.

## 非目标

- 不在这里列出 `Inode` 的每个方法和元数据字段.
- 不描述具体磁盘 inode 格式.
- 不承诺 dentry cache 是强一致缓存.

## 模块边界

- `inode.rs` 定义对象能力: 元数据, 读写, 目录操作, 链接, 设备节点, 时间戳等.
- `dentry.rs` 定义命名空间缓存: parent/children, mount point, full path, global cache.
- `path.rs` 是两者的主要协调者: miss 时调用 inode lookup, hit 时复用 dentry.
- 具体 FS 只应返回 inode, 不应自己维护 VFS 路径树.

## 关键流程

### lookup miss

```text
parent Dentry
  -> parent Inode lookup name
  -> child Inode
  -> Dentry new
  -> parent children cache
  -> optional global cache
```

缓存策略的关键点是 `Weak`. 全局缓存不会延长 dentry 生命周期, 因此长期不用的路径可以自然释放.

### full path

`Dentry::full_path` 沿 parent 链向上拼接路径. 如果当前 dentry 是某个挂载文件系统的根, 它会通过 mounted-on 关系回到外层挂载点, 使 `/mnt/file` 这类路径保持用户可见形式.

### mutation

创建, 删除, rename 等修改由父 inode 执行. VFS 的 dentry 子缓存需要随操作更新或失效. 当前实现依赖调用路径主动移除局部缓存, 不是完整 Linux dcache invalidation 模型.

## 并发和生命周期约束

- `Dentry` 持有 `Arc<dyn Inode>`, inode 生命周期至少覆盖该路径节点.
- parent 使用 `Weak<Dentry>`, children 使用 `Arc<Dentry>`, 避免父子强引用环.
- global cache 使用 `Weak<Dentry>`, 只作为加速索引.
- inode 到 dentry 的反向关系必须是可选或 weak, 否则容易形成泄漏.
- 动态 inode, 尤其 `/proc/[pid]`, 不应无条件缓存.

## 已知限制

- dentry cache 没有版本号或统一失效事件.
- hard link 的多个 dentry 共享 inode 语义依赖具体 FS 正确实现.
- symlink 的最终解析在 `path.rs`, inode 只负责返回 link target.
- 跨文件系统 rename 等复杂语义仍由上层约束.

## 源码索引

- `os/src/vfs/inode.rs`: `Inode`, `InodeMetadata`, `DirEntry`, `FileMode`.
- `os/src/vfs/dentry.rs`: `Dentry`, `DentryCache`, mount relation.
- `os/src/vfs/path.rs`: lookup miss/hit, symlink 跟随, mount crossing.
- `os/src/fs/ext4/inode.rs`: 持久化 inode 实现和 dentry weak 反向引用.
- `os/src/fs/tmpfs/inode.rs`: 内存 inode 和目录树.
- `os/src/fs/proc/inode.rs`: 动态 inode 和 cacheable 策略.
- `os/src/fs/sysfs/inode.rs`: 属性文件, 目录和 symlink inode.
- `os/src/fs/vfat/inode.rs`: FAT/VFAT inode 适配.
