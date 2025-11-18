use super::*;
use crate::{kassert, test_case};

// P1 重要功能测试

test_case!(test_create_stdio_files, {
    // 创建标准 I/O 文件
    let (stdin, stdout, stderr) = create_stdio_files();

    // 验证 stdin
    kassert!(stdin.readable());
    kassert!(!stdin.writable());

    // 验证 stdout
    kassert!(!stdout.readable());
    kassert!(stdout.writable());

    // 验证 stderr
    kassert!(!stderr.readable());
    kassert!(stderr.writable());
});
