use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::vec;

// P1 重要功能测试

test_case!(test_ext4_read_at_offset, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 从偏移 3 读取 5 字节
    let mut buf = vec![0u8; 5];
    let bytes_read = inode.read_at(3, &mut buf).unwrap();
    kassert!(bytes_read == 5);
    kassert!(&buf[..] == b"34567");
});

test_case!(test_ext4_write_at_offset, {
    // 创建文件并写入初始内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 在偏移 5 处写入
    let bytes_written = inode.write_at(5, b"ABCDE").unwrap();
    kassert!(bytes_written == 5);

    // 读取验证
    let mut buf = vec![0u8; 10];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"01234ABCDE");
});

test_case!(test_ext4_overwrite, {
    // 创建文件并写入初始内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 覆盖写入
    inode.write_at(0, b"World").unwrap();

    // 读取验证
    let mut buf = vec![0u8; 5];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"World");
});

test_case!(test_ext4_append_write, {
    // 创建文件并写入初始内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 追加写入
    inode.write_at(5, b" World").unwrap();

    // 读取验证
    let mut buf = vec![0u8; 11];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Hello World");
});

test_case!(test_ext4_large_data_block, {
    // 创建大数据块 (4KB)
    let fs = create_test_ext4();
    let large_data = alloc::vec![0x42u8; 4096];
    let inode = create_test_file_with_content(&fs, "large.txt", &large_data).unwrap();

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 4096);

    // 读取部分验证
    let mut buf = vec![0u8; 100];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(buf[0] == 0x42);
    kassert!(buf[99] == 0x42);
});

test_case!(test_ext4_multiple_writes, {
    // 创建文件
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let inode = root
        .create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 多次写入
    inode.write_at(0, b"AAA").unwrap();
    inode.write_at(3, b"BBB").unwrap();
    inode.write_at(6, b"CCC").unwrap();

    // 读取验证
    let mut buf = vec![0u8; 9];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"AAABBBCCC");
});

test_case!(test_ext4_chunked_write_across_blocks, {
    const BLOCK_SIZE: usize = 4096;
    const CHUNK_SIZE: usize = 1024;
    const TOTAL_SIZE: usize = BLOCK_SIZE * 2 + CHUNK_SIZE;

    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "chunked.bin").unwrap();
    let mut expected = vec![0u8; TOTAL_SIZE];

    for i in 0..TOTAL_SIZE {
        expected[i] = (i % 251) as u8;
    }

    for offset in (0..TOTAL_SIZE).step_by(CHUNK_SIZE) {
        let end = offset + CHUNK_SIZE;
        let written = inode.write_at(offset, &expected[offset..end]).unwrap();
        kassert!(written == CHUNK_SIZE);
    }

    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == TOTAL_SIZE);

    let mut actual = vec![0u8; TOTAL_SIZE];
    let read = inode.read_at(0, &mut actual).unwrap();
    kassert!(read == TOTAL_SIZE);
    kassert!(actual == expected);
});

test_case!(test_ext4_write_beyond_eof_zero_fills_gap, {
    const TAIL_OFFSET: usize = 8192;
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "gap.bin").unwrap();

    let written = inode.write_at(TAIL_OFFSET, b"tail").unwrap();
    kassert!(written == 4);

    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == TAIL_OFFSET + 4);

    let mut buf = vec![0xffu8; TAIL_OFFSET + 4];
    let read = inode.read_at(0, &mut buf).unwrap();
    kassert!(read == TAIL_OFFSET + 4);

    for byte in &buf[..TAIL_OFFSET] {
        kassert!(*byte == 0);
    }
    kassert!(&buf[TAIL_OFFSET..] == b"tail");
});

