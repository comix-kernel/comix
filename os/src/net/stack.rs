//! 网络栈实现
//!
//! 此模块实现了基本的网络协议栈，包括链路层、网络层和传输层。

use alloc::{sync::Arc, vec::Vec};
use spin::Mutex;

use crate::net::{
    interface::NetworkInterface,
    protocol::{EthernetHeader, arp, icmp},
};

/// 网络栈
pub struct NetworkStack {
    /// ARP缓存
    arp_cache: arp::ArpCache,
}

impl NetworkStack {
    /// 创建一个新的网络栈
    pub fn new() -> Self {
        Self {
            arp_cache: arp::ArpCache::new(),
        }
    }

    /// 处理接收到的数据包
    pub fn process_packet(&mut self, interface: &Arc<Mutex<NetworkInterface>>, data: &[u8]) {
        // 检查数据包长度是否足够包含以太网头部
        if data.len() < 14 {
            return;
        }

        // 解析以太网头部
        let eth_header = EthernetHeader {
            dest_mac: [data[0], data[1], data[2], data[3], data[4], data[5]],
            src_mac: [data[6], data[7], data[8], data[9], data[10], data[11]],
            ether_type: [data[12], data[13]],
        };

        // 获取有效载荷
        let payload = &data[14..];

        // 根据以太网类型处理不同协议
        match eth_header.ether_type {
            [0x08, 0x06] => self.process_arp_packet(interface, &eth_header, payload),
            [0x08, 0x00] => self.process_ipv4_packet(interface, &eth_header, payload),
            _ => {}
        }
    }

    /// 处理ARP数据包
    fn process_arp_packet(
        &mut self,
        interface: &Arc<Mutex<NetworkInterface>>,
        eth_header: &EthernetHeader,
        payload: &[u8],
    ) {
        if payload.len() < 28 {
            return;
        }

        // 解析ARP数据包
        let arp_packet = arp::ArpPacket {
            hardware_type: [payload[0], payload[1]],
            protocol_type: [payload[2], payload[3]],
            hardware_len: payload[4],
            protocol_len: payload[5],
            operation: [payload[6], payload[7]],
            sender_mac: [
                payload[8],
                payload[9],
                payload[10],
                payload[11],
                payload[12],
                payload[13],
            ],
            sender_ip: [payload[14], payload[15], payload[16], payload[17]],
            target_mac: [
                payload[18],
                payload[19],
                payload[20],
                payload[21],
                payload[22],
                payload[23],
            ],
            target_ip: [payload[24], payload[25], payload[26], payload[27]],
        };

        // 更新ARP缓存
        self.arp_cache
            .add_entry(arp_packet.sender_ip, arp_packet.sender_mac);

        // 处理ARP请求
        if arp_packet.operation == [0x00, 0x01] {
            let interface_lock = interface.lock();

            // 检查是否是发给我们的ARP请求
            if arp_packet.target_ip == interface_lock.config.ip_address {
                // 构建ARP响应
                let reply = arp::ArpPacket::new_reply(
                    interface_lock.config.mac_address,
                    interface_lock.config.ip_address,
                    arp_packet.sender_mac,
                    arp_packet.sender_ip,
                );

                // 构建以太网帧
                let mut frame = eth_header.to_bytes();
                frame.extend_from_slice(&reply.to_bytes());

                // 发送响应
                drop(interface_lock);
                interface.lock().send_packet(&frame);
            }
        }
    }

    /// 处理IPv4数据包
    fn process_ipv4_packet(
        &mut self,
        interface: &Arc<Mutex<NetworkInterface>>,
        eth_header: &EthernetHeader,
        payload: &[u8],
    ) {
        if payload.len() < 20 {
            return;
        }

        // 检查版本和头部长度
        let version = (payload[0] >> 4) & 0x0F;
        if version != 4 {
            return;
        }

        let header_len = (payload[0] & 0x0F) as usize * 4;
        if header_len < 20 || payload.len() < header_len {
            return;
        }

        // 获取协议类型
        let protocol = payload[9];

        // 根据协议类型处理
        match protocol {
            1 => self.process_icmp_packet(interface, eth_header, &payload[header_len..]),
            _ => {}
        }
    }

    /// 处理ICMP数据包
    fn process_icmp_packet(
        &mut self,
        interface: &Arc<Mutex<NetworkInterface>>,
        eth_header: &EthernetHeader,
        payload: &[u8],
    ) {
        if payload.len() < 8 {
            return;
        }

        // 解析ICMP数据包
        let icmp_packet = icmp::IcmpPacket {
            icmp_type: payload[0],
            code: payload[1],
            checksum: [payload[2], payload[3]],
            identifier: [payload[4], payload[5]],
            sequence: [payload[6], payload[7]],
            data: payload[8..].to_vec(),
        };

        // 处理ICMP回显请求
        if icmp_packet.icmp_type == icmp::IcmpType::EchoRequest as u8 {
            // 构建ICMP回显响应
            let reply = icmp::IcmpPacket::new_echo_reply(&icmp_packet);

            // 构建以太网帧（交换源和目标MAC）
            let reply_eth_header = EthernetHeader::new(
                eth_header.src_mac,
                interface.lock().config.mac_address,
                eth_header.ether_type,
            );

            // 这里需要添加IPv4头部的构建
            // 为简化实现，暂时省略

            // 发送响应
            // interface.lock().send_packet(&frame);
        }
    }

    /// 发送ARP请求
    pub fn send_arp_request(
        &mut self,
        interface: &Arc<Mutex<NetworkInterface>>,
        target_ip: [u8; 4],
    ) {
        let interface_lock = interface.lock();

        // 构建ARP请求
        let arp_request = arp::ArpPacket::new_request(
            interface_lock.config.mac_address,
            interface_lock.config.ip_address,
            target_ip,
        );

        // 构建以太网帧（广播）
        let eth_header = EthernetHeader::new(
            [0xff; 6], // 广播MAC
            interface_lock.config.mac_address,
            [0x08, 0x06], // ARP
        );

        let mut frame = eth_header.to_bytes();
        frame.extend_from_slice(&arp_request.to_bytes());

        // 发送请求
        drop(interface_lock);
        interface.lock().send_packet(&frame);
    }

    /// 发送ICMP回显请求（ping）
    pub fn send_icmp_echo_request(
        &mut self,
        interface: &Arc<Mutex<NetworkInterface>>,
        target_ip: [u8; 4],
        identifier: u16,
        sequence: u16,
    ) {
        // 检查ARP缓存
        if let Some(target_mac) = self.arp_cache.find_mac(target_ip) {
            let interface_lock = interface.lock();

            // 构建ICMP请求
            let icmp_request =
                icmp::IcmpPacket::new_echo_request(identifier, sequence, b"Hello, World!");

            // 构建以太网帧
            let eth_header = EthernetHeader::new(
                target_mac,
                interface_lock.config.mac_address,
                [0x08, 0x00], // IPv4
            );

            // 这里需要添加IPv4头部的构建
            // 为简化实现，暂时省略

            // 发送请求
            // interface_lock.send_packet(&frame);
        } else {
            // 如果ARP缓存中没有，先发送ARP请求
            self.send_arp_request(interface, target_ip);
        }
    }
}
