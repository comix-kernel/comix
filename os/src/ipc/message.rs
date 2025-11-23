//! 进程间消息模块

use core::sync::atomic::Ordering;

use alloc::{collections::vec_deque::VecDeque, vec::Vec};

use crate::{
    kernel::{WaitQueue, current_cpu, current_task, yield_task},
    sync::{Mutex, SpinLock},
};

/// 最大队列容量（字节）
const DEFAULT_QUEUE_BYTES: usize = 1024;

/// 进程间消息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// 消息类型
    pub mtype: i32,
    /// 消息大小
    pub msize: usize,
    /// 消息内容
    pub mtext: Vec<u8>,
}

impl Message {
    /// 创建一个新的进程间消息
    pub fn new(mtype: i32, mtext: Vec<u8>) -> Self {
        let msize = mtext.len();
        Message {
            mtype,
            msize,
            mtext,
        }
    }
}

/// 内部状态：受 Mutex 保护
struct QueueState {
    messages: VecDeque<Message>,
    bytes: usize,
}

impl QueueState {
    fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            bytes: 0,
        }
    }
}

/// 进程间消息队列（支持阻塞的发送/接收）
pub struct MessageQueue {
    /// 队列状态：消息与当前字节数
    state: Mutex<QueueState>,
    /// 等待发送（队列满）的进程
    send_waiters: SpinLock<WaitQueue>,
    /// 等待接收（队列空）的进程
    recv_waiters: SpinLock<WaitQueue>,
    /// 队列的最大容量（字节）
    max_bytes: usize,
}

impl MessageQueue {
    /// 创建一个新的进程间消息队列
    pub fn new() -> Self {
        MessageQueue {
            state: Mutex::new(QueueState::new()),
            send_waiters: SpinLock::new(WaitQueue::new()),
            recv_waiters: SpinLock::new(WaitQueue::new()),
            max_bytes: DEFAULT_QUEUE_BYTES,
        }
    }

    /// 发送消息（阻塞直到有空间）
    pub fn send(&self, msg: Message) {
        let size = msg.msize;
        let mut pending = Some(msg); // 保存消息所有权，直到真正 push 时再取走
        loop {
            let mut st = self.state.lock();
            if st.bytes + size <= self.max_bytes {
                let m = pending.take().expect("message already taken");
                st.bytes += size;
                st.messages.push_back(m);
                self.recv_waiters.lock().wake_up_all();
                return;
            }

            // XXX: 是否有丢失唤醒风险
            drop(st);
            self.send_waiters.lock().sleep(current_task());
            // 被唤醒后重试
        }
    }

    /// 接收任意类型的消息（阻塞直到有消息）
    pub fn recv(&self) -> Message {
        loop {
            if let Some(msg) = {
                let mut st = self.state.lock();
                match st.messages.pop_front() {
                    Some(m) => {
                        st.bytes -= m.msize;
                        Some(m)
                    }
                    None => None,
                }
            } {
                self.send_waiters.lock().wake_up_all();
                return msg;
            }

            self.recv_waiters.lock().sleep(current_task());
        }
    }

    /// 按类型接收（阻塞直到有匹配类型）
    pub fn recv_by_type(&self, mtype: i32) -> Message {
        loop {
            if let Some(msg) = {
                let mut st = self.state.lock();
                if let Some(i) = st.messages.iter().position(|m| m.mtype == mtype) {
                    let m = st.messages.remove(i).unwrap();
                    st.bytes -= m.msize;
                    Some(m)
                } else {
                    None
                }
            } {
                self.send_waiters.lock().wake_up_all();
                return msg;
            }

            self.recv_waiters.lock().sleep(current_task());
        }
    }

    /// 非阻塞尝试：返回是否成功发送
    pub fn try_send(&self, msg: Message) -> bool {
        let mut st = self.state.lock();
        if st.bytes + msg.msize <= self.max_bytes {
            st.bytes += msg.msize;
            st.messages.push_back(msg);
            drop(st);
            self.recv_waiters.lock().wake_up_all();
            true
        } else {
            false
        }
    }

    /// 非阻塞尝试接收
    pub fn try_recv(&self) -> Option<Message> {
        let mut st = self.state.lock();
        let msg = st.messages.pop_front()?;
        st.bytes -= msg.msize;
        drop(st);
        self.send_waiters.lock().wake_up_all();
        Some(msg)
    }

    /// 非阻塞按类型接收
    pub fn try_recv_by_type(&self, mtype: i32) -> Option<Message> {
        let mut st = self.state.lock();
        let ix = st.messages.iter().position(|m| m.mtype == mtype)?;
        let msg = st.messages.remove(ix).unwrap();
        st.bytes -= msg.msize;
        drop(st);
        self.send_waiters.lock().wake_up_all();
        Some(msg)
    }

    /// 查询当前已用字节数
    pub fn used_bytes(&self) -> usize {
        self.state.lock().bytes
    }

    /// 设置最大容量
    pub fn set_max_bytes(&mut self, max: usize) {
        let mut st = self.state.lock();
        st.bytes = st.bytes.min(max);
        self.max_bytes = max;
        // 容量增大后唤醒可能阻塞的发送者
        drop(st);
        self.send_waiters.lock().wake_up_all();
    }
}
