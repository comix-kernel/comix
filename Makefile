DOCKER_TAG ?= comix:latest
.PHONY: docker build_docker fmt run build clean clean-all

docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt --name comix ${DOCKER_TAG} bash

build_docker:
	docker build -t ${DOCKER_TAG} --target build .

fmt:
	cd os && cargo fmt

# 构建内核（build.rs 会自动编译 user 并打包镜像）
build:
	cd os && cargo build

# 运行内核（build.rs 会自动编译 user 并打包镜像）
run:
	cd os && cargo run

# 清理 OS 构建产物
clean:
	cd os && cargo clean

# 清理所有构建产物（包括 user）
clean-all: clean
	cd user && make clean

# 手动编译用户程序（通常不需要，build.rs 会自动处理）
build-user:
	cd user && make

# 手动打包镜像（通常不需要，build.rs 会自动处理）
pack-simple-fs: build-user
	@echo "Packing simple_fs..."
	python3 scripts/make_init_simple_fs.py user/bin os/simple_fs.img

# 检查镜像内容
inspect-simple-fs:
	@IMG=$$(find os/target -name "simple_fs.img" -type f 2>/dev/null | head -1); \
	if [ -n "$$IMG" ]; then \
		python3 scripts/make_init_simple_fs.py --inspect $$IMG; \
	else \
		echo "Error: simple_fs.img not found. Run 'make build' first."; \
		exit 1; \
	fi