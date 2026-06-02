# Comix（OS 大赛提交版说明）

本 README 面向 OS 大赛评测环境（GitLab 仓库展示 / 评测机构建约束），与 GitHub 上的开发版说明不同。

## 上游仓库（主要开发在 GitHub）

本项目主要在 GitHub 上开发与维护：

- 上游仓库：`https://github.com/comix-kernel/comix`

此 GitLab 仓库由 GitHub Actions 自动镜像（含 Rust vendored 依赖等评测所需内容），仅用于提交与评测，请勿直接在此修改。

## 评测构建要求（必须）

- 评测机在项目根目录执行：`make all`
- `make all` 生成以下 ELF 内核：
  - `kernel-rv`：RISC-V 内核（`riscv64gc-unknown-none-elf`，release）
  - `kernel-la`：LoongArch 内核（`loongarch64-unknown-none`，release）
- 同时生成可选的 rootfs 镜像（评测启动 QEMU 时一并挂载）：
  - `disk.img`
  - `disk-la.img`

## 离线依赖 / 隐藏目录过滤（重要）

评测系统在 clone 时会过滤所有隐藏文件和目录（例如 `.cargo/`）。为此本仓库做了两件事：

1. `make all` 会在构建时重建 `os/.cargo/config.toml`（恢复链接脚本所需的 rustflags），即使 `.cargo/` 被评测机过滤也能正常构建。
2. 仓库内包含 Rust vendored 依赖以离线构建：
   - `os/vendor/`
   - `os/cargo-vendor-config.toml`

当 `os/cargo-vendor-config.toml` 存在时，`make all` 会自动将其追加到 `os/.cargo/config.toml`，使 Cargo 使用 vendored 源并以离线模式构建。

本项目**不使用 `build-std`**：两个目标的预编译 `rust-std` 已自带所需的 `mem` intrinsics，因此评测机只需具备对应 target 的预编译标准库，无需 `rust-src`。

## 工具链

- `os/rust-toolchain.toml` 固定为 `nightly-2025-10-28`。
