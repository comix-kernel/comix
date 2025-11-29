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

// P1 测试 vfs_lookup_no_follow 函数

test_case!(test_vfs_lookup_no_follow_nonexistent, {
    // 测试查找不存在的路径
    use crate::vfs::vfs_lookup_no_follow;

    // 查找一个不太可能存在的路径
    let result = vfs_lookup_no_follow("/nonexistent_test_file_12345");
    // 应该返回错误
    kassert!(result.is_err());
});
