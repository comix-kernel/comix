use super::*;
use crate::fs::simple_fs::SimpleFs;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;

// P1 重要功能测试 - 简化版本,避免 add_child 导致的问题

test_case!(test_dentry_create, {
    // 创建一个测试文件系统和 dentry
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();
    let dentry = Dentry::new("test".to_string(), root_inode.clone());

    kassert!(dentry.name == "test");
    kassert!(Arc::ptr_eq(&dentry.inode, &root_inode));
});

test_case!(test_dentry_lookup_child_not_found, {
    // 查找不存在的子项
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();
    let parent = Dentry::new("parent".to_string(), root_inode);

    let found = parent.lookup_child("nonexistent");
    kassert!(found.is_none());
});

test_case!(test_dentry_parent_none_initially, {
    // 测试初始时没有父节点
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();
    let dentry = Dentry::new("test".to_string(), root_inode);

    let parent = dentry.parent();
    kassert!(parent.is_none());
});

test_case!(test_dentry_add_child, {
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();

    let parent = Dentry::new("parent".to_string(), root_inode.clone());
    let child = Dentry::new("child".to_string(), root_inode.clone());

    parent.add_child(child.clone());

    let found = parent.lookup_child("child");
    kassert!(found.is_some());
    kassert!(Arc::ptr_eq(&found.unwrap(), &child));
});

test_case!(test_dentry_parent_relationship, {
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();

    let parent = Dentry::new("parent".to_string(), root_inode.clone());
    let child = Dentry::new("child".to_string(), root_inode.clone());

    parent.add_child(child.clone());

    let found_parent = child.parent();
    kassert!(found_parent.is_some());
    kassert!(Arc::ptr_eq(&found_parent.unwrap(), &parent));
});

test_case!(test_dentry_full_path, {
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();

    let root = Dentry::new("/".to_string(), root_inode.clone());
    let dir1 = Dentry::new("dir1".to_string(), root_inode.clone());
    let dir2 = Dentry::new("dir2".to_string(), root_inode.clone());
    let file = Dentry::new("file.txt".to_string(), root_inode.clone());

    root.add_child(dir1.clone());
    dir1.add_child(dir2.clone());
    dir2.add_child(file.clone());

    let path = file.full_path();
    kassert!(path == "/dir1/dir2/file.txt");
});

test_case!(test_dentry_multiple_children, {
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();
    let parent = Dentry::new("parent".to_string(), root_inode.clone());

    for i in 0..5 {
        let child_name = format!("child{}", i);
        let child = Dentry::new(child_name.clone(), root_inode.clone());
        parent.add_child(child);
    }

    for i in 0..5 {
        let child_name = format!("child{}", i);
        let found = parent.lookup_child(&child_name);
        kassert!(found.is_some());
    }
});

test_case!(test_dentry_overwrite_child, {
    let fs = SimpleFs::new();
    let root_inode = fs.root_inode();
    let parent = Dentry::new("parent".to_string(), root_inode.clone());

    let child1 = Dentry::new("child".to_string(), root_inode.clone());
    let child2 = Dentry::new("child".to_string(), root_inode.clone());

    parent.add_child(child1.clone());
    parent.add_child(child2.clone());

    let found = parent.lookup_child("child");
    kassert!(found.is_some());
    kassert!(Arc::ptr_eq(&found.unwrap(), &child2));
});
