# 目标 1: run (默认启动 QEMU)
run:
	@echo "Running $(PROJECT_DIR)..."
ifeq ($(TEST), 1)
	@echo "TEST=1 detected. Running tests instead of 'cargo run'."
	@cargo test
else
	@cargo run
endif

# 目标 2: test (显式运行测试)
test:
	@cargo test 

.PHONY: run test