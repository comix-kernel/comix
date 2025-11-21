//! 网络协议实现
//! 
//! 此模块实现了基本的网络协议，包括ARP和ICMP。

use alloc::{vec::Vec, string::String};

use crate::println;

/// 以太网帧头部
#[derive(Debug, Clone)]
pub struct EthernetHeader {
    pub dest_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub ether_type: [u8; 2],
}

impl EthernetHeader {
    /// 创建一个新的以太网帧头部
    pub fn new(dest_mac: [u8; 6], src_mac: [u8; 6], ether_type: [u8; 2]) -> Self {
        Self {
            dest_mac,
            src_mac,
            ether_type,
        }
    }
    
    /// 将以太网帧头部转换为字节数组
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(14);
        bytes.extend_from_slice(&self.dest_mac);
        bytes.extend_from_slice(&self.src_mac);
        bytes.extend_from_slice(&self.ether_type);
        bytes
    }
}

/// ARP协议相关实现
pub mod arp {
    use super::*;
    
    /// ARP操作类型
    pub enum ArpOperation {
        Request,
        Reply,
    }
    
    /// ARP数据包
    #[derive(Debug, Clone)]
    pub struct ArpPacket {
        pub hardware_type: [u8; 2],
        pub protocol_type: [u8; 2],
        pub hardware_len: u8,
        pub protocol_len: u8,
        pub operation: [u8; 2],
        pub sender_mac: [u8; 6],
        pub sender_ip: [u8; 4],
        pub target_mac: [u8; 6],
        pub target_ip: [u8; 4],
    }
    
    impl ArpPacket {
        /// 创建一个新的ARP请求数据包
        pub fn new_request(sender_mac: [u8; 6], sender_ip: [u8; 4], target_ip: [u8; 4]) -> Self {
            Self {
                hardware_type: [0x00, 0x01], // Ethernet
                protocol_type: [0x08, 0x00], // IPv4
                hardware_len: 6,
                protocol_len: 4,
                operation: [0x00, 0x01], // Request
                sender_mac,
                sender_ip,
                target_mac: [0xff; 6], // 广播MAC
                target_ip,
            }
        }
        
        /// 创建一个新的ARP响应数据包
        pub fn new_reply(sender_mac: [u8; 6], sender_ip: [u8; 4], target_mac: [u8; 6], target_ip: [u8; 4]) -> Self {
            Self {
                hardware_type: [0x00, 0x01], // Ethernet
                protocol_type: [0x08, 0x00], // IPv4
                hardware_len: 6,
                protocol_len: 4,
                operation: [0x00, 0x02], // Reply
                sender_mac,
                sender_ip,
                target_mac,
                target_ip,
            }
        }
        
        /// 将ARP数据包转换为字节数组
        pub fn to_bytes(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(28);
            bytes.extend_from_slice(&self.hardware_type);
            bytes.extend_from_slice(&self.protocol_type);
            bytes.push(self.hardware_len);
            bytes.push(self.protocol_len);
            bytes.extend_from_slice(&self.operation);
            bytes.extend_from_slice(&self.sender_mac);
            bytes.extend_from_slice(&self.sender_ip);
            bytes.extend_from_slice(&self.target_mac);
            bytes.extend_from_slice(&self.target_ip);
            bytes
        }
    }
    
    /// ARP缓存
    pub struct ArpCache {
        entries: Vec<(u32, [u8; 6])>, // (IP地址, MAC地址)
    }
    
    impl ArpCache {
        /// 创建一个新的ARP缓存
        pub fn new() -> Self {
            Self {
                entries: Vec::new(),
            }
        }
        
        /// 添加或更新ARP表项
        pub fn add_entry(&mut self, ip: [u8; 4], mac: [u8; 6]) {
            let ip_u32 = u32::from_be_bytes(ip);
            
            // 查找是否已存在该IP的表项
            if let Some(entry) = self.entries.iter_mut().find(|(ip_entry, _)| *ip_entry == ip_u32) {
                entry.1 = mac;
            } else {
                self.entries.push((ip_u32, mac));
            }
        }
        
        /// 根据IP地址查找MAC地址
        pub fn find_mac(&self, ip: [u8; 4]) -> Option<[u8; 6]> {
            let ip_u32 = u32::from_be_bytes(ip);
            
            for (ip_entry, mac_entry) in &self.entries {
                if *ip_entry == ip_u32 {
                    return Some(*mac_entry);
                }
            }
            
            None
        }
    }
}

/// ICMP协议相关实现
pub mod icmp {
    use super::*;
    
    /// ICMP类型
    pub enum IcmpType {
        EchoReply = 0,
        EchoRequest = 8,
    }
    
    /// ICMP数据包
    #[derive(Debug, Clone)]
    pub struct IcmpPacket {
        pub icmp_type: u8,
        pub code: u8,
        pub checksum: [u8; 2],
        pub identifier: [u8; 2],
        pub sequence: [u8; 2],
        pub data: Vec<u8>,
    }
    
    impl IcmpPacket {
        /// 创建一个新的ICMP回显请求数据包
        pub fn new_echo_request(identifier: u16, sequence: u16, data: &[u8]) -> Self {
            let mut packet = Self {
                icmp_type: IcmpType::EchoRequest as u8,
                code: 0,
                checksum: [0; 2],
                identifier: identifier.to_be_bytes(),
                sequence: sequence.to_be_bytes(),
                data: data.to_vec(),
            };
            
            // 计算校验和
            packet.checksum = packet.calculate_checksum();
            
            packet
        }
        
        /// 创建一个新的ICMP回显响应数据包
        pub fn new_echo_reply(request: &Self) -> Self {
            let mut packet = Self {
                icmp_type: IcmpType::EchoReply as u8,
                code: 0,
                checksum: [0; 2],
                identifier: request.identifier,
                sequence: request.sequence,
                data: request.data.clone(),
            };
            
            // 计算校验和
            packet.checksum = packet.calculate_checksum();
            
            packet
        }
        
        /// 计算ICMP数据包的校验和
        fn calculate_checksum(&self) -> [u8; 2] {
            let mut sum = 0u32;
            
            // 添加类型和代码
            sum += self.icmp_type as u32;
            sum += self.code as u32;
            
            // 添加标识符和序列号
            sum += u16::from_be_bytes(self.identifier) as u32;
            sum += u16::from_be_bytes(self.sequence) as u32;
            
            // 添加数据部分
            let mut data_iter = self.data.chunks(2);
            for chunk in &mut data_iter {
                if chunk.len() == 2 {
                    sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
                } else if chunk.len() == 1 {
                    sum += chunk[0] as u32;
                }
            }
            
            // 计算校验和
            while sum >> 16 != 0 {
                sum = (sum & 0xFFFF) + (sum >> 16);
            }
            
            (!sum as u16).to_be_bytes()
        }
        
        /// 将ICMP数据包转换为字节数组
        pub fn to_bytes(&self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(8 + self.data.len());
            bytes.push(self.icmp_type);
            bytes.push(self.code);
            bytes.extend_from_slice(&self.checksum);
            bytes.extend_from_slice(&self.identifier);
            bytes.extend_from_slice(&self.sequence);
            bytes.extend_from_slice(&self.data);
            bytes
        }
    }
}

/// 协议栈初始化
pub fn init() {
    println!("[Network] Initializing network protocols...");
    // 协议栈初始化代码
    println!("[Network] Network protocols initialized");
}