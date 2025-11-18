use super::*;
use crate::{kassert, test_case};

// P0 核心功能测试

test_case!(test_file_read, {
    // 创建文件系统和文件
    let fs = create_test_simplefs();
    let content = b"Hello, World!";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    // 创建 File 对象
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 读取文件内容
    let mut buf = [0u8; 13];
    let bytes_read = file.read(&mut buf).unwrap();
    kassert!(bytes_read == 13);
    kassert!(&buf[..] == content);
});

test_case!(test_file_write, {
    // 创建文件系统和文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"").unwrap();

    // 创建 File 对象（可写）
    let file = create_test_file("test.txt", inode.clone(), OpenFlags::O_WRONLY);

    // 写入数据
    let content = b"Hello, Rust!";
    let bytes_written = file.write(content).unwrap();
    kassert!(bytes_written == 12);

    // 读取验证
    let mut buf = [0u8; 12];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == content);
});

test_case!(test_file_lseek_set, {
    // 创建文件
    let fs = create_test_simplefs();
    let content = b"0123456789";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // Seek 到偏移 5
    let new_offset = file.lseek(5, SeekWhence::Set).unwrap();
    kassert!(new_offset == 5);

    // 读取数据
    let mut buf = [0u8; 5];
    file.read(&mut buf).unwrap();
    kassert!(&buf[..] == b"56789");
});

test_case!(test_file_lseek_cur, {
    // 创建文件
    let fs = create_test_simplefs();
    let content = b"0123456789";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 读取 3 个字节
    let mut buf = [0u8; 3];
    file.read(&mut buf).unwrap();

    // 从当前位置 seek +2
    let new_offset = file.lseek(2, SeekWhence::Cur).unwrap();
    kassert!(new_offset == 5);

    // 读取数据
    let mut buf2 = [0u8; 2];
    file.read(&mut buf2).unwrap();
    kassert!(&buf2[..] == b"56");
});

test_case!(test_file_lseek_end, {
    // 创建文件
    let fs = create_test_simplefs();
    let content = b"0123456789";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // Seek 到末尾 -5
    let new_offset = file.lseek(-5, SeekWhence::End).unwrap();
    kassert!(new_offset == 5);

    // 读取数据
    let mut buf = [0u8; 5];
    file.read(&mut buf).unwrap();
    kassert!(&buf[..] == b"56789");
});

// P1 重要功能测试

test_case!(test_file_append_mode, {
    // 创建文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 以追加模式打开
    let file = create_test_file(
        "test.txt",
        inode.clone(),
        OpenFlags::O_WRONLY | OpenFlags::O_APPEND,
    );

    // 写入数据（应该追加到末尾）
    file.write(b", World!").unwrap();

    // 读取验证
    let mut buf = [0u8; 13];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Hello, World!");
});

test_case!(test_file_readable_check, {
    // 创建文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();

    // 只写模式
    let file_wo = create_test_file("test.txt", inode.clone(), OpenFlags::O_WRONLY);
    kassert!(!file_wo.readable());

    // 只读模式
    let file_ro = create_test_file("test.txt", inode.clone(), OpenFlags::O_RDONLY);
    kassert!(file_ro.readable());

    // 读写模式
    let file_rw = create_test_file("test.txt", inode, OpenFlags::O_RDWR);
    kassert!(file_rw.readable());
});

test_case!(test_file_writable_check, {
    // 创建文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();

    // 只读模式
    let file_ro = create_test_file("test.txt", inode.clone(), OpenFlags::O_RDONLY);
    kassert!(!file_ro.writable());

    // 只写模式
    let file_wo = create_test_file("test.txt", inode.clone(), OpenFlags::O_WRONLY);
    kassert!(file_wo.writable());

    // 读写模式
    let file_rw = create_test_file("test.txt", inode, OpenFlags::O_RDWR);
    kassert!(file_rw.writable());
});

// P2 边界和错误处理测试

test_case!(test_file_read_permission_denied, {
    // 创建文件（只写）
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_WRONLY);

    // 尝试读取
    let mut buf = [0u8; 4];
    let result = file.read(&mut buf);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::PermissionDenied)));
});

test_case!(test_file_write_permission_denied, {
    // 创建文件（只读）
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // 尝试写入
    let result = file.write(b"data");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::PermissionDenied)));
});

test_case!(test_file_lseek_negative_set, {
    // 创建文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    // Seek 到负偏移（SET 模式不允许）
    let result = file.lseek(-5, SeekWhence::Set);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::InvalidArgument)));
});
