//! 16550 UART 串行端口驱动程序模块

use alloc::{format, sync::Arc};
use fdt::node::FdtNode;
use uart_16550::MmioSerialPort;

use crate::{
    device::{
        DRIVERS, DeviceType, Driver, SERIAL_DRIVERS, console::uart_console,
        device_tree::DEVICE_TREE_REGISTRY, serial::SerialDriver,
    },
    kernel::current_memory_space,
    mm::address::{Paddr, UsizeConvert},
    pr_info, pr_warn,
    sync::SpinLock,
};

/// 16550 UART 串行端口驱动程序结构体
pub struct Uart16550 {
    serial_port: SpinLock<MmioSerialPort>,
}

impl Driver for Uart16550 {
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool {
        todo!()
    }

    fn device_type(&self) -> crate::device::DeviceType {
        DeviceType::Serial
    }

    fn get_id(&self) -> alloc::string::String {
        format!("ns16550a")
    }

    fn as_serial(&self) -> Option<&dyn SerialDriver> {
        Some(self)
    }
}

impl SerialDriver for Uart16550 {
    fn read(&self) -> u8 {
        // Now the serial port is ready to be used. To send a byte:
        self.serial_port.lock().receive()
    }

    fn write(&self, data: &[u8]) {
        for &byte in data {
            self.serial_port.lock().send(byte);
        }
    }

    fn try_read(&self) -> Option<u8> {
        match self.serial_port.lock().try_receive() {
            Ok(byte) => Some(byte),
            Err(_) => None,
        }
    }
}

pub fn init(node: &FdtNode) {
    let reg = node
        .reg()
        .and_then(|mut reg| reg.next())
        .expect("No reg property found for ns16550a");
    let paddr = reg.starting_address as usize;
    let size = reg.size.unwrap_or(0);
    if size == 0 {
        pr_warn!(
            "[Device] ns16550a device tree node {} has no size",
            node.name
        );
        return;
    }
    let vaddr = current_memory_space()
        .lock()
        .map_mmio(Paddr::from_usize(paddr), size)
        .ok()
        .expect("Failed to map MMIO region");
    let mut serial_port = unsafe { MmioSerialPort::new(vaddr.as_usize()) };
    serial_port.init();
    let driver = Arc::new(Uart16550 {
        serial_port: SpinLock::new(serial_port),
    });
    DRIVERS.write().push(driver.clone());
    SERIAL_DRIVERS.write().push(driver.clone());
    uart_console::init(driver);
    pr_info!("[Device] Serial driver (uart16550) is initialized");
}

pub fn driver_init() {
    DEVICE_TREE_REGISTRY.write().insert("ns16550a", init);
}
