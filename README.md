# VlanGaming

基于 UDP 隧道 + WinTun 虚拟网卡的游戏局域网联机工具。让不在同一物理网络的玩家通过互联网组建虚拟局域网，实现局域网游戏联机。

## 工作原理

```
┌──────────────────┐         UDP          ┌──────────────────┐
│   客户端 A       │◄───────────────────►  │     服务端       │
│  TUN 虚拟网卡    │                       │  房间管理/转发    │
│  10.10.0.1       │         UDP          │  0.0.0.0:9876    │
└──────────────────┘◄───────────────────►  └──────────────────┘
                                                    ▲
                                                    │ UDP
                                                    ▼
                                           ┌──────────────────┐
                                           │   客户端 B       │
                                           │  TUN 虚拟网卡    │
                                           │  10.10.0.2       │
                                           └──────────────────┘
```

1. 客户端启动后向服务端注册，加入指定房间
2. 服务端为每个客户端分配虚拟 IP（`10.10.0.x`）
3. 客户端创建 WinTun 虚拟网卡，配置分配到的虚拟 IP
4. 本机发往虚拟网段的流量被虚拟网卡捕获，通过 UDP 隧道发送到服务端
5. 服务端根据目标虚拟 IP 将数据包转发给对应客户端
6. 接收方将数据包写入本地虚拟网卡，操作系统视其为局域网流量

## 项目结构

```
vlan-gaming/
├── protocol/        # 通信协议定义（消息类型 + bincode 序列化）
├── server/          # UDP 中继服务端（房间管理、IP 分配、数据转发）
├── client-core/     # 客户端核心库（网络通信 + WinTun 虚拟网卡封装）
│   ├── src/
│   │   ├── lib.rs   # 客户端主逻辑、事件系统、隧道双向转发
│   │   └── tun.rs   # WinTun 虚拟网卡创建与读写封装
│   └── examples/
│       ├── tun_client.rs  # 完整客户端示例（虚拟网卡模式）
│       └── chat_test.rs   # 集成测试（双客户端通信验证）
└── deps/            # 依赖文件（wintun.dll）
```

## 协议设计

使用 bincode 进行高效二进制序列化，基于 UDP 传输。

**客户端 → 服务端：**

| 消息类型 | 说明 |
|---------|------|
| `Register { room_id }` | 注册加入指定房间 |
| `Data { dst_ip, payload }` | 发送数据到目标虚拟 IP |
| `Heartbeat` | 心跳保活 |

**服务端 → 客户端：**

| 消息类型 | 说明 |
|---------|------|
| `RegisterOk { virtual_ip }` | 注册成功，返回分配的虚拟 IP |
| `RegisterFailed { reason }` | 注册失败 |
| `Data { src_ip, payload }` | 转发来自其他客户端的数据 |
| `PeerJoined { virtual_ip }` | 新节点加入通知 |
| `PeerLeft { virtual_ip }` | 节点离开通知 |
| `HeartbeatAck` | 心跳响应 |

## 快速开始

### 环境要求

- Rust 工具链（推荐通过 [rustup](https://rustup.rs/) 安装）
- Windows 系统（WinTun 仅支持 Windows）
- 管理员权限（创建虚拟网卡需要）

### 启动服务端

```bash
cargo run -p server
```

服务端默认监听 `0.0.0.0:9876`。

### 启动客户端

以**管理员身份**运行：

```bash
cargo run -p client-core --example tun_client -- <服务端IP:端口> <房间名>
```

示例：

```bash
# 连接本地服务端，加入 "my-game" 房间
cargo run -p client-core --example tun_client -- 127.0.0.1:9876 my-game

# 连接远程服务端
cargo run -p client-core --example tun_client -- 1.2.3.4:9876 my-game
```

连接成功后，同一房间内的客户端可以通过虚拟 IP（`10.10.0.x`）互相访问，就像在同一个局域网中一样。

### 通信测试

先启动服务端，然后运行集成测试验证基础通信：

```bash
cargo run -p server
# 另一个终端
cargo run -p client-core --example chat_test
```

## 技术栈

- **Rust** — 系统级语言，保证性能和内存安全
- **Tokio** — 异步运行时，处理高并发 UDP I/O
- **WinTun** — Windows 内核级虚拟网卡驱动
- **Bincode + Serde** — 高效二进制序列化协议

## 许可证

WinTun 驱动遵循其自身许可证，详见 `deps/wintun/LICENSE.txt`。
