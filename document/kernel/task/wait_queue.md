# 等待队列设计

## 当前状态

`WaitQueue` 是事件等待队列, 用于把任务从运行队列移出并在事件满足时重新唤醒.它内部用 `TaskQueue` 保存等待者, 用 `RawSpinLock` 保护队列.

等待队列不拥有任务生命周期.它只保存 `SharedTask` 引用, 状态转换仍委托给调度器.

## 目标和非目标

目标:

- 为锁,定时器,wait 子进程等事件等待提供统一阻塞容器.
- 在唤醒时先从等待队列移除, 再调用调度器唤醒.
- 支持一次唤醒一个或全部等待者.

非目标:

- 不表达等待条件本身; 条件由调用方维护.
- 不负责进程退出码,信号递送或资源回收.
- 不在持有内部队列锁时执行复杂唤醒工作.

## 关键流程

### 睡眠

调用方把当前任务加入等待队列, 然后调用调度器将其置为 `Interruptible` 或 `Uninterruptible` 并从 run queue 移除.该操作本身不必立即切换 CPU, 调用方应在合适位置进入调度.

### 唤醒

唤醒路径先在等待队列锁内取出任务, 释放锁后调用 `wake_up_task`.这样可以避免 `WaitQueue -> Scheduler -> Task` 的锁链在等待队列内部长期持有.

### lost wakeup 防护

需要"检查条件并睡眠"的场景应使用原子 prepare 风格接口, 在同一临界区内完成条件检查,入队和状态转换, 避免事件在检查后,睡眠前到达.

## 并发和生命周期约束

- 等待队列锁只保护等待者列表, 不保护业务条件.
- 唤醒应在释放等待队列锁后进行.
- 重复唤醒由调度器的 `Running` 状态检查兜底, 但调用方仍应尽量维护清晰的事件状态.

## 源码索引

- `os/src/kernel/scheduler/wait_queue.rs`: `WaitQueue` 实现.
- `os/src/kernel/scheduler/mod.rs`: `sleep_task`,`wake_up_task`,`sleep_task_prepare`.
- `os/src/kernel/scheduler/task_queue.rs`: 等待队列复用的任务队列容器.
