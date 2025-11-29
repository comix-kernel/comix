use bitflags::bitflags;

bitflags! {
    /// Linux 能力位标志
    ///
    /// 这些标志代表了内核中各种特权操作的权限。
    /// 在单 root 用户系统中，所有进程默认拥有所有能力。
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Capabilities: u64 {
        /// 改变文件所有者
        const CHOWN              = 1 << 0;
        /// 绕过文件读写执行权限检查
        const DAC_OVERRIDE       = 1 << 1;
        /// 绕过文件读权限检查
        const DAC_READ_SEARCH    = 1 << 2;
        /// 绕过文件所有者检查
        const FOWNER             = 1 << 3;
        /// 不清除 setuid/setgid 位
        const FSETID             = 1 << 4;
        /// 发送信号给任意进程
        const KILL               = 1 << 5;
        /// 修改进程 GID
        const SETGID             = 1 << 6;
        /// 修改进程 UID
        const SETUID             = 1 << 7;
        /// 传递能力
        const SETPCAP            = 1 << 8;
        /// 设置不可变标志
        const LINUX_IMMUTABLE    = 1 << 9;
        /// 绑定特权端口 (<1024)
        const NET_BIND_SERVICE   = 1 << 10;
        /// 网络广播
        const NET_BROADCAST      = 1 << 11;
        /// 网络管理操作
        const NET_ADMIN          = 1 << 12;
        /// 使用 RAW 和 PACKET socket
        const NET_RAW            = 1 << 13;
        /// 锁定内存
        const IPC_LOCK           = 1 << 14;
        /// 绕过 IPC 所有权检查
        const IPC_OWNER          = 1 << 15;
        /// 加载/卸载内核模块
        const SYS_MODULE         = 1 << 16;
        /// 执行 I/O 端口操作
        const SYS_RAWIO          = 1 << 17;
        /// 使用 chroot()
        const SYS_CHROOT         = 1 << 18;
        /// 使用 ptrace()
        const SYS_PTRACE         = 1 << 19;
        /// 进程统计
        const SYS_PACCT          = 1 << 20;
        /// 系统管理操作
        const SYS_ADMIN          = 1 << 21;
        /// 重启系统
        const SYS_BOOT           = 1 << 22;
        /// 修改进程优先级
        const SYS_NICE           = 1 << 23;
        /// 覆盖资源限制
        const SYS_RESOURCE       = 1 << 24;
        /// 设置系统时间
        const SYS_TIME           = 1 << 25;
        /// 配置 TTY
        const SYS_TTY_CONFIG     = 1 << 26;
        /// 创建设备文件
        const MKNOD              = 1 << 27;
        /// 建立文件租约
        const LEASE              = 1 << 28;
        /// 写入审计日志
        const AUDIT_WRITE        = 1 << 29;
        /// 配置审计
        const AUDIT_CONTROL      = 1 << 30;
        /// 设置文件能力
        const SETFCAP            = 1 << 31;
        /// 覆盖 MAC 策略
        const MAC_OVERRIDE       = 1 << 32;
        /// 配置 MAC
        const MAC_ADMIN          = 1 << 33;
        /// 访问内核日志
        const SYSLOG             = 1 << 34;
        /// 触发唤醒告警
        const WAKE_ALARM         = 1 << 35;
        /// 阻止系统挂起
        const BLOCK_SUSPEND      = 1 << 36;
        /// 读取审计日志
        const AUDIT_READ         = 1 << 37;
        /// 性能监控
        const PERFMON            = 1 << 38;
        /// BPF 操作
        const BPF                = 1 << 39;
        /// 检查点/恢复
        const CHECKPOINT_RESTORE = 1 << 40;
    }
}

impl Capabilities {
    /// 拥有所有能力（root 用户）
    pub const fn full() -> Self {
        Self::all()
    }

    /// 空能力集
    pub const fn empty_set() -> Self {
        Self::empty()
    }
}

/// 能力集合
///
/// Linux 为每个进程维护 5 个能力集：
/// - effective: 当前有效的能力（用于权限检查）
/// - permitted: 允许的能力上限
/// - inheritable: 可以继承给子进程的能力
/// - bounding: 能力边界集（限制可获得的能力）
/// - ambient: 环境能力（保持跨 execve）
#[derive(Clone, Copy, Debug)]
pub struct CapabilitySet {
    /// 有效能力集（当前生效的能力）
    pub effective: Capabilities,
    /// 允许能力集（进程可以使用的能力）
    pub permitted: Capabilities,
    /// 可继承能力集（execve 时可以继承的能力）
    pub inheritable: Capabilities,
    /// 边界能力集（能力的上限）
    pub bounding: Capabilities,
    /// 环境能力集（保持跨 execve 的能力）
    pub ambient: Capabilities,
}

