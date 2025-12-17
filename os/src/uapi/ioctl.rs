//! ioctl 请求码定义
//!
//! 该模块定义了各种设备类型的 ioctl 请求码，遵循 Linux 标准。
//!
//! # ioctl 编码规则
//!
//! Linux ioctl 请求码通常使用 _IO, _IOR, _IOW, _IOWR 宏构造：
//! - `_IO(type, nr)`: 无参数
//! - `_IOR(type, nr, size)`: 读取数据
//! - `_IOW(type, nr, size)`: 写入数据
//! - `_IOWR(type, nr, size)`: 读写数据
//!
//! 参考：include/uapi/asm-generic/ioctl.h

/// ioctl 方向：无数据传输
pub const IOC_NONE: u32 = 0;
/// ioctl 方向：写入（用户空间 -> 内核）
pub const IOC_WRITE: u32 = 1;
/// ioctl 方向：读取（内核 -> 用户空间）
pub const IOC_READ: u32 = 2;

/// ioctl 编码掩码
pub const IOC_NRBITS: u32 = 8;
pub const IOC_TYPEBITS: u32 = 8;
pub const IOC_SIZEBITS: u32 = 14;
pub const IOC_DIRBITS: u32 = 2;

pub const IOC_NRSHIFT: u32 = 0;
pub const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
pub const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
pub const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;

/// 构造 ioctl 请求码（无参数）
pub const fn _IO(type_: u32, nr: u32) -> u32 {
    _IOC(IOC_NONE, type_, nr, 0)
}

/// 构造 ioctl 请求码（读取数据）
pub const fn _IOR(type_: u32, nr: u32, size: u32) -> u32 {
    _IOC(IOC_READ, type_, nr, size)
}

/// 构造 ioctl 请求码（写入数据）
pub const fn _IOW(type_: u32, nr: u32, size: u32) -> u32 {
    _IOC(IOC_WRITE, type_, nr, size)
}

/// 构造 ioctl 请求码（读写数据）
pub const fn _IOWR(type_: u32, nr: u32, size: u32) -> u32 {
    _IOC(IOC_READ | IOC_WRITE, type_, nr, size)
}

/// ioctl 请求码构造函数
pub const fn _IOC(dir: u32, type_: u32, nr: u32, size: u32) -> u32 {
    (dir << IOC_DIRSHIFT) | (type_ << IOC_TYPESHIFT) | (nr << IOC_NRSHIFT) | (size << IOC_SIZESHIFT)
}

/// 从请求码中提取方向
pub const fn _IOC_DIR(nr: u32) -> u32 {
    (nr >> IOC_DIRSHIFT) & ((1 << IOC_DIRBITS) - 1)
}

/// 从请求码中提取类型
pub const fn _IOC_TYPE(nr: u32) -> u32 {
    (nr >> IOC_TYPESHIFT) & ((1 << IOC_TYPEBITS) - 1)
}

/// 从请求码中提取编号
pub const fn _IOC_NR(nr: u32) -> u32 {
    (nr >> IOC_NRSHIFT) & ((1 << IOC_NRBITS) - 1)
}

/// 从请求码中提取数据大小
pub const fn _IOC_SIZE(nr: u32) -> u32 {
    (nr >> IOC_SIZESHIFT) & ((1 << IOC_SIZEBITS) - 1)
}

// ========== 文件/通用 I/O 操作 ==========

/// 设置/清除非阻塞 I/O 标志（int）
pub const FIONBIO: u32 = 0x5421;

/// 获取可读字节数（int）
pub const FIONREAD: u32 = 0x541B;

/// 设置/清除异步 I/O 通知（int）
pub const FIOASYNC: u32 = 0x5452;

/// 获取文件系统块大小（long）
pub const FIGETBSZ: u32 = 2;

// ========== 终端 ioctl（TTY/PTY）==========

/// 终端 ioctl 魔数
pub const TCGETS: u32 = 0x5401;
pub const TCSETS: u32 = 0x5402;
pub const TCSETSW: u32 = 0x5403;
pub const TCSETSF: u32 = 0x5404;

/// 获取终端窗口大小（struct winsize）
pub const TIOCGWINSZ: u32 = 0x5413;

/// 设置终端窗口大小（struct winsize）
pub const TIOCSWINSZ: u32 = 0x5414;

/// 获取终端进程组 ID（pid_t）
pub const TIOCGPGRP: u32 = 0x540F;

/// 设置终端进程组 ID（pid_t）
pub const TIOCSPGRP: u32 = 0x5410;

/// 获取输出队列中的字节数（int）
pub const TIOCOUTQ: u32 = 0x5411;

/// 获取输入队列中的字节数（int）
pub const TIOCINQ: u32 = FIONREAD;