test_case!(test_ext4_overwrite_existing_multiblock_file, {
    const TOTAL_SIZE: usize = 8192;
    const OVERWRITE_OFFSET: usize = 3072;
    let fs = create_test_ext4();
    let initial = vec![b'A'; TOTAL_SIZE];
    let inode = create_test_file_with_content(&fs, "overwrite.bin", &initial).unwrap();

    let written = inode.write_at(OVERWRITE_OFFSET, b"BBBBBBBB").unwrap();
    kassert!(written == 8);

    let mut actual = vec![0u8; TOTAL_SIZE];
    let read = inode.read_at(0, &mut actual).unwrap();
    kassert!(read == TOTAL_SIZE);

    for byte in &actual[..OVERWRITE_OFFSET] {
        kassert!(*byte == b'A');
    }
    kassert!(&actual[OVERWRITE_OFFSET..OVERWRITE_OFFSET + 8] == b"BBBBBBBB");
    for byte in &actual[OVERWRITE_OFFSET + 8..] {
        kassert!(*byte == b'A');
    }
});

test_case!(test_ext4_cached_read_invalidated_by_write, {
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "cached-write.bin", b"AAAA").unwrap();

    let mut buf = vec![0u8; 4];
    kassert!(inode.read_at(0, &mut buf).unwrap() == 4);
    kassert!(&buf[..] == b"AAAA");

    kassert!(inode.write_at(0, b"BBBB").unwrap() == 4);

    let mut reread = vec![0u8; 4];
    kassert!(inode.read_at(0, &mut reread).unwrap() == 4);
    kassert!(&reread[..] == b"BBBB");
});

test_case!(
    test_ext4_frame_cached_read_invalidated_by_cross_page_write,
    {
        const TOTAL_SIZE: usize = 4096 + 16;
        let fs = create_test_ext4();
        let initial = vec![b'A'; TOTAL_SIZE];
        let inode = create_test_file_with_content(&fs, "cached-cross-write.bin", &initial).unwrap();

        let mut cached = vec![0u8; 32];
        kassert!(inode.read_at(4096 - 8, &mut cached).unwrap() == 24);
        kassert!(&cached[..24] == &initial[4096 - 8..]);

        kassert!(inode.write_at(4096 - 4, b"BBBBCCCC").unwrap() == 8);

        let mut reread = vec![0u8; 24];
        kassert!(inode.read_at(4096 - 8, &mut reread).unwrap() == 24);
        kassert!(&reread[..4] == b"AAAA");
        kassert!(&reread[4..12] == b"BBBBCCCC");
        kassert!(&reread[12..] == &[b'A'; 12]);
    }
);

test_case!(test_ext4_cached_partial_write_refreshes_single_page, {
    let fs = create_test_ext4();
    let inode =
        create_test_file_with_content(&fs, "cached-partial-write.bin", b"0123456789").unwrap();

    let mut cached = vec![0u8; 10];
    kassert!(inode.read_at(0, &mut cached).unwrap() == 10);
    kassert!(&cached[..] == b"0123456789");

    kassert!(inode.write_at(3, b"abc").unwrap() == 3);

    let mut reread = vec![0u8; 10];
    kassert!(inode.read_at(0, &mut reread).unwrap() == 10);
    kassert!(&reread[..] == b"012abc6789");
});

test_case!(test_ext4_cached_cross_page_write_refreshes_intersections, {
    const TOTAL_SIZE: usize = 4096 * 2;
    let fs = create_test_ext4();
    let initial = vec![b'A'; TOTAL_SIZE];
    let inode =
        create_test_file_with_content(&fs, "cached-cross-page-refresh.bin", &initial).unwrap();

    let mut warm = vec![0u8; TOTAL_SIZE];
    kassert!(inode.read_at(0, &mut warm).unwrap() == TOTAL_SIZE);
    kassert!(warm == initial);

    kassert!(inode.write_at(4096 - 2, b"WXYZ").unwrap() == 4);

    let mut reread = vec![0u8; 8];
    kassert!(inode.read_at(4096 - 4, &mut reread).unwrap() == 8);
    kassert!(&reread[..2] == b"AA");
    kassert!(&reread[2..6] == b"WXYZ");
    kassert!(&reread[6..] == b"AA");
});

