# SMP 调试与回滚 Walkthrough（当前基线：保留 A/B/C/E）

本文记录在当前工作区的“分组回滚”后（保留锚点 A/B/C/E，回滚 D/F/G）的验证步骤、定位思路与快速对照点。目标是先恢复稳定，再按需增量恢复功能。

## 1. 快速构建与运行

```sh
make clean && make build
cd os && SMP=2 make run
```

运行成功的基本判据：可加载 /sbin/init、无 freeze/卡死、无 IPI 风暴日志、无用户态 Instruction Page Fault 或内核态 panic。

## 2. 锚点对照（期望状态与验证）

- A｜TID/Idle 策略（已保留）
  - 期望：init 固定为 PID/TID=1；分配器从 2 开始；idle 任务正常从分配器获取并加入 TASK_MANAGER。
  - 关键文件：
    - os/src/arch/riscv/boot/mod.rs:56（init=1）
    - os/src/arch/riscv/boot/mod.rs:359（create_idle_task 路径）
    - os/src/kernel/task/tid_allocator.rs:17（next_tid=2）
  - 验证：日志/调试输出中 init pid=1；idle 获得 TID 2/3/…；无 “PID must be 1” 类报错。

- B｜调度器临界区与窗口期重构（已保留）
  - 期望：尽量缩短 current_task=None 窗口，配合 PreemptGuard/关中断策略，避免竞态。
  - 关键文件：
    - os/src/kernel/scheduler/rr_scheduler.rs
    - os/src/kernel/scheduler/mod.rs
    - os/src/kernel/task/mod.rs（try_current_task() 存在并被调用）
  - 验证：无 “current_task: CPU has no current task” panic；调度切换平稳。

- C｜Trap 调度与异常路径（已保留）
  - 期望：仅在用户态路径触发调度；内核态 timer 中断不主动 schedule；异常打印保持必要级别。
  - 关键文件：
    - os/src/arch/riscv/trap/trap_handler.rs
  - 验证：用户态定时片到期时才 schedule；异常打印不过量、不递归进入 trap。

- E｜SMP 启动与 IPI/CPU 日志（已保留）
  - 期望：从核上线后进入 idle；仅在有就绪任务/重分配需求时发送 IPI；无 IPI 风暴。
  - 关键文件：
    - os/src/arch/riscv/boot/mod.rs（secondary_start/idle 初始化）
    - os/src/kernel/cpu.rs（IPI 路径与日志）
  - 验证：日志中 [IPI] 不高频刷屏；仅收到 Reschedule IPI 时发生切换。

## 3. 常见症状与快速定位

- 卡死/日志时间戳异常（“极大值”或停滞）
  - 排查：优先看 IPI 是否高频；若是，检查调度器 run queue 判定与 IPI 触发条件（E）。
- current_task 相关 panic
  - 排查：确认 try_current_task 使用点；检查 rr_scheduler take/switch 顺序与关中断范围（B）。
- 用户态 Instruction Page Fault（sepc/ra 落在栈页、X=0）
  - 说明：本轮已回滚信号跳板（D）。若仍发生，优先定位 syscall 返回路径是否写坏 TrapFrame（与信号无关）。
- IPI 风暴
  - 排查：确保仅在有就绪任务/跨核迁移时发送；空队列不应触发（E）。

## 4. 建议的增量恢复（当基线稳定后）

1) 恢复“用户态 sigreturn 跳板页 + RA/SP 修正”（锚点 D），一次性合并：
   - 在 install_user_signal_trap_frame 设置 `tf.x1_ra = USER_SIGTRAMP`、`sp = ucontext`；
   - 在 from_elf/地址空间构建时映射用户态 RX 跳板页并复制 `__sigreturn_trampoline` 指令；
   - 保持 rt_sigreturn 轻量日志以便一次性确认路径正确。
2) 若仍需更多诊断，再逐步恢复更细粒度日志（trap/页表遍历等），但避免在 trap 上下文做重操作。

## 5. 运行中快速检查命令

```sh
# 关键信息过滤
rg -n "\[IPI\]|UNEXPECTED TRAP|current_task|panic|Instruction Page Fault" os/smp.log

# 查看 TID 分配路径与 init=1 实现
rg -n "allocate_tid|init.*tid|create_idle_task" os/src
```

如需把“信号跳板回归（锚点 D）”写成补充章节，请告知，我会追加到本文件末尾并附上代码位置速查与验证要点。
