pub mod tun;

use protocol::{decode, encode, ClientMessage, ServerMessage};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tun::TunDevice;

/// 客户端事件，供上层（GUI）消费
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// 注册成功，获得虚拟 IP
    Connected { virtual_ip: u32 },
    /// 注册失败
    ConnectFailed { reason: String },
    /// 收到来自其他节点的数据
    DataReceived { src_ip: u32, payload: Vec<u8> },
    /// 新节点加入
    PeerJoined { virtual_ip: u32 },
    /// 节点离开
    PeerLeft { virtual_ip: u32 },
}

/// 客户端核心，管理与服务端的 UDP 通信
pub struct Client {
    socket: Arc<UdpSocket>,
    server_addr: SocketAddr,
    virtual_ip: Option<u32>,
}

impl Client {
    /// 创建客户端并绑定本地 UDP 端口
    pub async fn new(server_addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Client {
            socket: Arc::new(socket),
            server_addr,
            virtual_ip: None,
        })
    }

    /// 发送注册请求加入房间
    pub async fn register(&self, room_id: &str) -> std::io::Result<()> {
        let msg = ClientMessage::Register {
            room_id: room_id.to_string(),
        };
        let data = encode(&msg).expect("编码失败");
        self.socket.send_to(&data, self.server_addr).await?;
        Ok(())
    }

    /// 发送数据到指定虚拟 IP
    pub async fn send_data(&self, dst_ip: u32, payload: Vec<u8>) -> std::io::Result<()> {
        let msg = ClientMessage::Data { dst_ip, payload };
        let data = encode(&msg).expect("编码失败");
        self.socket.send_to(&data, self.server_addr).await?;
        Ok(())
    }

    /// 发送心跳
    pub async fn send_heartbeat(&self) -> std::io::Result<()> {
        let msg = ClientMessage::Heartbeat;
        let data = encode(&msg).expect("编码失败");
        self.socket.send_to(&data, self.server_addr).await?;
        Ok(())
    }

    /// 启动接收循环，将服务端消息转为事件发送到 channel
    pub async fn run(
        &mut self,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
    ) -> std::io::Result<()> {
        let mut buf = [0u8; 65535];
        loop {
            let (len, _src) = self.socket.recv_from(&mut buf).await?;
            let data = &buf[..len];

            let msg: ServerMessage = match decode(data) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("解码服务端消息失败: {}", e);
                    continue;
                }
            };

            match msg {
                ServerMessage::RegisterOk { virtual_ip } => {
                    self.virtual_ip = Some(virtual_ip);
                    let _ = event_tx.send(ClientEvent::Connected { virtual_ip });
                }
                ServerMessage::RegisterFailed { reason } => {
                    let _ = event_tx.send(ClientEvent::ConnectFailed { reason });
                }
                ServerMessage::Data { src_ip, payload } => {
                    let _ = event_tx.send(ClientEvent::DataReceived { src_ip, payload });
                }
                ServerMessage::PeerJoined { virtual_ip } => {
                    let _ = event_tx.send(ClientEvent::PeerJoined { virtual_ip });
                }
                ServerMessage::PeerLeft { virtual_ip } => {
                    let _ = event_tx.send(ClientEvent::PeerLeft { virtual_ip });
                }
                ServerMessage::HeartbeatAck => {}
            }
        }
    }

    /// 获取当前虚拟 IP
    pub fn virtual_ip(&self) -> Option<u32> {
        self.virtual_ip
    }

    /// 启动完整的虚拟网卡隧道（注册 → 创建网卡 → 双向转发）
    pub async fn run_with_tun(
        &mut self,
        room_id: &str,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. 注册到服务端
        self.register(room_id).await?;

        // 2. 等待注册成功，获取虚拟 IP
        let virtual_ip = self.wait_for_register(&event_tx).await?;
        let ip_bytes = virtual_ip.to_be_bytes();
        let ip_addr = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);

        println!("已连接，虚拟 IP: {}", ip_addr);

        // 3. 创建虚拟网卡
        let tun = TunDevice::create(ip_addr, Ipv4Addr::new(255, 255, 255, 0))?;
        println!("虚拟网卡已创建");

        let session_read = tun.session.clone();
        let session_write = tun.session.clone();
        let socket_send = self.socket.clone();
        let socket_recv = self.socket.clone();
        let server_addr = self.server_addr;

        // 4. 启动双向转发

        // 方向 A：虚拟网卡 → UDP 隧道（从网卡读取 IP 包，发送到服务端）
        let tun_to_udp = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            loop {
                match session_read.receive_blocking() {
                    Ok(packet) => {
                        let bytes = packet.bytes().to_vec();
                        // 从 IP 包头提取目标 IP（偏移 16-19 字节）
                        if bytes.len() >= 20 {
                            let dst_ip = u32::from_be_bytes([
                                bytes[16], bytes[17], bytes[18], bytes[19],
                            ]);
                            let msg = ClientMessage::Data {
                                dst_ip,
                                payload: bytes,
                            };
                            let data = encode(&msg).expect("编码失败");
                            let _ = rt.block_on(socket_send.send_to(&data, server_addr));
                        }
                    }
                    Err(e) => {
                        eprintln!("读取虚拟网卡失败: {}", e);
                        break;
                    }
                }
            }
        });

        // 方向 B：UDP 隧道 → 虚拟网卡（从服务端收到数据，写入网卡）
        let udp_to_tun = tokio::spawn(async move {
            let mut buf = [0u8; 65535];
            loop {
                match socket_recv.recv_from(&mut buf).await {
                    Ok((len, _)) => {
                        let data = &buf[..len];
                        let msg: ServerMessage = match decode(data) {
                            Ok(m) => m,
                            Err(_) => continue,
                        };
                        match msg {
                            ServerMessage::Data { payload, .. } => {
                                if let Ok(mut write_pack) =
                                    session_write.allocate_send_packet(payload.len() as u16)
                                {
                                    write_pack.bytes_mut().copy_from_slice(&payload);
                                    session_write.send_packet(write_pack);
                                }
                            }
                            ServerMessage::PeerJoined { virtual_ip } => {
                                let _ = event_tx.send(ClientEvent::PeerJoined { virtual_ip });
                            }
                            ServerMessage::PeerLeft { virtual_ip } => {
                                let _ = event_tx.send(ClientEvent::PeerLeft { virtual_ip });
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        eprintln!("接收 UDP 数据失败: {}", e);
                        break;
                    }
                }
            }
        });

        // 等待任一方向结束
        tokio::select! {
            _ = tun_to_udp => {},
            _ = udp_to_tun => {},
        }

        Ok(())
    }

    /// 等待服务端返回注册结果
    async fn wait_for_register(
        &mut self,
        event_tx: &mpsc::UnboundedSender<ClientEvent>,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let mut buf = [0u8; 65535];
        loop {
            let (len, _) = self.socket.recv_from(&mut buf).await?;
            let data = &buf[..len];
            let msg: ServerMessage = decode(data)?;

            match msg {
                ServerMessage::RegisterOk { virtual_ip } => {
                    self.virtual_ip = Some(virtual_ip);
                    let _ = event_tx.send(ClientEvent::Connected { virtual_ip });
                    return Ok(virtual_ip);
                }
                ServerMessage::RegisterFailed { reason } => {
                    let _ = event_tx.send(ClientEvent::ConnectFailed { reason: reason.clone() });
                    return Err(format!("注册失败: {}", reason).into());
                }
                ServerMessage::PeerJoined { virtual_ip } => {
                    let _ = event_tx.send(ClientEvent::PeerJoined { virtual_ip });
                }
                _ => {}
            }
        }
    }
}
