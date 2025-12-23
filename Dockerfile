FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# 使用清华源加速 apt
RUN sed -i 's/archive.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list.d/ubuntu.sources && \
    sed -i 's/security.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list.d/ubuntu.sources

# 安装基础开发工具和依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    git build-essential curl wget python3 python3-pip ninja-build cmake \
    autoconf automake autotools-dev bison flex texinfo gperf libtool patchutils \
    libtool-bin help2man gawk \
    pkg-config libglib2.0-dev libpixman-1-dev libsdl2-dev libslirp-dev \
    libncurses5-dev libreadline-dev libssl-dev sudo tmux unzip xz-utils ca-certificates \
    openssl openssh-client \
    gcc-riscv64-linux-gnu g++-riscv64-linux-gnu \
    libgmp-dev libexpat1-dev fish libmpfr-dev libmpc-dev \
    lldb \
    && rm -rf /var/lib/apt/lists/*

# 安装 Python tomli（QEMU 编译需求）
RUN pip3 install tomli --break-system-packages

# 删除 初始ubuntu 用户
RUN userdel -r ubuntu

# 新建非 root 用户 vscode
RUN useradd -m -s /bin/bash vscode && \
    echo "vscode ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# 安装 Rust nightly 工具链并设为默认（作为 vscode 用户）
USER vscode
ENV CARGO_HOME=/home/vscode/.cargo
ENV RUSTUP_HOME=/home/vscode/.rustup
ENV PATH="${CARGO_HOME}/bin:${PATH}"

# 先安装 rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain none

# 然后安装指定版本的 nightly
RUN rustup toolchain install nightly-2025-10-28 && \
    rustup default nightly-2025-10-28 && \
    rustup component add rustfmt clippy rust-src rust-analyzer llvm-tools && \
    rustup target add \
        riscv64gc-unknown-none-elf \
        riscv64imac-unknown-none-elf \
        loongarch64-unknown-linux-gnu \
        loongarch64-unknown-none \
        x86_64-unknown-linux-gnu
        
# 配置 cargo 使用 sparse index
RUN mkdir -p ${CARGO_HOME} && \
    echo '[source.crates-io]' > ${CARGO_HOME}/config.toml && \
    echo 'registry = "sparse+https://index.crates.io/"' >> ${CARGO_HOME}/config.toml

# 配置 cargo binutils
RUN cargo install cargo-binutils 

# 切回 root 编译 QEMU 与 GDB
USER root
WORKDIR /root

# 编译安装 QEMU 9.2.1
RUN wget https://download.qemu.org/qemu-9.2.1.tar.xz && \
    tar -xf qemu-9.2.1.tar.xz && cd qemu-9.2.1 && \
    ./configure --target-list=riscv64-softmmu,riscv64-linux-user,loongarch64-softmmu,loongarch64-linux-user \
        --enable-sdl --enable-slirp && \
    make -j$(nproc) && make install && \
    cd .. && rm -rf qemu-9.2.1*

# 编译安装 GDB 13.1
RUN wget https://mirrors.tuna.tsinghua.edu.cn/gnu/gdb/gdb-13.1.tar.xz && \
    tar -xf gdb-13.1.tar.xz

WORKDIR /root/gdb-13.1
RUN mkdir build-loongarch64 && cd build-loongarch64 && \
    ../configure --prefix=/usr/local --target=loongarch64-unknown-elf --enable-tui=yes && \
    make -j$(nproc) && make install

WORKDIR /root/gdb-13.1
RUN mkdir build-riscv64 && cd build-riscv64 && \
    ../configure --prefix=/usr/local --target=riscv64-unknown-elf --enable-tui=yes && \
    make -j$(nproc) && make install

# 设置 vscode 用户工作目录
USER vscode
WORKDIR /workspace
SHELL ["/bin/fish", "-c"]
CMD ["fish"]
