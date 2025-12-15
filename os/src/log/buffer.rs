//! 锁无关的日志存储环形缓冲区
//!
//! 该模块实现了高性能、多生产者单消费者 (MPSC) 环形缓冲区，
//! 使用原子操作进行同步。

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

use super::config::GLOBAL_LOG_BUFFER_SIZE;
use super::entry::LogEntry;

/// 单个日志条目的大小（以字节为单位）
const LOG_ENTRY_SIZE: usize = core::mem::size_of::<LogEntry>();

/// 缓冲区中可存储的最大日志条目数
pub(crate) const MAX_LOG_ENTRIES: usize = GLOBAL_LOG_BUFFER_SIZE / LOG_ENTRY_SIZE;

/// 计算日志条目格式化后的精确字节长度
///
/// 格式: "{color_code}{level} [{timestamp:12}] [CPU{cpu_id}/T{task_id:3}] {message}{reset}\n"
///
/// **重要**: 此函数的计算逻辑必须与以下格式化函数保持一致：
/// - `log_core::direct_print_entry` - 控制台输出格式
/// - `log_core::format_log_entry` - syslog 系统调用格式
///
/// 如果修改了日志输出格式，需要同步更新三处：
/// 1. `log_core::direct_print_entry` - 实际格式化输出
/// 2. `log_core::format_log_entry` - 字符串格式化
/// 3. `calculate_formatted_length` (此函数) - 字节长度计算
///
/// # 组成部分计算
/// - ANSI 颜色代码: entry.level().color_code().len() (开始)
/// - ANSI 重置代码: entry.level().reset_color_code().len() (结束)
/// - 级别标签: entry.level().as_str().len() (例如 "\\[INFO\\]")
/// - 时间戳: 14 字节 (" [" + 12位数字 + "]")
/// - CPU ID: " [CPU" + digit_count(cpu_id)
/// - 任务 ID: "/T" + digit_count_padded(task_id, 3) + "]"
/// - 消息内容: entry.message().len()
/// - 空格分隔: 3 字节 (level后1个, timestamp后1个, context后1个)
/// - 换行: 1 字节
fn calculate_formatted_length(entry: &LogEntry) -> usize {
    // ANSI 颜色代码长度
    let color_start_len = entry.level().color_code().len();
    let color_reset_len = entry.level().reset_color_code().len();

    // 级别标签长度
    let level_len = entry.level().as_str().len();

    // 时间戳: " [{:12}]" = 2 + 12 = 14 字节
    let timestamp_len = 14;

    // CPU ID 的数字位数
    let cpu_id = entry.cpu_id();
    let cpu_digits = if cpu_id == 0 {
        1
    } else {
        // 计算十进制位数: floor(log10(n)) + 1
        let mut n = cpu_id;
        let mut digits = 0;
        while n > 0 {
            digits += 1;
            n /= 10;
        }
        digits
    };

    // 任务 ID 的数字位数（至少3位，有padding）
    let task_id = entry.task_id();
    let task_digits = if task_id == 0 {
        3 // "  0" 三位
    } else {
        let mut n = task_id;
        let mut digits = 0;
        while n > 0 {
            digits += 1;
            n /= 10;
        }
        if digits < 3 {
            3 // padding 到至少3位
        } else {
            digits
        }
    };

    // " [CPU<digits>/T<digits>]"
    // " [CPU" = 5, "/T" = 2, "]" = 1
    let context_len = 5 + cpu_digits + 2 + task_digits + 1;

    // 消息内容长度
    let message_len = entry.message().len();

    // 分隔符和换行: level后1个空格 + timestamp后1个空格 + context后1个空格 = 3
    let separators_len = 3;

    color_start_len
        + level_len
        + timestamp_len
        + context_len
        + message_len
        + color_reset_len
        + separators_len
}

/// 缓存行填充封装器，用于防止伪共享
///
/// 将封装的类型填充到 64 字节（典型的缓存行大小），以确保
/// 不同 CPU 核心使用的不同原子变量不会共享缓存行，
/// 从而避免因缓存一致性流量导致的性能下降。
#[repr(C, align(64))]
struct CachePadded64<T> {
    inner: T,
}

