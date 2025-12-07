//! Build script for the OS kernel
//!
//! This script automatically:
//! 1. Compiles user programs in ../user directory
//! 2. Packs them into an init_simple_fs image
//! 3. Embeds the image into the kernel binary

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // 获取环境变量
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    // 设置路径
    let project_root = PathBuf::from(&manifest_dir).parent().unwrap().to_path_buf();
    let user_dir = project_root.join("user");
    let _user_bin_dir = user_dir.join("bin");
    let img_path = PathBuf::from(&out_dir).join("simple_fs.img");
    let _tool_path = project_root.join("scripts").join("make_init_simple_fs.py");

    println!("cargo:rerun-if-changed=../user");
    println!("cargo:rerun-if-changed=../scripts/make_init_simple_fs.py");

    // 步骤 1: 编译用户程序
    if user_dir.exists() {
        println!("cargo:warning=[build.rs] Building user programs...");
        let status = Command::new("make")
            .current_dir(&user_dir)
            .env("BUILD_MODE", "release")
            // 清除可能从父目录继承的 CARGO 环境变量，避免用户程序继承 os 的构建配置
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("CARGO_BUILD_RUSTFLAGS")
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=[build.rs] User programs built successfully");
            }
            Ok(s) => {
                panic!(
                    "User program build failed with status: {}. Aborting kernel build.",
                    s
                );
            }
            Err(e) => {
                panic!(
                    "Failed to execute make for user programs: {}. Aborting kernel build.",
                    e
                );
            }
        }
    } else {
        println!("cargo:warning=[build.rs] User directory not found, skipping user build");
    }

    // 步骤 2: 打包 simple_fs 镜像 (暂时禁用，直接创建空镜像)
    println!("cargo:warning=[build.rs] Creating empty simple_fs image (user programs disabled)...");
    create_empty_image(&img_path);

    // let status = Command::new("python3")
    //     .arg(&tool_path)
    //     .arg(&user_bin_dir)
    //     .arg(&img_path)
    //     .status();
    //
    // match status {
    //     Ok(s) if s.success() => {
    //         let img_size = fs::metadata(&img_path).map(|m| m.len()).unwrap_or(0);
    //         println!(
    //             "cargo:warning=[build.rs] Simple_fs image created: {} bytes",
    //             img_size
    //         );
    //     }
    //     Ok(s) => {
    //         println!(
    //             "cargo:warning=[build.rs] Failed to pack simple_fs: status {}",
    //             s
    //         );
    //         // 创建空镜像以避免编译失败
    //         create_empty_image(&img_path);
    //     }
    //     Err(e) => {
    //         println!(
    //             "cargo:warning=[build.rs] Failed to run make_init_simple_fs.py: {}",
    //             e
    //         );
    //         create_empty_image(&img_path);
    //     }
    // }
    //
    // // 验证镜像文件存在
    // if !img_path.exists() {
    //     println!("cargo:warning=[build.rs] Image not found, creating empty image");
    //     create_empty_image(&img_path);
    // }

    // 输出镜像路径供代码使用
    println!("cargo:rustc-env=SIMPLE_FS_IMAGE={}", img_path.display());

    // 步骤 3: 创建 ext4 镜像
    // 检测是否为测试模式
    // 注意: CARGO_CFG_TEST 只在运行测试时设置，编译时不会设置
    // 因此我们检查 TEST 环境变量 (由 Makefile 传递) 或检查是否有测试相关的 cfg
    let is_test = env::var("TEST").is_ok()
        || env::var("CARGO_CFG_TEST").is_ok()
        || env::var("PROFILE").map(|p| p == "test").unwrap_or(false);

    // 3.1: 创建用于 include_bytes! 嵌入的镜像
    let ext4_embed_img = PathBuf::from(&out_dir).join("ext4_test.img");
    if is_test {
        // 测试模式: 创建 8MB 镜像用于测试
        // 只有在测试模式下才需要这个环境变量
        println!("cargo:warning=[build.rs] Creating ext4 test image for embedding (8MB)...");
        create_ext4_test_image(&ext4_embed_img);
        println!("cargo:rustc-env=EXT4_FS_IMAGE={}", ext4_embed_img.display());
    } else {
        // IDE 修复: 即使不在测试模式下，也需要定义 EXT4_FS_IMAGE 环境变量
        // 这里的代码会被 rust-analyzer 分析，如果缺少环境变量会报错
        // 我们创建一个空的伪文件来满足 include_bytes! 的需求
        let dummy_img = PathBuf::from(&out_dir).join("ext4_test_dummy.img");
        if !dummy_img.exists() {
            let _ = fs::write(&dummy_img, &[]);
        }
        println!(
            "cargo:warning=[build.rs] Skipping real test image creation (using dummy for IDE)"
        );
        println!("cargo:rustc-env=EXT4_FS_IMAGE={}", dummy_img.display());
    }

    // 3.2: 非测试模式下创建完整的运行时镜像
    if !is_test {
        let fs_img_path = PathBuf::from(&manifest_dir).join("fs.img");
        let data_dir = project_root.join("data");
        // user_bin_dir 已经在上面通过 user_dir 引用了, user/bin
        let user_bin_dir = user_dir.join("bin");

        // 检查依赖
        let dependencies = vec![data_dir, user_bin_dir];

        if should_rebuild(&fs_img_path, &dependencies) {
            println!(
                "cargo:warning=[build.rs] Creating full ext4 runtime image (4GB) at fs.img..."
            );
            create_full_ext4_image(&fs_img_path, &project_root);
            println!(
                "cargo:warning=[build.rs] Runtime image created: {}",
                fs_img_path.display()
            );
        } else {
            println!("cargo:warning=[build.rs] fs.img is up to date, skipping regeneration.");
        }
    }
}

