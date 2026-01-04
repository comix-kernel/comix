# iperf3 系统调用追踪分析

## 测试环境
- 工具: QEMU RISC-V 用户模式 (qemu-riscv64 -strace)
- 程序: iperf3 (TCP 性能测试)
- 测试: 客户端连接到 127.0.0.1:5001，传输 1 秒

## 关键系统调用序列

### 服务器端 (Server)

1. **初始化监听 socket**
   - socket(10, SOCK_STREAM, IPPROTO_IP) = 3  // AF_INET6
   - setsockopt(3, SOL_SOCKET, SO_REUSEADDR, ...)
   - setsockopt(3, IPPROTO_IPV6, IPV6_V6ONLY, ...)
   - bind(3, {sa_family=AF_INET6, port=5001}, 28)
   - listen(3, 2147483647)

2. **等待并接受控制连接**
   - pselect6(4, readfds, writefds, ...) = 1
   - accept(3, ...) = 4  // 控制连接
   - setsockopt(4, IPPROTO_TCP, TCP_NODELAY, ...)
   - read(4, ..., 37)  // 读取控制消息
   - write(4, ..., 1)  // 发送响应

3. **接受数据连接**
   - pselect6(5, readfds, writefds, ...) = 1
   - accept(3, ...) = 5  // 数据连接
   - read(5, ..., 37)  // 读取初始数据
   - fcntl(5, F_GETFL)
   - fcntl(5, F_SETFL, O_RDWR|O_LARGEFILE|O_NONBLOCK)  // 设置非阻塞

4. **数据接收循环**
   - pselect6(6, readfds, writefds, ...) = 1
   - read(5, buffer, 131072) = N  // 读取数据
   - read(5, ...) = -1 errno=11 (EAGAIN)  // 非阻塞读，无数据
   - [重复 pselect6 + read 循环]

### 客户端 (Client)

1. **建立控制连接**
   - socket(PF_INET, SOCK_STREAM, IPPROTO_IP) = 4
   - connect(4, {127.0.0.1:5001}, 16)
   - setsockopt(4, IPPROTO_TCP, TCP_NODELAY, ...)
   - write(4, ..., 37)  // 发送控制消息
   - getsockopt(4, IPPROTO_TCP, TCP_MAXSEG, ...)

2. **等待服务器响应**
   - pselect6(5, readfds, writefds, ...) = 1
   - read(4, ..., 1)
   - write(4, ..., 4)
   - write(4, ..., 123)

3. **建立数据连接**
   - socket(PF_INET, SOCK_STREAM, IPPROTO_IP) = 5
   - getsockopt(5, SOL_SOCKET, SO_SNDBUF, ...)
   - getsockopt(5, SOL_SOCKET, SO_RCVBUF, ...)
   - connect(5, {127.0.0.1:5001}, 16)
   - write(5, ..., 37)
   - getsockopt(5, IPPROTO_TCP, TCP_MAXSEG, ...)

4. **数据发送循环**
   - pselect6(6, readfds, writefds, ...) = 1  // 大量重复调用
   - [持续轮询，等待可写/可读事件]

## 核心系统调用统计

### 网络相关
- **socket**: 创建 socket (AF_INET/AF_INET6, SOCK_STREAM)
- **bind**: 服务器绑定地址和端口
- **listen**: 服务器开始监听
- **accept**: 服务器接受连接 (控制连接 + 数据连接)
- **connect**: 客户端连接服务器
- **read/write**: 数据传输
- **setsockopt/getsockopt**: 设置/获取 socket 选项
  - TCP_NODELAY: 禁用 Nagle 算法
  - SO_REUSEADDR: 允许地址重用
  - SO_SNDBUF/SO_RCVBUF: 缓冲区大小
  - TCP_MAXSEG: 最大段大小

### I/O 多路复用
- **pselect6**: 主要的 I/O 多路复用机制
  - 服务器: 监听新连接 + 数据接收
  - 客户端: 大量轮询等待可写/可读事件

### 文件控制
- **fcntl**: 
  - F_GETFL: 获取文件状态标志
  - F_SETFL: 设置非阻塞模式 (O_NONBLOCK)

## 关键观察

1. **双连接架构**: iperf3 使用两个 TCP 连接
   - 控制连接 (fd=4): 交换控制消息和测试参数
   - 数据连接 (fd=5): 实际数据传输

2. **非阻塞 I/O**: 服务器将数据 socket 设置为非阻塞模式
   - 使用 pselect6 等待事件
   - read 返回 EAGAIN 时继续轮询

3. **pselect6 密集使用**: 客户端在数据传输期间大量调用 pselect6
   - 这是性能关键路径
   - 需要确保 pselect6 正确报告 socket 可读/可写状态

4. **Socket 选项优化**:
   - TCP_NODELAY: 减少延迟
   - 非阻塞模式: 避免阻塞
   - 大缓冲区: 提高吞吐量

## 对 OS 实现的要求

1. **必须正确实现的系统调用**:
   - socket, bind, listen, accept, connect
   - read, write (TCP socket)
   - pselect6 (或 ppoll)
   - fcntl (F_GETFL, F_SETFL)
   - setsockopt, getsockopt

2. **pselect6 的关键要求**:
   - 必须正确查询 socket 的可读/可写状态
   - 对于 socket 文件，不能只检查文件标志位
   - 必须穿透到 SocketFile 实现，查询 smoltcp 状态
   - 需要支持非阻塞 socket 的 EAGAIN 处理

3. **非阻塞 I/O 支持**:
   - fcntl 设置 O_NONBLOCK
   - read/write 在无数据时返回 EAGAIN
   - pselect6 正确报告就绪状态