impl<T> Deref for CachePadded64<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for CachePadded64<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// 全局日志缓冲区实例
///
/// 我们不需要锁 (如 Mutex) 或延迟初始化 (如 OnceCell)，因为：
///
/// 1.  **不需要 `Mutex` (锁):** `GlobalLogBuffer` 本身是线程安全的。
///     它使用 `AtomicUsize` 字段以**内部可变性**设计。
///     由于其所有方法 (`write`, `read`) 都对共享引用 (`&self`) 操作，
///     并且所有内部修改都通过原子操作安全处理，因此整个
///     结构体是 `Sync`。我们不需要将一个已经线程安全的
///     类型封装在**另一个**锁中。
///
/// 2.  **不需要 `Lazy` 或 `OnceCell`:** `GlobalLogBuffer::new()`
///     函数是一个 `const fn`。这意味着它在**编译时**执行，
///     而不是在运行时执行。整个、完全初始化的 `GlobalLogBuffer`
///     实例在内核编译时被直接烘焙到内核的数据段 (`.data` 或 `.bss`) 中。
///
///     因此，没有运行时初始化步骤，也就不存在 `Lazy` 旨在解决的
///     "首次初始化"竞态条件。缓冲区从第一条 CPU 指令开始，
///     就已完全初始化并存在于内存中。
///
/// 这种模式产生了零开销、锁无关且无数据竞争的
/// 全局静态实例。
static GLOBAL_LOG_BUFFER: GlobalLogBuffer = GlobalLogBuffer::new();

/// 存储日志条目的锁无关环形缓冲区
///
/// 采用多生产者单消费者 (MPSC) 设计，其中：
/// - 多个 CPU 可以并发地写入日志而无需阻塞
/// - 单个消费者线程按顺序读取日志
#[repr(C)]
pub(super) struct GlobalLogBuffer {
    /// 写入侧数据（由生产者更新）
    writer_data: CachePadded64<WriterData>,
    /// 读取侧数据（由消费者更新）
    reader_data: CachePadded64<ReaderData>,
    /// 固定大小的日志条目数组
    buffer: [LogEntry; MAX_LOG_ENTRIES],
    /// 记录未读日志的总字节数
    unread_bytes: AtomicUsize,
}

/// 写入侧同步数据
#[repr(C)]
struct WriterData {
    /// 写入操作的单调递增序列号
    write_seq: AtomicUsize,
}

/// 读取侧同步数据
#[repr(C)]
struct ReaderData {
    /// 读取操作的单调递增序列号
    read_seq: AtomicUsize,
    /// 由于缓冲区溢出而丢弃的日志计数
    dropped: AtomicUsize,
}

impl GlobalLogBuffer {
    /// 在编译时创建一个新的全局日志缓冲区
    pub(super) const fn new() -> Self {
        const EMPTY: LogEntry = LogEntry::empty();
        Self {
            writer_data: CachePadded64 {
                inner: WriterData {
                    write_seq: AtomicUsize::new(1),
                },
            },
            reader_data: CachePadded64 {
                inner: ReaderData {
                    read_seq: AtomicUsize::new(1),
                    dropped: AtomicUsize::new(0),
                },
            },
            buffer: [EMPTY; MAX_LOG_ENTRIES],
            unread_bytes: AtomicUsize::new(0),
        }
    }

