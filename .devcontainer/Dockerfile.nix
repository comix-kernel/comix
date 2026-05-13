FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# 使用清华源加速 apt
RUN sed -i 's/archive.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list.d/ubuntu.sources && \
    sed -i 's/security.ubuntu.com/mirrors.tuna.tsinghua.edu.cn/g' /etc/apt/sources.list.d/ubuntu.sources

# 安装最小依赖（git/curl/fish/sudo/make，其余全部由 nix develop 提供）
RUN apt-get update && apt-get install -y --no-install-recommends \
    git curl xz-utils ca-certificates sudo fish gnumake \
    && rm -rf /var/lib/apt/lists/*

# 删除初始 ubuntu 用户
RUN userdel -r ubuntu

# 新建非 root 用户 vscode
RUN useradd -m -s /bin/bash vscode && \
    echo "vscode ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# /nix 目录授权给 vscode（单用户模式需要）
RUN mkdir -m 0755 /nix && chown vscode /nix

# 切换 vscode 用户安装 Nix（单用户模式，无需 systemd）
USER vscode
WORKDIR /home/vscode

RUN curl --proto '=https' --tlsv1.2 -sSf -L https://nixos.org/nix/install | sh -s -- install linux --no-daemon --no-confirm

# 配置 PATH 以包含 Nix profile
ENV PATH="/home/vscode/.nix-profile/bin:${PATH}"

# 启用 flakes
RUN mkdir -p /home/vscode/.config/nix && \
    echo 'experimental-features = nix-command flakes' >> /home/vscode/.config/nix/nix.conf

CMD ["fish"]
