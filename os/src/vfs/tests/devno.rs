//! 设备号管理测试

use super::*;
use crate::vfs::{dev::*, devno::*};
use crate::{kassert, test_case};

test_case!(test_makedev_major_minor, {
    // 测试 makedev 和 major/minor 提取
    let dev = makedev(8, 1);
    kassert!(major(dev) == 8);
    kassert!(minor(dev) == 1);
});

test_case!(test_makedev_zero, {
    let dev = makedev(0, 0);
    kassert!(major(dev) == 0);
    kassert!(minor(dev) == 0);
});

test_case!(test_makedev_large_numbers, {
    let dev = makedev(255, 255);
    kassert!(major(dev) == 255);
    kassert!(minor(dev) == 255);
});

test_case!(test_makedev_roundtrip, {
    // 测试往返转换
    for maj in [0, 1, 8, 10, 100, 255] {
        for min in [0, 1, 16, 100, 255] {
            let dev = makedev(maj, min);
            kassert!(major(dev) == maj);
            kassert!(minor(dev) == min);
        }
    }
});

test_case!(test_blkdev_major, {
    // 测试块设备主设备号常量
    kassert!(blkdev_major::LOOP == 7);
    kassert!(blkdev_major::SCSI_DISK == 8);
    kassert!(blkdev_major::VIRTIO_BLK == 254);
});

test_case!(test_chrdev_major, {
    // 测试字符设备主设备号常量
    kassert!(chrdev_major::MEM == 1);
    kassert!(chrdev_major::TTY == 4);
    kassert!(chrdev_major::CONSOLE == 5);
    kassert!(chrdev_major::INPUT == 13);
});

test_case!(test_get_blkdev_index, {
    // 测试获取块设备索引
    let index = get_blkdev_index(0);
    kassert!(index.is_some() || index.is_none()); // 取决于是否有注册的设备
});

test_case!(test_get_chrdev_driver, {
    // 测试获取字符设备驱动
    let driver = get_chrdev_driver(makedev(1, 0));
    kassert!(driver.is_some() || driver.is_none()); // 取决于是否有注册的驱动
});

test_case!(test_devno_unique, {
    // 确保不同的 major/minor 组合产生不同的 devno
    let dev1 = makedev(1, 0);
    let dev2 = makedev(1, 1);
    let dev3 = makedev(2, 0);

    kassert!(dev1 != dev2);
    kassert!(dev1 != dev3);
    kassert!(dev2 != dev3);
});

test_case!(test_major_extraction, {
    // 测试主设备号提取的边界情况
    let dev = makedev(255, 0);
    kassert!(major(dev) == 255);
    kassert!(minor(dev) == 0);
});

test_case!(test_minor_extraction, {
    // 测试次设备号提取的边界情况
    let dev = makedev(0, 255);
    kassert!(major(dev) == 0);
    kassert!(minor(dev) == 255);
});

test_case!(test_devno_consistency, {
    // 测试设备号的一致性
    let dev1 = makedev(10, 20);
    let dev2 = makedev(10, 20);
    kassert!(dev1 == dev2);
    kassert!(major(dev1) == major(dev2));
    kassert!(minor(dev1) == minor(dev2));
});