impl CapabilitySet {
    /// 创建拥有所有能力的集合（root 用户）
    pub const fn full() -> Self {
        Self {
            effective: Capabilities::full(),
            permitted: Capabilities::full(),
            inheritable: Capabilities::full(),
            bounding: Capabilities::full(),
            ambient: Capabilities::full(),
        }
    }

    /// 创建空能力集
    pub const fn empty() -> Self {
        Self {
            effective: Capabilities::empty_set(),
            permitted: Capabilities::empty_set(),
            inheritable: Capabilities::empty_set(),
            bounding: Capabilities::empty_set(),
            ambient: Capabilities::empty_set(),
        }
    }

    /// 检查是否拥有某个能力
    pub fn has(&self, cap: Capabilities) -> bool {
        // 在单 root 用户系统中，总是返回 true
        self.effective.contains(cap)
    }

    /// 检查是否拥有所有指定的能力
    pub fn has_all(&self, caps: Capabilities) -> bool {
        self.effective.contains(caps)
    }

    /// 添加能力（在单 root 用户系统中无实际效果）
    pub fn add(&mut self, cap: Capabilities) {
        self.effective.insert(cap);
        self.permitted.insert(cap);
    }

    /// 移除能力（在单 root 用户系统中无实际效果）
    pub fn remove(&mut self, cap: Capabilities) {
        self.effective.remove(cap);
    }
}

// 为了与 Linux 系统调用兼容，提供能力位的数值常量
pub const CAP_CHOWN: u32 = 0;
pub const CAP_DAC_OVERRIDE: u32 = 1;
pub const CAP_DAC_READ_SEARCH: u32 = 2;
pub const CAP_FOWNER: u32 = 3;
pub const CAP_FSETID: u32 = 4;
pub const CAP_KILL: u32 = 5;
pub const CAP_SETGID: u32 = 6;
pub const CAP_SETUID: u32 = 7;
pub const CAP_SETPCAP: u32 = 8;
pub const CAP_LINUX_IMMUTABLE: u32 = 9;
pub const CAP_NET_BIND_SERVICE: u32 = 10;
pub const CAP_NET_BROADCAST: u32 = 11;
pub const CAP_NET_ADMIN: u32 = 12;
pub const CAP_NET_RAW: u32 = 13;
pub const CAP_IPC_LOCK: u32 = 14;
pub const CAP_IPC_OWNER: u32 = 15;
pub const CAP_SYS_MODULE: u32 = 16;
pub const CAP_SYS_RAWIO: u32 = 17;
pub const CAP_SYS_CHROOT: u32 = 18;
pub const CAP_SYS_PTRACE: u32 = 19;
pub const CAP_SYS_PACCT: u32 = 20;
pub const CAP_SYS_ADMIN: u32 = 21;
pub const CAP_SYS_BOOT: u32 = 22;
pub const CAP_SYS_NICE: u32 = 23;
pub const CAP_SYS_RESOURCE: u32 = 24;
pub const CAP_SYS_TIME: u32 = 25;
pub const CAP_SYS_TTY_CONFIG: u32 = 26;
pub const CAP_MKNOD: u32 = 27;
pub const CAP_LEASE: u32 = 28;
pub const CAP_AUDIT_WRITE: u32 = 29;
pub const CAP_AUDIT_CONTROL: u32 = 30;
pub const CAP_SETFCAP: u32 = 31;
pub const CAP_MAC_OVERRIDE: u32 = 32;
pub const CAP_MAC_ADMIN: u32 = 33;
pub const CAP_SYSLOG: u32 = 34;
pub const CAP_WAKE_ALARM: u32 = 35;
pub const CAP_BLOCK_SUSPEND: u32 = 36;
pub const CAP_AUDIT_READ: u32 = 37;
pub const CAP_PERFMON: u32 = 38;
pub const CAP_BPF: u32 = 39;
pub const CAP_CHECKPOINT_RESTORE: u32 = 40;
pub const CAP_LAST_CAP: u32 = CAP_CHECKPOINT_RESTORE;

/// 从能力索引转换为能力位
pub fn capability_from_u32(cap_index: u32) -> Option<Capabilities> {
    if cap_index > CAP_LAST_CAP {
        return None;
    }
    Some(Capabilities::from_bits_truncate(1u64 << cap_index))
}
