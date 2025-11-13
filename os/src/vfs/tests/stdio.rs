use super::*;
use crate::{kassert, test_case};

// P1 重要功能测试

test_case!(test_create_stdio_files, {
    // 创建标准 I/O 文件
    let (stdin, stdout, stderr) = create_stdio_files();

    // 验证 stdin
    kassert!(stdin.flags.readable());
    kassert!(!stdin.flags.writable());

    // 验证 stdout
    kassert!(!stdout.flags.readable());
    kassert!(stdout.flags.writable());

    // 验证 stderr
    kassert!(!stderr.flags.readable());
    kassert!(stderr.flags.writable());
});
