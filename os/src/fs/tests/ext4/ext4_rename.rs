//! Ext4 rename operation tests

use super::create_test_ext4_with_root;
use crate::vfs::inode::FileMode;

/// Test basic file rename in same directory
#[test_case]
fn test_ext4_rename_file_same_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create a test file
    let file = root
        .create("oldname.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file");

    // Write some content
    let content = b"test content";
    file.write_at(0, content).expect("Failed to write to file");

    // Rename the file
    root.rename("oldname.txt", root.clone(), "newname.txt")
        .expect("Failed to rename file");

    // Old name should not exist
    assert!(root.lookup("oldname.txt").is_err());

    // New name should exist
    let renamed_file = root
        .lookup("newname.txt")
        .expect("Failed to find renamed file");

    // Content should be preserved
    let mut buf = [0u8; 12];
    renamed_file
        .read_at(0, &mut buf)
        .expect("Failed to read renamed file");
    assert_eq!(&buf, content);
}

/// Test directory rename in same directory
#[test_case]
fn test_ext4_rename_dir_same_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create a directory
    let dir = root
        .mkdir("olddir", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create directory");

    // Create a file inside
    let _file = dir
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file in directory");

    // Rename the directory
    root.rename("olddir", root.clone(), "newdir")
        .expect("Failed to rename directory");

    // Old name should not exist
    assert!(root.lookup("olddir").is_err());

    // New name should exist
    let renamed_dir = root
        .lookup("newdir")
        .expect("Failed to find renamed directory");

    // File inside should still be accessible
    renamed_dir
        .lookup("file.txt")
        .expect("Failed to find file in renamed directory");
}

/// Test file move across directories
#[test_case]
fn test_ext4_rename_file_cross_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create two directories
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create dir1");
    let dir2 = root
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create dir2");

    // Create a file in dir1
    let file = dir1
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file");

    let content = b"move me";
    file.write_at(0, content).expect("Failed to write to file");

    // Move file from dir1 to dir2
    dir1.rename("file.txt", dir2.clone(), "moved.txt")
        .expect("Failed to move file");

    // File should not exist in dir1
    assert!(dir1.lookup("file.txt").is_err());

    // File should exist in dir2
    let moved_file = dir2.lookup("moved.txt").expect("Failed to find moved file");

    // Content should be preserved
    let mut buf = [0u8; 7];
    moved_file
        .read_at(0, &mut buf)
        .expect("Failed to read moved file");
    assert_eq!(&buf, content);
}

/// Test directory move across directories
#[test_case]
fn test_ext4_rename_dir_cross_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create directory structure: parent1/child, parent2/
    let parent1 = root
        .mkdir("parent1", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create parent1");
    let parent2 = root
        .mkdir("parent2", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create parent2");
    let child = parent1
        .mkdir("child", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create child");

    // Create a file in child
    child
        .create("marker.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create marker file");

    // Move child from parent1 to parent2
    parent1
        .rename("child", parent2.clone(), "moved_child")
        .expect("Failed to move directory");

    // Child should not exist in parent1
    assert!(parent1.lookup("child").is_err());

    // Child should exist in parent2
    let moved_child = parent2
        .lookup("moved_child")
        .expect("Failed to find moved directory");

    // Marker file should still be accessible
    moved_child
        .lookup("marker.txt")
        .expect("Failed to find marker file in moved directory");
}

/// Test rename with target overwrite (file)
#[test_case]
fn test_ext4_rename_overwrite_file() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create two files
    let file1 = root
        .create("file1.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file1");
    let file2 = root
        .create("file2.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file2");

    let content1 = b"content1";
    let content2 = b"content2";
    file1
        .write_at(0, content1)
        .expect("Failed to write to file1");
    file2
        .write_at(0, content2)
        .expect("Failed to write to file2");

    // Rename file1 to file2 (overwrite)
    root.rename("file1.txt", root.clone(), "file2.txt")
        .expect("Failed to rename with overwrite");

    // file1 should not exist
    assert!(root.lookup("file1.txt").is_err());

    // file2 should have content from file1
    let result_file = root.lookup("file2.txt").expect("Failed to find file2");
    let mut buf = [0u8; 8];
    result_file
        .read_at(0, &mut buf)
        .expect("Failed to read file2");
    assert_eq!(&buf, content1);
}

/// Test rename with target overwrite (empty directory)
#[test_case]
fn test_ext4_rename_overwrite_empty_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create two directories
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create dir1");
    let _dir2 = root
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create dir2");

    // Create a marker file in dir1
    dir1.create("marker.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create marker");

    // Rename dir1 to dir2 (overwrite empty dir2)
    root.rename("dir1", root.clone(), "dir2")
        .expect("Failed to rename with overwrite");

    // dir1 should not exist
    assert!(root.lookup("dir1").is_err());

    // dir2 should have the marker file from dir1
    let result_dir = root.lookup("dir2").expect("Failed to find dir2");
    result_dir
        .lookup("marker.txt")
        .expect("Failed to find marker in dir2");
}

/// Test rename error: target is non-empty directory
#[test_case]
fn test_ext4_rename_error_nonempty_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create two directories
    let _dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create dir1");
    let dir2 = root
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create dir2");

    // Put a file in dir2
    dir2.create("file.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file in dir2");

    // Try to rename dir1 to dir2 (should fail because dir2 is not empty)
    let result = root.rename("dir1", root.clone(), "dir2");
    assert!(result.is_err());

    // Both directories should still exist
    assert!(root.lookup("dir1").is_ok());
    assert!(root.lookup("dir2").is_ok());
}

/// Test rename error: source doesn't exist
#[test_case]
fn test_ext4_rename_error_source_not_found() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Try to rename non-existent file
    let result = root.rename("nonexistent.txt", root.clone(), "newname.txt");
    assert!(result.is_err());
}

/// Test rename error: move directory into itself
#[test_case]
fn test_ext4_rename_error_dir_into_itself() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create a directory
    let dir = root
        .mkdir("dir", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create directory");

    // Try to move directory into itself (should fail)
    let result = root.rename("dir", dir.clone(), "renamed");
    assert!(result.is_err());
}

/// Test rename error: parent is not a directory
#[test_case]
fn test_ext4_rename_error_parent_not_dir() {
    let (_fs, root_dentry) = create_test_ext4_with_root();
    let root = root_dentry.inode.clone();

    // Create a file and a directory
    let file = root
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .expect("Failed to create file");
    let _dir = root
        .mkdir("dir", FileMode::from_bits_truncate(0o755))
        .expect("Failed to create directory");

    // Try to rename with file as new parent (should fail)
    let result = root.rename("dir", file.clone(), "newname");
    assert!(result.is_err());
}
