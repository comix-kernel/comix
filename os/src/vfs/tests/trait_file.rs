use super::*;
use crate::vfs::{PipeFile, RegFile};
use crate::{kassert, test_case};

/// 测试 File trait 的多态行为

// P0 核心功能测试

test_case!(test_trait_polymorphism, {
    // 创建不同类型的 File 实现
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"disk file").unwrap();
    let dentry = create_test_dentry("test.txt", inode);

    // RegFile
    let disk_file: Arc<dyn File> = Arc::new(RegFile::new(dentry, OpenFlags::O_RDWR));

    // PipeFile
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let pipe_file_r: Arc<dyn File> = Arc::new(pipe_read);
    let pipe_file_w: Arc<dyn File> = Arc::new(pipe_write);

    // 统一的 File trait 接口调用
    kassert!(disk_file.readable());
    kassert!(pipe_file_r.readable());
    kassert!(!pipe_file_w.readable());
});

test_case!(test_disk_file_vs_pipe_file_lseek, {
    // RegFile 支持 lseek
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();
    let disk_file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    let result = disk_file.lseek(5, SeekWhence::Set);
    kassert!(result.is_ok());
    kassert!(result.unwrap() == 5);

    // PipeFile 不支持 lseek
    let (pipe_read, _) = PipeFile::create_pair();
    let pipe_file: Arc<dyn File> = Arc::new(pipe_read);

    let result = pipe_file.lseek(0, SeekWhence::Set);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotSupported)));
});

test_case!(test_disk_file_vs_pipe_file_offset, {
    // RegFile 有 offset
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let disk_file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    kassert!(disk_file.offset() == 0);
    let mut buf = [0u8; 2];
    disk_file.read(&mut buf).unwrap();
    kassert!(disk_file.offset() == 2);

    // PipeFile 的 offset 总是 0（使用默认实现）
    let (pipe_read, _) = PipeFile::create_pair();
    let pipe_file: Arc<dyn File> = Arc::new(pipe_read);

    kassert!(pipe_file.offset() == 0);
});

test_case!(test_disk_file_vs_pipe_file_flags, {
    // RegFile 有真实的 flags
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let disk_file = create_test_file("test.txt", inode, OpenFlags::O_RDWR);

    let flags = disk_file.flags();
    kassert!(flags.contains(OpenFlags::O_RDWR));

    // PipeFile 的 flags 是空的（使用默认实现）
    let (pipe_read, _) = PipeFile::create_pair();
    let pipe_file: Arc<dyn File> = Arc::new(pipe_read);

    let flags = pipe_file.flags();
    kassert!(flags.is_empty());
});

// P1 重要功能测试

test_case!(test_fdtable_with_mixed_files, {
    // 在 FDTable 中混合存储不同类型的文件
    let fd_table = FDTable::new();

    // 添加 RegFile
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"disk").unwrap();
    let disk_file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);
    let fd1 = fd_table.alloc(disk_file).unwrap();

    // 添加 PipeFile
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let fd2 = fd_table
        .alloc(Arc::new(pipe_read) as Arc<dyn File>)
        .unwrap();
    let fd3 = fd_table
        .alloc(Arc::new(pipe_write) as Arc<dyn File>)
        .unwrap();

    // 验证可以取出
    kassert!(fd_table.get(fd1).is_ok());
    kassert!(fd_table.get(fd2).is_ok());
    kassert!(fd_table.get(fd3).is_ok());

    // 验证不同的 fd
    kassert!(fd1 != fd2 && fd2 != fd3 && fd1 != fd3);
});

test_case!(test_read_write_through_fdtable, {
    // 通过 FDTable 读写不同类型的文件
    let fd_table = FDTable::new();

    // 创建管道
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let fd_read = fd_table
        .alloc(Arc::new(pipe_read) as Arc<dyn File>)
        .unwrap();
    let fd_write = fd_table
        .alloc(Arc::new(pipe_write) as Arc<dyn File>)
        .unwrap();

    // 通过 fd 写入
    let write_file = fd_table.get(fd_write).unwrap();
    write_file.write(b"via fdtable").unwrap();

    // 通过 fd 读取
    let read_file = fd_table.get(fd_read).unwrap();
    let mut buf = [0u8; 11];
    let nread = read_file.read(&mut buf).unwrap();
    kassert!(nread == 11);
    kassert!(&buf[..] == b"via fdtable");
});

// P2 边界和错误处理测试

test_case!(test_metadata_consistency, {
    // 不同类型的文件返回正确的 metadata
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test").unwrap();
    let disk_file = create_test_file("test.txt", inode, OpenFlags::O_RDONLY);

    let meta = disk_file.metadata().unwrap();
    kassert!(meta.inode_type == InodeType::File);
    kassert!(meta.size == 4);

    // PipeFile 的 metadata
    let (pipe_read, _) = PipeFile::create_pair();
    let pipe_file: Arc<dyn File> = Arc::new(pipe_read);

    let meta = pipe_file.metadata().unwrap();
    kassert!(meta.inode_type == InodeType::Fifo);
    kassert!(meta.size == 0);
});