test_case!(test_ext4_cached_write_preserves_adjacent_cached_page, {
    const TOTAL_SIZE: usize = 4096 * 3;
    let fs = create_test_ext4();
    let mut initial = vec![0u8; TOTAL_SIZE];
    for page in 0..3 {
        for byte in &mut initial[page * 4096..(page + 1) * 4096] {
            *byte = b'0' + page as u8;
        }
    }
    let inode =
        create_test_file_with_content(&fs, "cached-adjacent-preserve.bin", &initial).unwrap();

    let mut warm = vec![0u8; TOTAL_SIZE];
    kassert!(inode.read_at(0, &mut warm).unwrap() == TOTAL_SIZE);
    kassert!(warm == initial);

    kassert!(inode.write_at(4096 + 17, b"MIDDLE").unwrap() == 6);

    let mut first_page = vec![0u8; 4096];
    let mut third_page = vec![0u8; 4096];
    kassert!(inode.read_at(0, &mut first_page).unwrap() == 4096);
    kassert!(inode.read_at(4096 * 2, &mut third_page).unwrap() == 4096);
    kassert!(first_page == vec![b'0'; 4096]);
    kassert!(third_page == vec![b'2'; 4096]);
});

test_case!(test_ext4_cached_write_beyond_eof_keeps_zero_gap, {
    const TAIL_OFFSET: usize = 4096 + 8;
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "cached-gap-write.bin", b"head").unwrap();

    let mut head = vec![0u8; 4];
    kassert!(inode.read_at(0, &mut head).unwrap() == 4);
    kassert!(&head[..] == b"head");

    kassert!(inode.write_at(TAIL_OFFSET, b"tail").unwrap() == 4);

    let mut all = vec![0xFF; TAIL_OFFSET + 4];
    kassert!(inode.read_at(0, &mut all).unwrap() == TAIL_OFFSET + 4);
    kassert!(&all[..4] == b"head");
    for byte in &all[4..TAIL_OFFSET] {
        kassert!(*byte == 0);
    }
    kassert!(&all[TAIL_OFFSET..] == b"tail");
});

test_case!(test_ext4_cached_read_invalidated_by_truncate, {
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "cached-truncate.bin", b"ABCDEFGH").unwrap();

    let mut buf = vec![0u8; 8];
    kassert!(inode.read_at(0, &mut buf).unwrap() == 8);
    kassert!(&buf[..] == b"ABCDEFGH");

    kassert!(inode.truncate(3).is_ok());

    let mut shrunk = vec![0xFF; 8];
    kassert!(inode.read_at(0, &mut shrunk).unwrap() == 3);
    kassert!(&shrunk[..3] == b"ABC");
});

test_case!(test_ext4_cached_truncate_shrink_invalidates_tail_page, {
    const TOTAL_SIZE: usize = 4096 + 16;
    let fs = create_test_ext4();
    let initial = vec![b'A'; TOTAL_SIZE];
    let inode = create_test_file_with_content(&fs, "cached-truncate-tail.bin", &initial).unwrap();

    let mut warm = vec![0u8; TOTAL_SIZE];
    kassert!(inode.read_at(0, &mut warm).unwrap() == TOTAL_SIZE);
    kassert!(warm == initial);

    kassert!(inode.truncate(4096 + 3).is_ok());

    let mut reread = vec![0xFF; 32];
    kassert!(inode.read_at(4096 - 8, &mut reread).unwrap() == 11);
    kassert!(&reread[..8] == &[b'A'; 8]);
    kassert!(&reread[8..11] == &[b'A'; 3]);
    kassert!(&reread[11..] == &[0xFF; 21]);
});

test_case!(test_ext4_cached_truncate_to_zero_drops_cached_pages, {
    let fs = create_test_ext4();
    let inode =
        create_test_file_with_content(&fs, "cached-truncate-zero.bin", &[b'Z'; 4096]).unwrap();

    let mut warm = vec![0u8; 4096];
    kassert!(inode.read_at(0, &mut warm).unwrap() == 4096);
    kassert!(warm == vec![b'Z'; 4096]);

    kassert!(inode.truncate(0).is_ok());

    let mut reread = vec![0xEE; 8];
    kassert!(inode.read_at(0, &mut reread).unwrap() == 0);
    kassert!(reread == vec![0xEE; 8]);
});

test_case!(test_ext4_cached_truncate_extend_zero_fills_new_range, {
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "cached-truncate-extend.bin", b"head").unwrap();

    let mut warm = vec![0u8; 4];
    kassert!(inode.read_at(0, &mut warm).unwrap() == 4);
    kassert!(&warm[..] == b"head");

    kassert!(inode.truncate(32).is_ok());

    let mut reread = vec![0xFF; 32];
    kassert!(inode.read_at(0, &mut reread).unwrap() == 32);
    kassert!(&reread[..4] == b"head");
    kassert!(reread[4..].iter().all(|byte| *byte == 0));
});

