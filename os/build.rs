//! Build script for the OS kernel
//!
//! This script automatically:
//! 1. Compiles user programs in ../user directory
//! 2. Packs them into an init_simple_fs image
//! 3. Embeds the image into the kernel binary

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // 获取环境变量
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    // 设置路径
    let project_root = PathBuf::from(&manifest_dir).parent().unwrap().to_path_buf();
    let user_dir = project_root.join("user");
    let user_bin_dir = user_dir.join("bin");
    let img_path = PathBuf::from(&out_dir).join("simple_fs.img");
    let tool_path = project_root.join("scripts").join("make_init_simple_fs.py");

    println!("cargo:rerun-if-changed=../user");
    println!("cargo:rerun-if-changed=../scripts/make_init_simple_fs.py");

    // 步骤 1: 编译用户程序 (暂时禁用)
    println!("cargo:warning=[build.rs] Skipping user programs build (disabled for now)...");

    // if user_dir.exists() {
    //     let status = Command::new("make")
    //         .current_dir(&user_dir)
    //         .env("BUILD_MODE", "release")
    //         // 清除可能从父目录继承的 CARGO 环境变量，避免用户程序继承 os 的构建配置
    //         .env_remove("CARGO_ENCODED_RUSTFLAGS")
    //         .env_remove("CARGO_BUILD_RUSTFLAGS")
    //         // .stdout(std::process::Stdio::null()) // 抑制 make 输出
    //         // .stderr(std::process::Stdio::null())
    //         .status();
    //
    //     match status {
    //         Ok(s) if s.success() => {
    //             println!("cargo:warning=[build.rs] User programs built successfully");
    //         }
    //         Ok(s) => {
    //             panic!(
    //                 "User program build failed with status: {}. Aborting kernel build.",
    //                 s
    //             );
    //         }
    //         Err(e) => {
    //             panic!(
    //                 "Failed to execute make for user programs: {}. Aborting kernel build.",
    //                 e
    //             );
    //         }
    //     }
    // } else {
    //     println!("cargo:warning=[build.rs] User directory not found, skipping user build");
    // }

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

    // 步骤 3: 创建 ext4 测试镜像
    println!("cargo:warning=[build.rs] Creating ext4 test image...");
    let ext4_img_path = PathBuf::from(&out_dir).join("ext4_test.img");
    create_ext4_image(&ext4_img_path);
    println!("cargo:rustc-env=EXT4_FS_IMAGE={}", ext4_img_path.display());
}

/// 创建空的 simple_fs 镜像
fn create_empty_image(path: &PathBuf) {
    // 空镜像格式: RAMDISK\0 + 0个文件 + 保留字段
    let empty_header: [u8; 16] = [
        b'R', b'A', b'M', b'D', b'I', b'S', b'K', 0, // 魔数
        0, 0, 0, 0, // 文件数量 = 0
        0, 0, 0, 0, // 保留
    ];

    if let Err(e) = fs::write(path, &empty_header) {
        println!(
            "cargo:warning=[build.rs] Failed to create empty image: {}",
            e
        );
    } else {
        println!("cargo:warning=[build.rs] Created empty simple_fs image");
    }
}

/// 创建 ext4 测试镜像
fn create_ext4_image(path: &PathBuf) {
    // 8MB 镜像
    const IMG_SIZE_MB: usize = 8;
    const BLOCK_SIZE: usize = 1024 * 1024;

    println!("cargo:warning=[build.rs] Creating {}MB ext4 image at {}", IMG_SIZE_MB, path.display());

    // 1. 创建空文件 (dd)
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
        panic!("Failed to create empty disk image");
    }

    // 2. 格式化为 ext4
    // 关键策略：通过 -g 512 强制生成多个块组 (Block Groups)
    // 8MB / 2MB(512*4k) = 4 个块组
    // 这样 ext4_rs 即使跳过 Group 0，也有 Group 1, 2, 3 可用
    let mkfs_status = Command::new("mkfs.ext4")
        .arg("-F")                  // 强制覆盖
        .arg("-b").arg("4096")      // 块大小 4K
        .arg("-g").arg("512")       // [关键!] 每组 512 块 (2MB)。确保 8MB 镜像有 4 个组。
        .arg("-m").arg("0")         // 0% 保留空间，最大化可用容量
        .arg("-I").arg("256")       // Inode 大小 256 字节
        
        // 特性控制 (Features):
        // ^has_journal:   [关键!] 禁用日志。小块组无法容纳日志，且 OS 开发初期建议无日志以简化。
        // ^resize_inode:  禁用在线调整大小预留，节省 GDT 空间。
        // ^metadata_csum: 禁用校验和，提高与旧版驱动/ext4_rs 的兼容性。
        // 64bit:          保持开启，现代 ext4 默认特性。
        .arg("-O").arg("64bit,^has_journal,^resize_inode,^dir_index,^metadata_csum")
        
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute mkfs.ext4");

    if mkfs_status.success() {
        println!("cargo:warning=[build.rs] Ext4 image formatted successfully (No Journal, Multi-Group).");
    } else {
        panic!("Failed to format ext4 image! Make sure 'mkfs.ext4' is installed.");
    }
}