use core::ffi::{c_uint, c_ulong};

/// 系统信息结构体
/// 对应 Linux 的 `struct sysinfo`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SysInfo {
    /// 系统启动后经过的时间，单位为秒
    pub uptime: c_ulong,
    /// 1 分钟、5 分钟和 15 分钟的平均负载
    pub loads: [c_ulong; 3],
    /// 总内存大小，单位为字节
    pub totalram: c_ulong,
    /// 可用内存大小，单位为字节
    pub freeram: c_ulong,
    /// 缓存大小，单位为字节
    pub sharedram: c_ulong,
    /// 用作文件缓存的内存大小，单位为字节
    pub bufferram: c_ulong,
    /// 总交换空间大小，单位为字节
    pub totalswap: c_ulong,
    /// 可用交换空间大小，单位为字节
    pub freeswap: c_ulong,
    /// 当前进程数
    pub procs: u16,
    /// 高端内存总大小，单位为字节
    pub totalhigh: c_ulong,
    /// 高端可用内存大小，单位为字节
    pub freehigh: c_ulong,
    /// 内存单位大小，单位为字节
    pub mem_unit: c_uint,
    /// 保留字段，供未来使用
    pub _reserved: [u8; 256],
}

impl SysInfo {
    /// 创建一个新的 SysInfo 实例，所有字段初始化为零
    pub fn new() -> Self {
        Self {
            uptime: 0,
            loads: [0; 3],
            totalram: 0,
            freeram: 0,
            sharedram: 0,
            bufferram: 0,
            totalswap: 0,
            freeswap: 0,
            procs: 0,
            totalhigh: 0,
            freehigh: 0,
            mem_unit: 0,
            _reserved: [0; 256],
        }
    }
}