/// 独占使用终端（void）
pub const TIOCEXCL: u32 = 0x540C;

/// 取消独占使用（void）
pub const TIOCNXCL: u32 = 0x540D;

/// 设置控制终端（void）- busybox init 需要
pub const TIOCSCTTY: u32 = 0x540E;

/// 查询可用的虚拟终端（int *）- 可选，用于 VT 切换
pub const VT_OPENQRY: u32 = 0x5600;

// ========== 网络 Socket ioctl ==========

/// Socket ioctl 魔数
pub const SIOCGIFNAME: u32 = 0x8910;

/// 获取接口列表（struct ifconf）
pub const SIOCGIFCONF: u32 = 0x8912;

/// 获取接口地址（struct ifreq）
pub const SIOCGIFADDR: u32 = 0x8915;

/// 设置接口地址（struct ifreq）
pub const SIOCSIFADDR: u32 = 0x8916;

/// 获取接口标志（struct ifreq）
pub const SIOCGIFFLAGS: u32 = 0x8913;

/// 设置接口标志（struct ifreq）
pub const SIOCSIFFLAGS: u32 = 0x8914;

/// 获取接口广播地址（struct ifreq）
pub const SIOCGIFBRDADDR: u32 = 0x8919;

/// 设置接口广播地址（struct ifreq）
pub const SIOCSIFBRDADDR: u32 = 0x891A;

/// 获取接口网络掩码（struct ifreq）
pub const SIOCGIFNETMASK: u32 = 0x891B;

/// 设置接口网络掩码（struct ifreq）
pub const SIOCSIFNETMASK: u32 = 0x891C;

/// 获取接口 MTU（struct ifreq）
pub const SIOCGIFMTU: u32 = 0x8921;

/// 设置接口 MTU（struct ifreq）
pub const SIOCSIFMTU: u32 = 0x8922;

/// 获取接口硬件地址/MAC（struct ifreq）
pub const SIOCGIFHWADDR: u32 = 0x8927;

/// 设置接口硬件地址/MAC（struct ifreq）
pub const SIOCSIFHWADDR: u32 = 0x8924;

/// 获取接口索引（struct ifreq）
pub const SIOCGIFINDEX: u32 = 0x8933;

/// 根据索引获取接口名称（struct ifreq）
pub const SIOCGIFNAME_BY_INDEX: u32 = 0x8910;

// ========== 设备特定 ioctl ==========

/// RTC（实时时钟）设备
/// size = sizeof(struct rtc_time) = 9 * sizeof(int) = 36
pub const RTC_RD_TIME: u32 = _IOR(b'p' as u32, 0x09, 36);
pub const RTC_SET_TIME: u32 = _IOW(b'p' as u32, 0x0A, 36);

/// RTC 时间结构体（对应 Linux struct rtc_time）
///
/// 参考：include/uapi/linux/rtc.h
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RtcTime {
    /// 秒 (0-59)
    pub tm_sec: i32,
    /// 分 (0-59)
    pub tm_min: i32,
    /// 时 (0-23)
    pub tm_hour: i32,
    /// 日 (1-31)
    pub tm_mday: i32,
    /// 月 (0-11, 注意是 0-based)
    pub tm_mon: i32,
    /// 年份 - 1900
    pub tm_year: i32,
    /// 星期 (0-6, 0=Sunday)
    pub tm_wday: i32,
    /// 年内第几天 (0-365)
    pub tm_yday: i32,
    /// 夏令时标志
    pub tm_isdst: i32,
}

/// 块设备
pub const BLKGETSIZE: u32 = _IO(0x12, 96);
pub const BLKGETSIZE64: u32 = _IOR(0x12, 114, 8);
pub const BLKFLSBUF: u32 = _IO(0x12, 97);

// ========== 终端窗口大小结构体 ==========

/// 终端窗口大小（用于 TIOCGWINSZ/TIOCSWINSZ）
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct WinSize {
    /// 窗口行数（字符）
    pub ws_row: u16,
    /// 窗口列数（字符）
    pub ws_col: u16,
    /// 窗口宽度（像素，通常未使用）
    pub ws_xpixel: u16,
    /// 窗口高度（像素，通常未使用）
    pub ws_ypixel: u16,
}

// ========== 终端属性结构体 (termios) ==========

/// 特殊控制字符数量（Linux asm-generic 标准）
pub const NCCS: usize = 19;

