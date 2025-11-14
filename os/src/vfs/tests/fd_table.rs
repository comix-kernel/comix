use super::*;
use crate::{kassert, test_case};

// P0 核心功能测试

test_case!(test_fdtable_create, {
    // 创建 FDTable
    let fd_table = FDTable::new();

    // FDTable 应该初始化为空
    let result = fd_table.get(0);
    kassert!(result.is_err());
});

test_case!(test_fdtable_alloc, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 分配 FD
    let fd = fd_table.alloc(file).unwrap();
    kassert!(fd >= 0);
});

test_case!(test_fdtable_get, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 分配并获取 FD
    let fd = fd_table.alloc(file.clone()).unwrap();
    let retrieved = fd_table.get(fd);
    kassert!(retrieved.is_ok());
});

test_case!(test_fdtable_close, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 分配 FD
    let fd = fd_table.alloc(file).unwrap();

    // 关闭 FD
    let result = fd_table.close(fd);
    kassert!(result.is_ok());

    // 再次获取应该失败
    let retrieved = fd_table.get(fd);
    kassert!(retrieved.is_err());
});

// P1 重要功能测试

test_case!(test_fdtable_dup, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 分配 FD
    let fd = fd_table.alloc(file.clone()).unwrap();

    // 复制 FD
    let new_fd = fd_table.dup(fd).unwrap();
    kassert!(new_fd != fd);

    // 两个 FD 应该指向同一个文件对象
    let file1 = fd_table.get(fd).unwrap();
    let file2 = fd_table.get(new_fd).unwrap();
    kassert!(Arc::ptr_eq(&file1, &file2));
});

test_case!(test_fdtable_dup2, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode1 = create_test_file_with_content(&fs, "test1.txt", b"test1").unwrap();
    let inode2 = create_test_file_with_content(&fs, "test2.txt", b"test2").unwrap();
    let file1 = create_test_file("test1.txt", inode1, OpenFlags::O_RDONLY);
    let file2 = create_test_file("test2.txt", inode2, OpenFlags::O_RDONLY);

    // 分配两个 FD
    let fd1 = fd_table.alloc(file1.clone()).unwrap();
    let fd2 = fd_table.alloc(file2).unwrap();

    // dup2: 将 fd1 复制到 fd2
    let result = fd_table.dup2(fd1, fd2).unwrap();
    kassert!(result == fd2);

    // fd2 现在应该指向 file1
    let retrieved = fd_table.get(fd2).unwrap();
    kassert!(Arc::ptr_eq(&retrieved, &file1));
});

test_case!(test_fdtable_install_at, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 在指定位置安装文件
    let result = fd_table.install_at(5, file.clone());
    kassert!(result.is_ok());

    // 验证
    let retrieved = fd_table.get(5);
    kassert!(retrieved.is_ok());
    kassert!(Arc::ptr_eq(&retrieved.unwrap(), &file));
});

test_case!(test_fdtable_clone, {
    // 创建 FDTable 和文件
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 分配 FD
    let fd = fd_table.alloc(file.clone()).unwrap();

    // 克隆 FDTable
    let cloned = fd_table.clone_table();

    // 验证克隆的 FDTable 包含相同的文件
    let retrieved = cloned.get(fd);
    kassert!(retrieved.is_ok());
    kassert!(Arc::ptr_eq(&retrieved.unwrap(), &file));
});

// P2 边界和错误处理测试

test_case!(test_fdtable_get_invalid_fd, {
    // 创建 FDTable
    let fd_table = FDTable::new();

    // 获取无效的 FD
    let retrieved = fd_table.get(99);
    kassert!(retrieved.is_err());
});

test_case!(test_fdtable_close_invalid_fd, {
    // 创建 FDTable
    let fd_table = FDTable::new();

    // 关闭无效的 FD
    let result = fd_table.close(99);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::BadFileDescriptor)));
});

test_case!(test_fdtable_dup_invalid_fd, {
    // 创建 FDTable
    let fd_table = FDTable::new();

    // 复制无效的 FD
    let result = fd_table.dup(99);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::BadFileDescriptor)));
});

test_case!(test_fdtable_alloc_multiple, {
    // 创建 FDTable
    let fd_table = FDTable::new();
    let fs = create_test_simplefs();

    // 分配多个 FD
    for i in 0..5 {
        let inode =
            create_test_file_with_content(&fs, &alloc::format!("test{}.txt", i), b"test").unwrap();
        let file = create_test_file(&alloc::format!("test{}.txt", i), inode, OpenFlags::O_RDONLY);
        let result = fd_table.alloc(file);
        kassert!(result.is_ok());
    }
});
