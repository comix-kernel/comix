//! 网络设备驱动接口
//! 
//! 此模块定义了网络设备的抽象接口，支持不同类型的网络设备驱动。

use alloc::{string::ToString, sync::Arc};
use spin::Mutex;

use crate::println;

/// 网络设备驱动接口
pub trait NetDevice: Send + Sync {
    /// 发送数据包
    fn send(&mut self, data: &[u8]) -> Result<(), ()>;
    
    /// 接收数据包
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, ()>;
    
    /// 获取设备名称
    fn get_name(&self) -> &str;
    
    /// 获取 MAC 地址
    fn get_mac_address(&self) -> [u8; 6];
    
    /// 获取最大传输单元 (MTU)
    fn get_mtu(&self) -> usize;
    
    /// 检查设备是否已初始化
    fn is_initialized(&self) -> bool;
}

/// 模拟网络设备实现
/// 用于测试环境
pub struct MockNetDevice {
    name: alloc::string::String,
    mac_address: [u8; 6],
    mtu: usize,
    initialized: bool,
}

impl MockNetDevice {
    /// 创建一个新的模拟网络设备
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            mac_address: [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
            mtu: 1500,
            initialized: false,
        }
    }
    
    /// 初始化设备
    pub fn init(&mut self) {
        self.initialized = true;
    }
}

impl NetDevice for MockNetDevice {
    fn send(&mut self, _data: &[u8]) -> Result<(), ()> {
        if !self.initialized {
            return Err(());
        }
        // 在模拟设备中，我们只是简单地返回成功
        Ok(())
    }
    
    fn receive(&mut self, _buffer: &mut [u8]) -> Result<usize, ()> {
        if !self.initialized {
            return Err(());
        }
        // 模拟设备暂时不接收数据
        Ok(0)
    }
    
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_mac_address(&self) -> [u8; 6] {
        self.mac_address
    }
    
    fn get_mtu(&self) -> usize {
        self.mtu
    }
    
    fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// 网络设备管理器
pub struct NetDeviceManager {
    devices: alloc::vec::Vec<Arc<Mutex<dyn NetDevice>>>,
}

impl NetDeviceManager {
    /// 创建一个新的设备管理器
    pub fn new() -> Self {
        Self {
            devices: alloc::vec::Vec::new(),
        }
    }
    
    /// 注册一个网络设备
    pub fn register_device(&mut self, device: Arc<Mutex<dyn NetDevice>>) {
        self.devices.push(device);
    }
    
    /// 根据名称查找设备
    pub fn find_device(&self, name: &str) -> Option<Arc<Mutex<dyn NetDevice>>> {
        for device in &self.devices {
            if device.lock().get_name() == name {
                return Some(device.clone());
            }
        }
        None
    }
    
    /// 获取所有设备
    pub fn get_all_devices(&self) -> &[Arc<Mutex<dyn NetDevice>>] {
        &self.devices
    }
}

/// 全局网络设备管理器
static mut DEVICE_MANAGER: Option<Arc<Mutex<NetDeviceManager>>> = None;

/// 初始化网络设备子系统
pub fn init() {
    println!("[Network] Initializing network device subsystem...");
    
    // 创建设备管理器
    let manager = Arc::new(Mutex::new(NetDeviceManager::new()));
    
    // 创建并注册模拟网络设备
    let mock_device = Arc::new(Mutex::new(MockNetDevice::new("eth0")));
    mock_device.lock().init();
    manager.lock().register_device(mock_device);
    
    // 存储全局实例
    unsafe {
        DEVICE_MANAGER = Some(manager);
    }
    
    println!("[Network] Network device subsystem initialized");
}

/// 获取网络设备管理器
pub fn get_device_manager() -> Option<Arc<Mutex<NetDeviceManager>>> {
    unsafe {
        match DEVICE_MANAGER {
            Some(ref manager) => Some(Arc::clone(manager)),
            None => None,
        }
    }
}