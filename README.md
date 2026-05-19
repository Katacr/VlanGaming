# VLan Gaming

基于 UDP 隧道 + WinTun 虚拟网卡的游戏局域网联机工具。让不在同一物理网络的玩家通过互联网组建虚拟局域网，实现局域网游戏联机。

## 功能特性

- **虚拟组网** — 通过 WinTun 创建虚拟网卡，自动分配虚拟 IP，实现二层网络互通
- **房间系统** — 创建/加入/离开房间，同一房间内的玩家处于同一虚拟子网
- **房间管理** — 房间创建者为管理员，可踢出玩家或解散房间
- **实时聊天** — 房间内文字聊天，支持聊天历史记录（最近 50 条）
- **延迟检测** — 实时 Ping 显示各玩家网络延迟
- **超时清理** — 服务端自动清理无活动的客户端和空房间
- **系统托盘** — 关闭窗口最小化到托盘，双击恢复
- **桌面客户端** — 基于 Tauri 2 的原生桌面应用，轻量高效

## 工作原理

```
┌──────────────────┐         UDP          ┌──────────────────┐
│   客户端 A       │◄───────────────────►  │     服务端       │
│  TUN 虚拟网卡    │                       │  房间管理/转发    │
│  10.10.x.1       │         UDP          │  0.0.0.0:9876    │
└──────────────────┘◄───────────────────►  └──────────────────┘
                                                    ▲
                                                    │ UDP
                                                    ▼
                                           ┌──────────────────┐
                                           │   客户端 B       │
                                           │  TUN 虚拟网卡    │
                                           │  10.10.x.2       │
                                           └──────────────────┘
```

1. 客户端连接服务端，设置昵称
2. 创建或加入房间，服务端为每个房间分配独立子网（`10.10.x.0/24`）
3. 客户端创建 WinTun 虚拟网卡，配置分配到的虚拟 IP
4. 本机发往虚拟网段的流量被虚拟网卡捕获，通过 UDP 隧道发送到服务端
5. 服务端根据目标虚拟 IP 将数据包转发给对应客户端
6. 接收方将数据包写入本地虚拟网卡，操作系统视其为局域网流量

## 项目结构

```
vlan-gaming/
├── protocol/              # 通信协议（消息类型 + bincode 序列化）
├── server/                # UDP 中继服务端
├── client-core/           # 客户端核心库（网络通信 + WinTun 封装）
│   ├── src/
│   │   ├── lib.rs         # 客户端主逻辑、事件系统
│   │   └── tun.rs         # WinTun 虚拟网卡创建与读写
│   └── examples/
│       ├── tun_client.rs  # CLI 客户端示例（虚拟网卡模式）
│       └── chat_test.rs   # 集成测试
├── client-tauri/          # Tauri 桌面客户端
│   ├── src/               # React 前端
│   │   ├── App.tsx        # 应用路由与页面切换
│   │   └── pages/
│   │       ├── HomePage.tsx      # 连接页面
│   │       ├── RoomListPage.tsx  # 房间列表
│   │       └── RoomPage.tsx      # 房间内（聊天/玩家列表）
│   └── src-tauri/         # Tauri 后端（Rust）
│       └── src/lib.rs     # Tauri commands + 事件转发 + 系统托盘
└── deps/                  # 依赖文件（wintun.dll）
```

## 协议设计

使用 Bincode 进行高效二进制序列化，基于 UDP 传输。

**客户端 → 服务端：**

| 消息类型 | 说明 |
|---------|------|
| `Connect { nickname }` | 连接服务器，注册昵称 |
| `ListRooms` | 请求房间列表 |
| `CreateRoom { room_id }` | 创建房间（自动加入） |
| `JoinRoom { room_id }` | 加入房间 |
| `LeaveRoom` | 离开房间 |
| `KickPlayer { virtual_ip }` | 踢出玩家（仅管理员） |
| `DisbandRoom` | 解散房间（仅管理员） |
| `Data { dst_ip, payload }` | 发送数据到目标虚拟 IP |
| `Chat { content }` | 发送聊天消息 |
| `Ping { timestamp }` | 延迟检测 |
| `Heartbeat` | 心跳保活 |

**服务端 → 客户端：**

| 消息类型 | 说明 |
|---------|------|
| `ConnectOk` | 连接成功 |
| `RoomList { rooms }` | 房间列表 |
| `CreateRoomOk { room_id, virtual_ip }` | 创建房间成功 |
| `JoinRoomOk { virtual_ip }` | 加入房间成功 |
| `LeaveRoomOk` | 离开确认 |
| `Kicked { reason }` | 被踢出通知 |
| `RoomDisbanded` | 房间被解散 |
| `Data { src_ip, payload }` | 转发数据 |
| `ChatBroadcast { msg }` | 聊天广播 |
| `PlayerList { players }` | 玩家列表更新 |
| `PeerJoined { nickname, virtual_ip }` | 新玩家加入 |
| `PeerLeft { nickname, virtual_ip }` | 玩家离开 |
| `Pong { timestamp }` | Ping 响应 |

## 快速开始

### 环境要求

- [Rust](https://rustup.rs/) 工具链
- [Node.js](https://nodejs.org/)（前端构建）
- Windows 系统（WinTun 仅支持 Windows）
- 管理员权限（创建虚拟网卡需要）

### 启动服务端

```bash
cargo run -p server
```

服务端默认监听 `0.0.0.0:9876`。

### 启动桌面客户端（开发模式）

```bash
cd client-tauri
npm install
npm run tauri dev
```

### CLI 客户端（虚拟网卡模式）

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

## 使用流程

1. 启动服务端（可部署在公网服务器）
2. 打开桌面客户端，输入服务器地址和昵称，点击连接
3. 创建或加入房间
4. 同一房间内的玩家自动获得虚拟 IP，可通过虚拟 IP 互相访问
5. 在游戏中使用局域网联机功能，搜索或直连虚拟 IP 即可

## 技术栈

| 层级 | 技术 |
|------|------|
| 协议 | Bincode + Serde |
| 服务端 | Rust + Tokio |
| 客户端核心 | Rust + Tokio + WinTun |
| 桌面前端 | React 19 + TypeScript + Vite 7 |
| 桌面框架 | Tauri 2 |

## 许可证

WinTun 驱动遵循其自身许可证，详见 `deps/wintun/LICENSE.txt`。
