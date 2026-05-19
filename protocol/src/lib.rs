use serde::{Deserialize, Serialize};

/// 客户端发送给服务端的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// 注册：客户端上线，请求加入网络
    Register { room_id: String },
    /// 数据转发：发送数据到指定虚拟 IP
    Data { dst_ip: u32, payload: Vec<u8> },
    /// 心跳保活
    Heartbeat,
}

/// 服务端发送给客户端的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// 注册成功，返回分配的虚拟 IP
    RegisterOk { virtual_ip: u32 },
    /// 注册失败
    RegisterFailed { reason: String },
    /// 转发来自其他客户端的数据
    Data { src_ip: u32, payload: Vec<u8> },
    /// 节点上线通知
    PeerJoined { virtual_ip: u32 },
    /// 节点下线通知
    PeerLeft { virtual_ip: u32 },
    /// 心跳响应
    HeartbeatAck,
}

/// 将消息序列化为字节
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(msg)
}

/// 从字节反序列化消息
pub fn decode<'a, T: Deserialize<'a>>(data: &'a [u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_roundtrip() {
        let msg = ClientMessage::Register {
            room_id: "test-room".to_string(),
        };
        let encoded = encode(&msg).unwrap();
        let decoded: ClientMessage = decode(&encoded).unwrap();
        match decoded {
            ClientMessage::Register { room_id } => assert_eq!(room_id, "test-room"),
            _ => panic!("unexpected message type"),
        }
    }

    #[test]
    fn test_server_message_roundtrip() {
        let msg = ServerMessage::RegisterOk {
            virtual_ip: 0x0A0A0001, // 10.10.0.1
        };
        let encoded = encode(&msg).unwrap();
        let decoded: ServerMessage = decode(&encoded).unwrap();
        match decoded {
            ServerMessage::RegisterOk { virtual_ip } => assert_eq!(virtual_ip, 0x0A0A0001),
            _ => panic!("unexpected message type"),
        }
    }
}
