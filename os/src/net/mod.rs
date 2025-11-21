//! 网络子系统模块
//! 
//! 此模块实现了操作系统的网络栈，包括网络接口抽象、协议栈实现
//! 以及网络系统调用支持。

use alloc::{sync::Arc, vec::Vec};
use spin::Mutex;

pub mod interface;
pub mod stack;
pub mod protocol;
pub mod device;

use crate::println;

use self::{interface::NetworkInterface};

/// 网络子系统全局状态
pub struct NetworkSubsystem {
    /// 已注册的网络接口列表
    interfaces: Vec<Arc<Mutex<NetworkInterface>>>,
    /// 默认网络接口
    default_interface: Option<Arc<Mutex<NetworkInterface>>>,
}

impl NetworkSubsystem {
    /// 创建一个新的网络子系统实例
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            default_interface: None,
        }
    }
    
    /// 注册一个网络接口
    pub fn register_interface(&mut self, interface: Arc<Mutex<NetworkInterface>>) {
        self.interfaces.push(interface.clone());
        
        // 如果还没有默认接口，将第一个接口设为默认
        if self.default_interface.is_none() {
            self.default_interface = Some(interface);
        }
    }
    
    /// 获取所有网络接口
    pub fn get_interfaces(&self) -> &[Arc<Mutex<NetworkInterface>>] {
        &self.interfaces
    }
    
    /// 获取默认网络接口
    pub fn get_default_interface(&self) -> Option<Arc<Mutex<NetworkInterface>>> {
        self.default_interface.clone()
    }
}

/// 网络子系统全局实例
static mut NETWORK_SUBSYSTEM: Option<Arc<Mutex<NetworkSubsystem>>> = None;

/// 初始化网络子系统
pub fn init() {
    println!("[Network] Initializing network subsystem...");
    
    // 创建网络子系统实例
    let subsystem = Arc::new(Mutex::new(NetworkSubsystem::new()));
    
    // 初始化协议栈
    protocol::init();
    
    // 初始化网络设备层
    device::init();
    
    // 存储全局实例
    unsafe {
        NETWORK_SUBSYSTEM = Some(subsystem);
    }
    
    println!("[Network] Network subsystem initialized");
}

/// 获取网络子系统实例
pub fn get_network_subsystem() -> Option<Arc<Mutex<NetworkSubsystem>>> {
    unsafe {
        match NETWORK_SUBSYSTEM {
            Some(ref subsystem) => Some(Arc::clone(subsystem)),
            None => None,
        }
    }
}

/// 运行网络测试
pub fn run_tests() {
    println!("[Network] Running network tests...");
    
    // 这里可以添加网络相关的测试代码
    println!("[Network] Network tests completed");
}