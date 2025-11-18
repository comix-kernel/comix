use super::*;
use crate::vfs::PipeFile;
use crate::{kassert, test_case};

// P0 核心功能测试

test_case!(test_pipe_basic_read_write, {
    // 创建管道对
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);
    let write_file: Arc<dyn File> = Arc::new(pipe_write);

    // 写入数据
    let write_buf = b"hello pipe";
    let nwritten = write_file.write(write_buf).unwrap();
    kassert!(nwritten == 10);

    // 读取数据
    let mut read_buf = [0u8; 10];
    let nread = read_file.read(&mut read_buf).unwrap();
    kassert!(nread == 10);
    kassert!(&read_buf[..] == b"hello pipe");
});

test_case!(test_pipe_empty_read, {
    // 创建管道对
    let (pipe_read, _pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);

    // 空管道读取应返回 0（或 WouldBlock，取决于实现）
    let mut buf = [0u8; 10];
    let result = read_file.read(&mut buf);
    // 允许返回 Ok(0) 或 Err(WouldBlock)
    kassert!(result.is_ok() || matches!(result, Err(FsError::WouldBlock)));
    if let Ok(n) = result {
        kassert!(n == 0);
    }
});

test_case!(test_pipe_multiple_writes, {
    // 创建管道对
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);
    let write_file: Arc<dyn File> = Arc::new(pipe_write);

    // 多次写入
    write_file.write(b"hello").unwrap();
    write_file.write(b" ").unwrap();
    write_file.write(b"world").unwrap();

    // 读取所有数据
    let mut buf = [0u8; 11];
    let nread = read_file.read(&mut buf).unwrap();
    kassert!(nread == 11);
    kassert!(&buf[..] == b"hello world");
});

// P1 重要功能测试

test_case!(test_pipe_not_seekable, {
    // 创建管道对
    let (pipe_read, _) = PipeFile::create_pair();
    let file: Arc<dyn File> = Arc::new(pipe_read);

    // 管道不支持 lseek
    let result = file.lseek(0, SeekWhence::Set);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotSupported)));
});

test_case!(test_pipe_readable_writable, {
    // 创建管道对
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);
    let write_file: Arc<dyn File> = Arc::new(pipe_write);

    // 读端只可读
    kassert!(read_file.readable());
    kassert!(!read_file.writable());

    // 写端只可写
    kassert!(!write_file.readable());
    kassert!(write_file.writable());
});

test_case!(test_pipe_wrong_direction_read, {
    // 创建管道对
    let (_pipe_read, pipe_write) = PipeFile::create_pair();
    let write_file: Arc<dyn File> = Arc::new(pipe_write);

    // 尝试从写端读取
    let mut buf = [0u8; 10];
    let result = write_file.read(&mut buf);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::InvalidArgument)));
});

test_case!(test_pipe_wrong_direction_write, {
    // 创建管道对
    let (pipe_read, _pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);

    // 尝试向读端写入
    let result = read_file.write(b"test");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::InvalidArgument)));
});

test_case!(test_pipe_metadata, {
    // 创建管道对
    let (pipe_read, _) = PipeFile::create_pair();
    let file: Arc<dyn File> = Arc::new(pipe_read);

    // 管道应该返回虚拟元数据
    let meta = file.metadata();
    kassert!(meta.is_ok());
    let meta = meta.unwrap();
    kassert!(meta.inode_type == InodeType::Fifo);
    kassert!(meta.size == 0); // 管道没有固定大小
});

// P2 边界和错误处理测试

test_case!(test_pipe_partial_read, {
    // 创建管道对
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);
    let write_file: Arc<dyn File> = Arc::new(pipe_write);

    // 写入 10 字节
    write_file.write(b"0123456789").unwrap();

    // 只读取 5 字节
    let mut buf = [0u8; 5];
    let nread = read_file.read(&mut buf).unwrap();
    kassert!(nread == 5);
    kassert!(&buf[..] == b"01234");

    // 再读取剩余 5 字节
    let mut buf2 = [0u8; 5];
    let nread2 = read_file.read(&mut buf2).unwrap();
    kassert!(nread2 == 5);
    kassert!(&buf2[..] == b"56789");
});

test_case!(test_pipe_zero_length_operations, {
    // 创建管道对
    let (pipe_read, pipe_write) = PipeFile::create_pair();
    let read_file: Arc<dyn File> = Arc::new(pipe_read);
    let write_file: Arc<dyn File> = Arc::new(pipe_write);

    // 写入 0 字节应该成功
    let result = write_file.write(b"");
    kassert!(result.is_ok());
    kassert!(result.unwrap() == 0);

    // 读取 0 字节应该成功
    let mut buf = [];
    let result = read_file.read(&mut buf);
    kassert!(result.is_ok());
    kassert!(result.unwrap() == 0);
});