/// 检查目标文件是否需要重新构建
///
/// 如果目标不存在，或者任何依赖项(目录或文件)比目标新，则返回 true
fn should_rebuild(target: &Path, dependencies: &[PathBuf]) -> bool {
    if !target.exists() {
        return true;
    }

    let target_mtime = match fs::metadata(target).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return true, // 无法获取时间，为了安全起见重新构建
    };

    for dep in dependencies {
        if let Some(latest_dep_mtime) = get_latest_mtime(dep) {
            if latest_dep_mtime > target_mtime {
                return true;
            }
        } else {
            return true;
        }
    }

    false
}

/// 递归获取目录中最新的修改时间
fn get_latest_mtime(path: &Path) -> Option<std::time::SystemTime> {
    if !path.exists() {
        return None;
    }

    if path.is_file() {
        return fs::metadata(path).ok().and_then(|m| m.modified().ok());
    }

    let mut latest = None;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // 忽略隐藏文件 (.git 等)
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            let mtime = get_latest_mtime(&path);
            match (latest, mtime) {
                (None, Some(m)) => latest = Some(m),
                (Some(l), Some(m)) if m > l => latest = Some(m),
                _ => {}
            }
        }
    }
    latest
}

/// 创建空的 simple_fs 镜像
fn create_empty_image(path: &PathBuf) {
    // 空镜像格式: RAMDISK\0 + 0个文件 + 保留字段
    let empty_header: [u8; 16] = [
        b'R', b'A', b'M', b'D', b'I', b'S', b'K', 0, // 魔数
        0, 0, 0, 0, // 文件数量 = 0
        0, 0, 0, 0, // 保留
    ];

    if let Err(e) = fs::write(path, empty_header) {
        println!(
            "cargo:warning=[build.rs] Failed to create empty image: {}",
            e
        );
    } else {
        println!("cargo:warning=[build.rs] Created empty simple_fs image");
    }
}

/// 创建 ext4 测试镜像 (8MB)
fn create_ext4_test_image(path: &PathBuf) {
    create_empty_ext4_image(path, 8);
}

/// 创建最小 ext4 镜像 (1MB)
fn create_minimal_ext4_image(path: &PathBuf) {
    create_empty_ext4_image(path, 1);
}

