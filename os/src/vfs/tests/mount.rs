use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::string::String;

// P1 重要功能测试

test_case!(test_mount_fs, {
    // 创建文件系统
    let fs = create_test_simplefs();

    // 挂载到 /test
    let result = MOUNT_TABLE.mount(
        fs.clone(),
        "/test",
        MountFlags::empty(),
        Some(String::from("testfs")),
    );
    kassert!(result.is_ok());

    // 查找挂载点
    let mount = MOUNT_TABLE.find_mount("/test");
    kassert!(mount.is_some());
});

test_case!(test_mount_list, {
    // 挂载文件系统
    let fs1 = create_test_simplefs();

    MOUNT_TABLE
        .mount(fs1, "/mnt_test", MountFlags::empty(), None)
        .ok();

    // 列出挂载点
    let mounts = MOUNT_TABLE.list_mounts();
    // 至少应该有根文件系统
    kassert!(mounts.len() >= 1);
});

// P2 边界和错误处理测试

test_case!(test_umount_fs, {
    // 创建文件系统并挂载
    let fs = create_test_simplefs();
    MOUNT_TABLE
        .mount(fs, "/test_umount2", MountFlags::empty(), None)
        .ok();

    // 卸载
    let result = MOUNT_TABLE.umount("/test_umount2");
    kassert!(result.is_ok());

    // 卸载后应该找不到原挂载点（可能会匹配到根挂载点，但不应该是 /test_umount2）
    let mount = MOUNT_TABLE.find_mount("/test_umount2");
    if let Some(m) = mount {
        kassert!(m.mount_path != "/test_umount2");
    }
    // 如果没有根挂载点，应该返回 None
    // 如果有根挂载点，应该返回根而不是 /test_umount2
});
