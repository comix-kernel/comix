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

    // 步骤 1: 编译用户程序
    println!("cargo:warning=[build.rs] Building user programs...");

    if user_dir.exists() {
        let status = Command::new("make")
            .current_dir(&user_dir)
            .env("BUILD_MODE", "release")
            .stdout(std::process::Stdio::null())  // 抑制 make 输出
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=[build.rs] User programs built successfully");
            }
            Ok(s) => {
                println!("cargo:warning=[build.rs] User build failed with status: {}", s);
                // 继续构建，使用已有的 bin/ 内容
            }
            Err(e) => {
                println!("cargo:warning=[build.rs] Failed to run make in user/: {}", e);
                // 继续构建
            }
        }
    } else {
        println!("cargo:warning=[build.rs] User directory not found, skipping user build");
    }

    // 步骤 2: 打包 simple_fs 镜像
    println!("cargo:warning=[build.rs] Packing simple_fs image...");

    let status = Command::new("python3")
        .arg(&tool_path)
        .arg(&user_bin_dir)
        .arg(&img_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            let img_size = fs::metadata(&img_path)
                .map(|m| m.len())
                .unwrap_or(0);
            println!("cargo:warning=[build.rs] Simple_fs image created: {} bytes", img_size);
        }
        Ok(s) => {
            println!("cargo:warning=[build.rs] Failed to pack simple_fs: status {}", s);
            // 创建空镜像以避免编译失败
            create_empty_image(&img_path);
        }
        Err(e) => {
            println!("cargo:warning=[build.rs] Failed to run make_init_simple_fs.py: {}", e);
            create_empty_image(&img_path);
        }
    }

    // 验证镜像文件存在
    if !img_path.exists() {
        println!("cargo:warning=[build.rs] Image not found, creating empty image");
        create_empty_image(&img_path);
    }

    // 输出镜像路径供代码使用
    println!("cargo:rustc-env=SIMPLE_FS_IMAGE={}", img_path.display());
}

/// 创建空的 simple_fs 镜像
fn create_empty_image(path: &PathBuf) {
    // 空镜像格式: RAMDISK\0 + 0个文件 + 保留字段
    let empty_header: [u8; 16] = [
        b'R', b'A', b'M', b'D', b'I', b'S', b'K', 0,  // 魔数
        0, 0, 0, 0,  // 文件数量 = 0
        0, 0, 0, 0,  // 保留
    ];

    if let Err(e) = fs::write(path, &empty_header) {
        println!("cargo:warning=[build.rs] Failed to create empty image: {}", e);
    } else {
        println!("cargo:warning=[build.rs] Created empty simple_fs image");
    }
}
