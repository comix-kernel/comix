use super::super::*;
use crate::vfs::path::PathComponent;
use crate::{kassert, test_case};
use alloc::string::ToString;

// P0 核心功能测试

test_case!(test_normalize_path_absolute, {
    // 测试绝对路径
    let result = normalize_path("/foo/bar");
    kassert!(result == "/foo/bar");

    // 根目录
    let result = normalize_path("/");
    kassert!(result == "/");

    // 多个斜杠
    let result = normalize_path("///foo///bar///");
    kassert!(result == "/foo/bar");
});

test_case!(test_normalize_path_current, {
    // 测试 "." 组件
    let result = normalize_path("/foo/./bar");
    kassert!(result == "/foo/bar");

    let result = normalize_path("./foo");
    kassert!(result == "foo");

    let result = normalize_path(".");
    kassert!(result == ".");
});

test_case!(test_normalize_path_parent, {
    // 测试 ".." 组件
    let result = normalize_path("/foo/bar/..");
    kassert!(result == "/foo");

    let result = normalize_path("/foo/../bar");
    kassert!(result == "/bar");

    // 根目录不能越过
    let result = normalize_path("/..");
    kassert!(result == "/");

    let result = normalize_path("/../..");
    kassert!(result == "/");
});

test_case!(test_normalize_path_relative, {
    // 测试相对路径
    let result = normalize_path("foo/bar");
    kassert!(result == "foo/bar");

    let result = normalize_path("foo/../bar");
    kassert!(result == "bar");

    // 相对路径可以有 ".." 前缀
    let result = normalize_path("../foo");
    kassert!(result == "../foo");

    let result = normalize_path("../../foo");
    kassert!(result == "../../foo");
});

test_case!(test_split_path_absolute, {
    // 测试分割绝对路径
    let result = split_path("/foo/bar.txt");
    kassert!(result.is_ok());
    let (dir, filename) = result.unwrap();
    kassert!(dir == "/foo");
    kassert!(filename == "bar.txt");

    // 根目录下的文件
    let result = split_path("/hello");
    kassert!(result.is_ok());
    let (dir, filename) = result.unwrap();
    kassert!(dir == "/");
    kassert!(filename == "hello");
});

test_case!(test_split_path_relative, {
    // 测试分割相对路径
    let result = split_path("foo/bar.txt");
    kassert!(result.is_ok());
    let (dir, filename) = result.unwrap();
    kassert!(dir == "foo");
    kassert!(filename == "bar.txt");

    // 无路径分隔符
    let result = split_path("hello.txt");
    kassert!(result.is_ok());
    let (dir, filename) = result.unwrap();
    kassert!(dir == ".");
    kassert!(filename == "hello.txt");
});

// P2 边界和错误处理测试

test_case!(test_normalize_path_empty, {
    // 空路径被当作当前目录
    let result = normalize_path("");
    kassert!(result == ".");
});

test_case!(test_normalize_path_complex, {
    // 复杂路径
    let result = normalize_path("/foo/./bar/../baz/./qux/..");
    kassert!(result == "/foo/baz");

    let result = normalize_path("foo/bar/../../baz");
    kassert!(result == "baz");
});

test_case!(test_split_path_trailing_slash, {
    // 结尾的斜杠
    let result = split_path("/foo/bar/");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::InvalidArgument)));
});

test_case!(test_split_path_multiple_slashes, {
    // 多个斜杠会被规范化
    let result = split_path("///foo///bar.txt");
    kassert!(result.is_ok());
    let (dir, filename) = result.unwrap();
    kassert!(dir == "/foo");
    kassert!(filename == "bar.txt");
});

// P1 重要功能测试

test_case!(test_parse_path_components, {
    // 测试解析路径组件
    let components = parse_path("/foo/bar");
    kassert!(components.len() == 3);
    kassert!(components[0] == PathComponent::Root);
    kassert!(components[1] == PathComponent::Normal("foo".to_string()));
    kassert!(components[2] == PathComponent::Normal("bar".to_string()));

    let components = parse_path("foo/./bar/../baz");
    kassert!(components.len() == 5);
    kassert!(components[0] == PathComponent::Normal("foo".to_string()));
    kassert!(components[1] == PathComponent::Current);
    kassert!(components[2] == PathComponent::Normal("bar".to_string()));
    kassert!(components[3] == PathComponent::Parent);
    kassert!(components[4] == PathComponent::Normal("baz".to_string()));
});

