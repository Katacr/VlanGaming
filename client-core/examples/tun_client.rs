use client_core::{Client, ClientEvent};
use tokio::sync::mpsc;

/// 完整客户端：连接服务端 + 创建虚拟网卡 + 双向转发
/// 使用方法：以管理员身份运行（创建虚拟网卡需要管理员权限）
/// cargo run -p client-core --example tun_client -- <server_ip:port> <room_id>
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server_addr = args
        .get(1)
        .unwrap_or(&"127.0.0.1:9876".to_string())
        .parse()
        .expect("无效的服务器地址");
    let room_id = args.get(2).map(|s| s.as_str()).unwrap_or("default");

    println!("正在连接服务端 {}，房间: {}", server_addr, room_id);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<ClientEvent>();

    // 启动事件监听
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                ClientEvent::Connected { virtual_ip } => {
                    let bytes = virtual_ip.to_be_bytes();
                    println!("事件: 已连接，虚拟 IP: {}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                }
                ClientEvent::PeerJoined { virtual_ip } => {
                    let bytes = virtual_ip.to_be_bytes();
                    println!("事件: 新节点加入 {}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                }
                ClientEvent::PeerLeft { virtual_ip } => {
                    let bytes = virtual_ip.to_be_bytes();
                    println!("事件: 节点离开 {}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                }
                _ => {}
            }
        }
    });

    let mut client = Client::new(server_addr).await.expect("创建客户端失败");

    if let Err(e) = client.run_with_tun(room_id, event_tx).await {
        eprintln!("客户端错误: {}", e);
    }
}