    /// 将日志条目写入缓冲区
    ///
    /// 这是一个**锁无关**操作，执行以下步骤：
    /// 1. 原子地获取一个唯一的序列号（票据）
    /// 2. 使用模运算计算目标槽位索引
    /// 3. 检查并处理潜在的缓冲区满（覆盖）逻辑
    /// 4. 将日志数据复制到槽位（*不包括* seq 字段）
    /// 5. 使用 **Release** 内存屏障原子地设置 seq 来发布条目
    /// 6. 增加未读字节计数
    pub(super) fn write(&self, entry: &LogEntry) {
        // step1: 原子地获取一个唯一的序列号（票据）
        let seq = self.writer_data.write_seq.fetch_add(1, Ordering::Relaxed);

        // step2: 从序列号计算目标槽位索引
        let slot = seq % MAX_LOG_ENTRIES;
        let slot_ptr = unsafe { self.buffer.as_ptr().add(slot) as *mut LogEntry };

        // step3: 检查并处理潜在的缓冲区满（覆盖）逻辑
        self.handle_overwrite(seq);

        // step4: 将所有日志数据（*不包括* seq 字段）复制到槽位
        unsafe {
            entry.copy_data_to(slot_ptr);
        }

        // step5: 通过原子地设置其 seq 来发布条目（Release 屏障）
        unsafe {
            entry.publish(slot_ptr, seq);
        }

        // step6: 增加未读字节计数
        let formatted_len = calculate_formatted_length(entry);
        self.unread_bytes
            .fetch_add(formatted_len, Ordering::Release);
    }

    /// 处理缓冲区溢出，必要时推进读取指针
    ///
    /// 当缓冲区满且新的写入将覆盖未读条目时，此函数：
    /// 1. 检测溢出条件
    /// 2. 计算将被覆盖的条目数
    /// 3. 更新丢弃计数
    /// 4. 使用 CAS 循环原子地推进读取指针
    fn handle_overwrite(&self, current_seq: usize) {
        let read_seq = self.reader_data.read_seq.load(Ordering::Acquire);
        if current_seq < read_seq + MAX_LOG_ENTRIES {
            return;
        }
        let new_read_seq = current_seq - MAX_LOG_ENTRIES + 1;
        let overwritten = new_read_seq.saturating_sub(read_seq);
        self.reader_data
            .dropped
            .fetch_add(overwritten, Ordering::Relaxed);

        // CAS 循环以推进 read_seq
        let mut current_read_seq = read_seq;
        while current_read_seq < new_read_seq {
            match self.reader_data.read_seq.compare_exchange_weak(
                current_read_seq,
                new_read_seq,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(seen_seq) => {
                    if seen_seq >= new_read_seq {
                        break;
                    }
                    current_read_seq = seen_seq;
                }
            }
        }
    }

    /// 从缓冲区读取下一个日志条目
    ///
    /// 如果没有可用条目，则返回 `None`。这是一个**锁无关**的
    /// 单消费者操作，使用 **Acquire** 内存顺序确保与生产者的正确同步。
    /// 读取后会减少未读字节计数。
    pub(super) fn read(&self) -> Option<LogEntry> {
        let read_seq = self.reader_data.read_seq.load(Ordering::Acquire);

        let slot = read_seq % MAX_LOG_ENTRIES;
        let slot_ptr = unsafe { self.buffer.as_ptr().add(slot) as *const LogEntry };

        const EMPTY: LogEntry = LogEntry::empty();
        unsafe {
            if !EMPTY.is_ready(slot_ptr, read_seq) {
                return None;
            }
        }

        let entry_data = unsafe { (*slot_ptr).clone() };

        // 减少未读字节计数
        let formatted_len = calculate_formatted_length(&entry_data);
        self.unread_bytes
            .fetch_sub(formatted_len, Ordering::Release);

        self.reader_data
            .read_seq
            .store(read_seq + 1, Ordering::Release);

        Some(entry_data)
    }

    /// 返回缓冲区中未读日志条目的数量
    pub(super) fn len(&self) -> usize {
        let write = self.writer_data.write_seq.load(Ordering::Relaxed);
        let read = self.reader_data.read_seq.load(Ordering::Relaxed);
        write.saturating_sub(read)
    }

    /// 返回未读日志的总字节数（格式化后）
    pub(super) fn unread_bytes(&self) -> usize {
        self.unread_bytes.load(Ordering::Acquire)
    }

    /// 返回由于缓冲区溢出而丢弃的日志总数
    pub(super) fn dropped_count(&self) -> usize {
        self.reader_data.dropped.load(Ordering::Relaxed)
    }

