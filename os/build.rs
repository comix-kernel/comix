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
    // 修改为 8MB，给堆内存留出 8MB 的喘息空间
    const IMG_SIZE_MB: usize = 8;
    const BLOCK_SIZE: usize = 1024 * 1024;

    // 1. 创建 8MB 空文件 (确保清零)
    let _ = Command::new("dd")
        .arg("if=/dev/zero")
        .arg(format!("of={}", path.display()))
        .arg(format!("bs={}", BLOCK_SIZE))
        .arg(format!("count={}", IMG_SIZE_MB))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // 2. 格式化为 ext4
    // 目标：在 8MB 空间内构建 ext4_rs 能读懂的布局
    let mkfs_status = Command::new("mkfs.ext4")
        .arg("-F")
        .arg("-b").arg("4096")      // 必须: 4K 块
        .arg("-m").arg("0")         // 0% 保留空间
        .arg("-I").arg("256")       // 指定 inode 大小 (标准值)
        
        // 尝试强制创建一个最小的 2MB 日志
        // 如果 mkfs 报错说空间不足，可能需要去掉这行（但那样会触发 ext4_rs 的无日志 bug）
        .arg("-J").arg("size=2")    
        
        // 核心特性控制：
        // 64bit: 必须开启！修复 ENOSPC 的关键
        // ^resize_inode: 必须禁用！节省 GDT 空间，避免布局混乱
        // ^dir_index, ^metadata_csum: 兼容性禁用
        .arg("-O").arg("64bit,^resize_inode,^dir_index,^metadata_csum")
        
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match mkfs_status {
        Ok(s) if s.success() => {
            println!("cargo:warning=[build.rs] 8MB Ext4 image created successfully.");
        }
        _ => {
            println!("cargo:warning=[build.rs] mkfs failed! trying without journal...");
            // 如果 8MB 塞不下日志，尝试无日志模式作为最后的退路
            let _ = Command::new("mkfs.ext4")
                .arg("-F")
                .arg("-b").arg("4096")
                .arg("-m").arg("0")
                .arg("-O").arg("64bit,^has_journal,^resize_inode,^dir_index,^metadata_csum")
                .arg(path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}
