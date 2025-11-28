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

// P3 Overmount 测试

test_case!(test_overmount, {
    // 创建两个文件系统
    let fs1 = create_test_simplefs();
    let fs2 = create_test_simplefs();

    // 在同一路径挂载两次
    let result1 = MOUNT_TABLE.mount(fs1, "/overmount_test", MountFlags::empty(), None);
    kassert!(result1.is_ok());

    let result2 = MOUNT_TABLE.mount(fs2, "/overmount_test", MountFlags::empty(), None);
    kassert!(result2.is_ok()); // 应该支持 overmount

    // 查找挂载点应该返回最新的
    let mount = MOUNT_TABLE.find_mount("/overmount_test");
    kassert!(mount.is_some());

    // 卸载一次，应该还能找到挂载点（下层的）
    let umount_result = MOUNT_TABLE.umount("/overmount_test");
    kassert!(umount_result.is_ok());

    let mount_after = MOUNT_TABLE.find_mount("/overmount_test");
    kassert!(mount_after.is_some()); // 应该还有下层挂载

    // 再卸载一次，这次应该彻底没有了
    let umount_result2 = MOUNT_TABLE.umount("/overmount_test");
    kassert!(umount_result2.is_ok());

    let mount_final = MOUNT_TABLE.find_mount("/overmount_test");
    if let Some(m) = mount_final {
        kassert!(m.mount_path != "/overmount_test");
    }
});

test_case!(test_umount_root_should_fail, {
    // 尝试卸载根文件系统应该失败
    let result = MOUNT_TABLE.umount("/");
    kassert!(result.is_err());
});