    /// 非破坏性读取：按索引 peek 日志条目，不移动读指针
    ///
    /// 此方法允许读取缓冲区中的日志而不删除它们，主要用于
    /// `SyslogAction::ReadAll` 操作。
    ///
    /// # 参数
    /// * `index` - 全局序列号（从 1 开始，与内部 seq 对应）
    ///
    /// # 返回值
    /// * `Some(LogEntry)` - 如果条目存在且有效
    /// * `None` - 如果索引超出范围或条目已被覆盖
    ///
    /// # 并发安全
    /// 此方法是完全无锁的，可以与 write 和 read 并发调用。
    pub(super) fn peek(&self, index: usize) -> Option<LogEntry> {
        let current_write = self.writer_data.write_seq.load(Ordering::Acquire);
        let current_read = self.reader_data.read_seq.load(Ordering::Acquire);

        // 检查索引是否在有效范围内
        // 有效范围: [current_read, current_write)
        if index < current_read || index >= current_write {
            return None;
        }

        // 检查是否已被覆盖（环形缓冲区溢出）
        // 如果 writer 已经超过 reader + BUFFER_SIZE，则旧数据可能被覆盖
        if current_write >= current_read + MAX_LOG_ENTRIES {
            // 缓冲区已满，旧数据可能被覆盖
            let oldest_valid = current_write.saturating_sub(MAX_LOG_ENTRIES);
            if index < oldest_valid {
                return None; // 数据已被覆盖
            }
        }

        // 计算缓冲区索引
        let slot = index % MAX_LOG_ENTRIES;
        let slot_ptr = unsafe { self.buffer.as_ptr().add(slot) as *const LogEntry };

        // 检查序列号是否匹配（确保数据有效）
        const EMPTY: LogEntry = LogEntry::empty();
        unsafe {
            if !EMPTY.is_ready(slot_ptr, index) {
                return None;
            }
        }

        // 克隆并返回条目
        Some(unsafe { (*slot_ptr).clone() })
    }

    /// 获取当前可读取的起始索引（读指针位置）
    pub(super) fn reader_index(&self) -> usize {
        self.reader_data.read_seq.load(Ordering::Acquire)
    }

    /// 获取当前写入位置（下一个要写入的索引）
    pub(super) fn writer_index(&self) -> usize {
        self.writer_data.write_seq.load(Ordering::Acquire)
    }
}

/// 将日志条目写入全局缓冲区（内部使用）
#[inline]
pub(super) fn write_log(entry: &LogEntry) {
    GLOBAL_LOG_BUFFER.write(entry);
}

/// 从全局缓冲区读取下一个日志条目
///
/// 如果没有可供读取的条目，则返回 `None`。
#[inline]
pub fn read_log() -> Option<LogEntry> {
    GLOBAL_LOG_BUFFER.read()
}

/// 返回由于缓冲区溢出而丢弃的日志总数
#[inline]
pub fn log_dropped_count() -> usize {
    GLOBAL_LOG_BUFFER.dropped_count()
}

/// 返回缓冲区中当前未读日志条目的数量
#[inline]
pub fn log_len() -> usize {
    GLOBAL_LOG_BUFFER.len()
}

/// 返回未读日志的总字节数（格式化后）
#[inline]
pub fn log_unread_bytes() -> usize {
    GLOBAL_LOG_BUFFER.unread_bytes()
}

/// 非破坏性读取：按索引 peek 日志条目
///
/// 不移动读指针，允许重复读取同一条目。
#[inline]
pub fn peek_log(index: usize) -> Option<LogEntry> {
    GLOBAL_LOG_BUFFER.peek(index)
}

/// 获取当前可读取的起始索引
#[inline]
pub fn log_reader_index() -> usize {
    GLOBAL_LOG_BUFFER.reader_index()
}

/// 获取当前写入位置
#[inline]
pub fn log_writer_index() -> usize {
    GLOBAL_LOG_BUFFER.writer_index()
}