/// 终端属性结构（用于 TCGETS/TCSETS）
///
/// 这是 Linux asm-generic/termbits.h 中定义的标准 termios 结构。
/// RISC-V 架构使用 asm-generic 定义，NCCS=19。
///
/// 内存布局：
/// - offset 0-15:  c_iflag, c_oflag, c_cflag, c_lflag (4 * u32 = 16 bytes)
/// - offset 16:    c_line (u8 = 1 byte)
/// - offset 17-35: c_cc\[19\] (19 * u8 = 19 bytes)
/// - offset 36-39: c_ispeed (u32 = 4 bytes, 对齐到4字节边界)
/// - offset 40-43: c_ospeed (u32 = 4 bytes)
/// 总大小：44字节
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Termios {
    /// 输入模式标志
    pub c_iflag: u32,
    /// 输出模式标志
    pub c_oflag: u32,
    /// 控制模式标志
    pub c_cflag: u32,
    /// 本地模式标志
    pub c_lflag: u32,
    /// 行规程
    pub c_line: u8,
    /// 特殊控制字符 [NCCS=19]
    pub c_cc: [u8; NCCS],
    /// 输入波特率（注意：musl 在此之前有3字节padding使其对齐到4字节边界）
    pub c_ispeed: u32,
    /// 输出波特率
    pub c_ospeed: u32,
}

impl Termios {
    /// 默认终端配置常量
    ///
    /// 提供标准的终端默认设置，适用于交互式 shell 和一般终端应用。
    pub const DEFAULT: Self = Self {
        // 输入模式：ICRNL (将 CR 转换为 NL)
        c_iflag: 0x0100,
        // 输出模式：OPOST | ONLCR (启用输出处理，将 NL 转换为 CR-NL)
        c_oflag: 0x0001 | 0x0004,
        // 控制模式：CS8 | CREAD (8位字符，允许接收)
        c_cflag: 0x0030 | 0x0080,
        // 本地模式：ISIG | ICANON | ECHO | ECHOE
        c_lflag: 0x0001 | 0x0002 | 0x0008 | 0x0010,
        // 行规程：0 (N_TTY)
        c_line: 0,
        // 特殊控制字符（使用常见默认值）
        // 索引: 0=VINTR, 1=VQUIT, 2=VERASE, 3=VKILL, 4=VEOF, 5=VTIME, 6=VMIN,
        //       7=VSWTC, 8=VSTART, 9=VSTOP, 10=VSUSP, 11=VEOL, 12=VREPRINT,
        //       13=VDISCARD, 14=VWERASE, 15=VLNEXT, 16=VEOL2, 17-18=保留
        c_cc: [
            3,   // 0: VINTR (Ctrl-C)
            28,  // 1: VQUIT (Ctrl-\)
            127, // 2: VERASE (DEL)
            21,  // 3: VKILL (Ctrl-U)
            4,   // 4: VEOF (Ctrl-D)
            0,   // 5: VTIME
            1,   // 6: VMIN
            0,   // 7: VSWTC
            17,  // 8: VSTART (Ctrl-Q)
            19,  // 9: VSTOP (Ctrl-S)
            26,  // 10: VSUSP (Ctrl-Z)
            0,   // 11: VEOL
            18,  // 12: VREPRINT (Ctrl-R)
            15,  // 13: VDISCARD (Ctrl-O)
            23,  // 14: VWERASE (Ctrl-W)
            22,  // 15: VLNEXT (Ctrl-V)
            0,   // 16: VEOL2
            0,   // 17: 保留
            0,   // 18: 保留
        ],
        // 波特率：38400 (B38400 = 0x0000000f)
        c_ispeed: 0x0000000f,
        c_ospeed: 0x0000000f,
    };
}

impl Default for Termios {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// ========== 网络接口结构体 ==========

/// 最大接口名称长度
pub const IFNAMSIZ: usize = 16;

/// 接口请求结构（用于 SIOC* 操作）
#[repr(C)]
#[derive(Clone, Copy)]
pub union IfreqIfru {
    pub ifru_addr: [u8; 16],      // sockaddr
    pub ifru_dstaddr: [u8; 16],   // sockaddr
    pub ifru_broadaddr: [u8; 16], // sockaddr
    pub ifru_netmask: [u8; 16],   // sockaddr
    pub ifru_hwaddr: [u8; 16],    // sockaddr
    pub ifru_flags: i16,
    pub ifru_ivalue: i32,
    pub ifru_mtu: i32,
    pub ifru_map: [u8; 16],
    pub ifru_slave: [u8; IFNAMSIZ],
    pub ifru_newname: [u8; IFNAMSIZ],
    pub ifru_data: usize, // void*
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ifreq {
    pub ifr_name: [u8; IFNAMSIZ],
    pub ifr_ifru: IfreqIfru,
}

/// 接口配置结构（用于 SIOCGIFCONF）
#[repr(C)]
pub struct Ifconf {
    pub ifc_len: i32,
    pub ifc_buf: usize, // void* 或 struct ifreq*
}
