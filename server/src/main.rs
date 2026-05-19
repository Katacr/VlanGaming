use protocol::{decode, encode, ClientMessage, ServerMessage};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// 房间中的客户端信息
struct Peer {
    addr: SocketAddr,
    virtual_ip: u32,
}

/// 房间状态
struct Room {
    peers: Vec<Peer>,
    next_ip: u8, // 下一个分配的 IP 末位（10.10.0.x）
}

impl Room {
    fn new() -> Self {
        Room {
            peers: Vec::new(),
            next_ip: 1,
        }
    }

    fn allocate_ip(&mut self) -> u32 {
        let ip = u32::from_be_bytes([10, 10, 0, self.next_ip]);
        self.next_ip += 1;
        ip
    }

    fn find_peer_by_ip(&self, virtual_ip: u32) -> Option<&Peer> {
        self.peers.iter().find(|p| p.virtual_ip == virtual_ip)
    }

    fn find_peer_by_addr(&self, addr: &SocketAddr) -> Option<&Peer> {
        self.peers.iter().find(|p| p.addr == *addr)
    }

    fn remove_peer_by_addr(&mut self, addr: &SocketAddr) -> Option<Peer> {
        if let Some(pos) = self.peers.iter().position(|p| p.addr == *addr) {
            Some(self.peers.remove(pos))
        } else {
            None
        }
    }
}

type Rooms = Arc<RwLock<HashMap<String, Room>>>;

#[tokio::main]
async fn main() {
    let bind_addr = "0.0.0.0:9876";
    let socket = UdpSocket::bind(bind_addr).await.expect("无法绑定 UDP 端口");
    let socket = Arc::new(socket);
    println!("服务端已启动，监听 {}", bind_addr);

    let rooms: Rooms = Arc::new(RwLock::new(HashMap::new()));

    let mut buf = [0u8; 65535];
    loop {
        let (len, src_addr) = match socket.recv_from(&mut buf).await {
            Ok(result) => result,
            Err(e) => {
                eprintln!("接收数据失败: {}", e);
                continue;
            }
        };

        let data = &buf[..len];
        let msg: ClientMessage = match decode(data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("解码消息失败 from {}: {}", src_addr, e);
                continue;
            }
        };

        match msg {
            ClientMessage::Register { room_id } => {
                let mut rooms = rooms.write().await;
                let room = rooms.entry(room_id.clone()).or_insert_with(Room::new);

                // 如果已注册，忽略重复注册
                if room.find_peer_by_addr(&src_addr).is_some() {
                    continue;
                }

                let virtual_ip = room.allocate_ip();
                println!(
                    "[{}] 新客户端注册: {} -> 虚拟IP 10.10.0.{}",
                    room_id,
                    src_addr,
                    virtual_ip.to_be_bytes()[3]
                );

                // 通知已有节点有新成员加入
                let join_msg = encode(&ServerMessage::PeerJoined { virtual_ip }).unwrap();
                for peer in &room.peers {
                    let _ = socket.send_to(&join_msg, peer.addr).await;
                }

                // 通知新客户端已有的节点
                for peer in &room.peers {
                    let existing_msg =
                        encode(&ServerMessage::PeerJoined { virtual_ip: peer.virtual_ip }).unwrap();
                    let _ = socket.send_to(&existing_msg, src_addr).await;
                }

                // 添加新节点
                room.peers.push(Peer {
                    addr: src_addr,
                    virtual_ip,
                });

                // 回复注册成功
                let reply = encode(&ServerMessage::RegisterOk { virtual_ip }).unwrap();
                let _ = socket.send_to(&reply, src_addr).await;
            }

            ClientMessage::Data { dst_ip, payload } => {
                let rooms = rooms.read().await;
                // 查找发送者所在的房间并转发
                for room in rooms.values() {
                    if let Some(src_peer) = room.find_peer_by_addr(&src_addr) {
                        let src_ip = src_peer.virtual_ip;
                        if let Some(dst_peer) = room.find_peer_by_ip(dst_ip) {
                            let forward =
                                encode(&ServerMessage::Data { src_ip, payload }).unwrap();
                            let _ = socket.send_to(&forward, dst_peer.addr).await;
                        }
                        break;
                    }
                }
            }

            ClientMessage::Heartbeat => {
                let reply = encode(&ServerMessage::HeartbeatAck).unwrap();
                let _ = socket.send_to(&reply, src_addr).await;
            }
        }
    }
}