/// 创建空的 ext4 镜像
fn create_empty_ext4_image(path: &PathBuf, size_mb: usize) {
    const BLOCK_SIZE: usize = 1024 * 1024;

    println!(
        "cargo:warning=[build.rs] Creating {}MB ext4 image at {}",
        size_mb,
        path.display()
    );

    // 1. 创建空文件 (dd)
    let dd_status = Command::new("dd")
        .arg("if=/dev/zero")
        .arg(format!("of={}", path.display()))
        .arg(format!("bs={}", BLOCK_SIZE))
        .arg(format!("count={}", size_mb))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute dd");

    if !dd_status.success() {
        panic!("Failed to create empty disk image");
    }

    // 2. 格式化为 ext4
    // 关键: 使用 -g 512 强制生成多个块组以避免 ext4_rs bug
    // 每组 512 块 (2MB), 8MB 镜像 = 4 个块组
    let mut mkfs_cmd = Command::new("mkfs.ext4");
    mkfs_cmd
        .arg("-F") // 强制覆盖
        .arg("-b")
        .arg("4096") // 块大小 4K
        .arg("-m")
        .arg("0") // 0% 保留空间
        .arg("-I")
        .arg("256"); // Inode 大小 256 字节

    // 只对测试镜像 (>=8MB) 添加 -g 选项以生成多个块组
    if size_mb >= 8 {
        mkfs_cmd.arg("-g").arg("512"); // 每组 512 块 (2MB)
    }

    let mkfs_status = mkfs_cmd
        .arg("-O")
        .arg("64bit,^has_journal,^resize_inode,^dir_index,^metadata_csum")
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute mkfs.ext4");

    if mkfs_status.success() {
        let block_groups = if size_mb >= 8 { " (multi-group)" } else { "" };
        println!(
            "cargo:warning=[build.rs] Ext4 image formatted successfully ({}MB{}).",
            size_mb, block_groups
        );
    } else {
        panic!("Failed to format ext4 image! Make sure 'mkfs.ext4' is installed.");
    }
}

/// 创建完整的 ext4 镜像 (包含 data/ 和 user/bin/)
fn create_full_ext4_image(path: &PathBuf, project_root: &Path) {
    const IMG_SIZE_MB: usize = 4096; // 4GB
    const BLOCK_SIZE: usize = 1024 * 1024;

    println!(
        "cargo:warning=[build.rs] Creating {}MB (4GB) full ext4 image at {}",
        IMG_SIZE_MB,
        path.display()
    );

    // 1. 创建临时目录用于组织文件系统内容
    let temp_root = std::env::temp_dir().join("comix_fs_content");
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root).ok();
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp directory");

    // 2. 复制 data/ 目录的内容到临时根目录
    let data_dir = project_root.join("data");
    if data_dir.exists() {
        copy_dir_recursive(&data_dir, &temp_root).expect("Failed to copy data directory");
        println!("cargo:warning=[build.rs] Copied data/ to temp root");
    }

    // 3. 创建 /home/user/bin 目录并复制 user/bin
    let home_user_bin = temp_root.join("home").join("user").join("bin");
    fs::create_dir_all(&home_user_bin).expect("Failed to create home/user/bin");

    let user_bin_src = project_root.join("user").join("bin");
    if user_bin_src.exists() {
        copy_dir_recursive(&user_bin_src, &home_user_bin).expect("Failed to copy user/bin");
        println!("cargo:warning=[build.rs] Copied user/bin to /home/user/bin");
    }

    // 4. 创建空镜像
    let dd_status = Command::new("dd")
        .arg("if=/dev/zero")
        .arg(format!("of={}", path.display()))
        .arg(format!("bs={}", BLOCK_SIZE))
        .arg(format!("count={}", IMG_SIZE_MB))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute dd");

    if !dd_status.success() {
        panic!("Failed to create disk image");
    }

    // 5. 使用 mkfs.ext4 -d 选项从临时目录创建文件系统
    let mkfs_status = Command::new("mkfs.ext4")
        .arg("-F")
        .arg("-b")
        .arg("4096")
        .arg("-m")
        .arg("0")
        .arg("-d")
        .arg(&temp_root) // 使用临时目录作为根
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute mkfs.ext4");

    if !mkfs_status.success() {
        panic!("Failed to format ext4 image with data!");
    }

    // 6. 清理临时目录
    fs::remove_dir_all(&temp_root).ok();

    println!("cargo:warning=[build.rs] Full ext4 image created successfully (1GB).");
}

/// 递归复制目录
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
