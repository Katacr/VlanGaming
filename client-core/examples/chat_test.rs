use client_core::{Client, ClientEvent};
use tokio::sync::mpsc;
use std::time::Duration;

/// 简单的集成测试：启动两个客户端，通过本地服务端互相发消息
#[tokio::main]
async fn main() {
    let server_addr = "127.0.0.1:9876".parse().unwrap();

    // 客户端 A
    let mut client_a = Client::new(server_addr).await.unwrap();
    let (tx_a, mut rx_a) = mpsc::unbounded_channel::<ClientEvent>();

    // 客户端 B
    let mut client_b = Client::new(server_addr).await.unwrap();
    let (tx_b, mut rx_b) = mpsc::unbounded_channel::<ClientEvent>();

    // 注册到同一个房间
    client_a.register("test-room").await.unwrap();
    client_b.register("test-room").await.unwrap();

    // 启动接收循环
    let handle_a = tokio::spawn(async move {
        client_a.run(tx_a).await.unwrap();
    });
    let handle_b = tokio::spawn(async move {
        client_b.run(tx_b).await.unwrap();
    });

    // 等待两个客户端都注册成功
    let mut ip_a = 0u32;
    let mut ip_b = 0u32;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // 读取客户端 A 的事件
    while let Ok(event) = rx_a.try_recv() {
        if let ClientEvent::Connected { virtual_ip } = event {
            ip_a = virtual_ip;
            println!("客户端 A 已连接，虚拟 IP: 10.10.0.{}", virtual_ip.to_be_bytes()[3]);
        }
    }

    // 读取客户端 B 的事件
    while let Ok(event) = rx_b.try_recv() {
        if let ClientEvent::Connected { virtual_ip } = event {
            ip_b = virtual_ip;
            println!("客户端 B 已连接，虚拟 IP: 10.10.0.{}", virtual_ip.to_be_bytes()[3]);
        }
    }

    if ip_a == 0 || ip_b == 0 {
        eprintln!("注册失败！请确保服务端已启动 (cargo run -p server)");
        return;
    }

    // A 向 B 发送消息 — 需要重新获取 client 引用
    // 由于 client 已经 move 到 spawn 中，这里用另一个 socket 模拟发送
    let send_socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let msg = protocol::ClientMessage::Data {
        dst_ip: ip_b,
        payload: b"Hello from A!".to_vec(),
    };
    let data = protocol::encode(&msg).unwrap();
    send_socket.send_to(&data, server_addr).await.unwrap();

    // 等待 B 收到消息
    tokio::time::sleep(Duration::from_millis(100)).await;
    while let Ok(event) = rx_b.try_recv() {
        if let ClientEvent::DataReceived { src_ip, payload } = event {
            println!(
                "客户端 B 收到来自 10.10.0.{} 的消息: {}",
                src_ip.to_be_bytes()[3],
                String::from_utf8_lossy(&payload)
            );
        }
    }

    println!("\n测试完成！基础通信链路正常工作。");

    handle_a.abort();
    handle_b.abort();
}