test_case!(
    test_ext4_cached_truncate_extend_across_pages_preserves_old_data,
    {
        const OLD_SIZE: usize = 4096 - 3;
        const NEW_SIZE: usize = 4096 + 9;
        let fs = create_test_ext4();
        let initial = vec![b'K'; OLD_SIZE];
        let inode =
            create_test_file_with_content(&fs, "cached-truncate-cross-extend.bin", &initial)
                .unwrap();

        let mut warm = vec![0u8; OLD_SIZE];
        kassert!(inode.read_at(0, &mut warm).unwrap() == OLD_SIZE);
        kassert!(warm == initial);

        kassert!(inode.truncate(NEW_SIZE).is_ok());

        let mut reread = vec![0xFF; 16];
        kassert!(inode.read_at(OLD_SIZE - 4, &mut reread).unwrap() == 16);
        kassert!(&reread[..4] == &[b'K'; 4]);
        kassert!(reread[4..].iter().all(|byte| *byte == 0));
    }
);

test_case!(test_ext4_unlink_recreate_does_not_read_old_cached_page, {
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let inode = root
        .create("reuse-name.bin", FileMode::from_bits_truncate(0o644))
        .unwrap();

    kassert!(inode.write_at(0, b"OLD!").unwrap() == 4);
    let mut old = vec![0u8; 4];
    kassert!(inode.read_at(0, &mut old).unwrap() == 4);
    kassert!(&old[..] == b"OLD!");

    kassert!(root.unlink("reuse-name.bin").is_ok());
    let new_inode = root
        .create("reuse-name.bin", FileMode::from_bits_truncate(0o644))
        .unwrap();
    kassert!(new_inode.write_at(0, b"NEW?").unwrap() == 4);

    let found = root.lookup("reuse-name.bin").unwrap();
    let mut new = vec![0u8; 4];
    kassert!(found.read_at(0, &mut new).unwrap() == 4);
    kassert!(&new[..] == b"NEW?");
});

test_case!(test_ext4_sync, {
    // 创建文件并写入
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test data").unwrap();

    // 同步文件
    let result = inode.sync();
    kassert!(result.is_ok());

    // 同步文件系统
    let result = fs.sync();
    kassert!(result.is_ok());
});

// P2 边界和错误处理测试

test_case!(test_ext4_read_beyond_eof, {
    // 创建小文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 尝试读取超出文件末尾
    let mut buf = vec![0u8; 10];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read <= 5); // 最多只能读取 5 字节
});

test_case!(test_ext4_cached_read_cross_page_partial_eof, {
    const TOTAL_SIZE: usize = 4096 + 9;
    let fs = create_test_ext4();
    let mut data = vec![0u8; TOTAL_SIZE];
    for i in 0..TOTAL_SIZE {
        data[i] = (i % 251) as u8;
    }
    let inode = create_test_file_with_content(&fs, "partial-eof.bin", &data).unwrap();

    let mut first = vec![0u8; 32];
    let read = inode.read_at(4096 - 8, &mut first).unwrap();
    kassert!(read == 17);
    kassert!(&first[..17] == &data[4096 - 8..]);

    let mut cached = vec![0u8; 32];
    let read = inode.read_at(4096 - 8, &mut cached).unwrap();
    kassert!(read == 17);
    kassert!(&cached[..17] == &data[4096 - 8..]);
});

test_case!(test_ext4_empty_file_read, {
    // 创建空文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "empty.txt", b"").unwrap();

    // 读取空文件
    let mut buf = vec![0u8; 10];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read == 0);
});

test_case!(test_ext4_truncate_to_zero, {
    // 创建文件并写入内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello, World!").unwrap();

    // 截断到 0 字节
    let result = inode.truncate(0);
    kassert!(result.is_ok());

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 0);
});

test_case!(test_ext4_truncate_extend, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 截断扩展到 20 字节
    let result = inode.truncate(20);
    kassert!(result.is_ok());

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 20);
});