test_case!(test_normalize_path_root_parent, {
    // 根目录的父目录是自己
    let result = normalize_path("/foo/..");
    kassert!(result == "/");

    let result = normalize_path("/foo/bar/../..");
    kassert!(result == "/");
});

// P4 跨文件系统 lookup 测试

test_case!(test_lookup_across_mount_point, {
    use super::create_test_simplefs;

    // 创建一个测试文件系统
    let fs = create_test_simplefs();

    // 在文件系统内创建一个文件
    let root = fs.root_inode();
    let create_result = root.create("testfile", FileMode::from_bits_truncate(0o644));
    kassert!(create_result.is_ok());

    // 挂载到 /mnt_lookup_test
    let mount_result = MOUNT_TABLE.mount(fs.clone(), "/mnt_lookup_test", MountFlags::empty(), None);
    kassert!(mount_result.is_ok());

    // 获取挂载点
    let mount_point = MOUNT_TABLE.find_mount("/mnt_lookup_test");
    kassert!(mount_point.is_some());

    if let Some(mp) = mount_point {
        // 从挂载点的根开始查找文件
        let lookup_result = vfs_lookup_from(mp.root.clone(), "testfile");
        kassert!(lookup_result.is_ok());
    }

    // 清理
    MOUNT_TABLE.umount("/mnt_lookup_test").ok();
});

test_case!(test_dentry_mount_cache, {
    use super::create_test_simplefs;

    // 创建测试文件系统
    let fs = create_test_simplefs();

    // 挂载到 /cache_test
    let mount_result = MOUNT_TABLE.mount(fs.clone(), "/cache_test", MountFlags::empty(), None);
    kassert!(mount_result.is_ok());

    // 获取挂载点
    let mount_point = MOUNT_TABLE.find_mount("/cache_test");
    kassert!(mount_point.is_some());

    if let Some(mp) = mount_point {
        // 检查挂载点的根 dentry
        kassert!(mp.root.name == "/");

        // 检查是否有挂载缓存（通过 find_mount 已经验证）
        kassert!(mp.mount_path == "/cache_test");
    }

    // 清理
    MOUNT_TABLE.umount("/cache_test").ok();
});

test_case!(test_check_mount_point_function, {
    use super::create_test_simplefs;

    // 创建根文件系统并挂载
    let root_fs = create_test_simplefs();
    let mount_result = MOUNT_TABLE.mount(root_fs.clone(), "/", MountFlags::empty(), None);

    // 如果根已经挂载，跳过（测试环境可能已有根）
    if mount_result.is_err() {
        return;
    }

    // 在根文件系统创建目录
    let root_inode = root_fs.root_inode();
    let mkdir_result = root_inode.mkdir("mountpoint", FileMode::from_bits_truncate(0o755));
    kassert!(mkdir_result.is_ok());

    // 创建另一个文件系统并挂载到 /mountpoint
    let child_fs = create_test_simplefs();
    let mount_child = MOUNT_TABLE.mount(child_fs.clone(), "/mountpoint", MountFlags::empty(), None);
    kassert!(mount_child.is_ok());

    // 在子文件系统创建文件
    let child_root = child_fs.root_inode();
    let create_result = child_root.create("file_in_child", FileMode::from_bits_truncate(0o644));
    kassert!(create_result.is_ok());

    // 通过 vfs_lookup 访问 /mountpoint，应该跨越挂载点
    let dentry = vfs_lookup("/mountpoint");
    if let Ok(d) = dentry {
        // 应该返回挂载文件系统的根
        kassert!(d.name == "/");

        // 继续查找子文件系统中的文件
        let file_lookup = vfs_lookup_from(d, "file_in_child");
        kassert!(file_lookup.is_ok());
    }

    // 清理
    MOUNT_TABLE.umount("/mountpoint").ok();
    MOUNT_TABLE.umount("/").ok();
});

// P1 测试 vfs_lookup_no_follow 函数
test_case!(test_vfs_lookup_no_follow_nonexistent, {
    // 测试查找不存在的路径
    use crate::vfs::vfs_lookup_no_follow;

    // 查找一个不太可能存在的路径
    let result = vfs_lookup_no_follow("/nonexistent_test_file_12345");
    // 应该返回错误
    kassert!(result.is_err());
});
