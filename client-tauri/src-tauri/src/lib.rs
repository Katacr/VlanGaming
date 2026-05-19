use client_core::{Client, ClientEvent};
use client_core::tun::TunDevice;
use protocol::{PlayerInfo, RoomInfo};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State, WindowEvent};
use tokio::sync::{mpsc, Mutex, watch};

/// 应用状态
struct AppState {
    client: Option<Arc<Client>>,
    connected: bool,
    nickname: String,
    tun_shutdown: Option<watch::Sender<bool>>,
}

type SharedState = Arc<Mutex<AppState>>;

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectParams {
    pub server_ip: String,
    pub port: String,
    pub nickname: String,
}

#[derive(Serialize, Clone)]
pub struct SimpleResult {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct RoomListResult {
    pub success: bool,
    pub rooms: Vec<RoomInfo>,
    pub error: Option<String>,
}

/// 前端接收的事件
#[derive(Serialize, Clone)]
pub struct ChatEvent {
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Serialize, Clone)]
pub struct PlayerListEvent {
    pub players: Vec<PlayerInfo>,
}

#[derive(Serialize, Clone)]
pub struct PeerEvent {
    pub nickname: String,
    pub virtual_ip: String,
}

#[derive(Serialize, Clone)]
pub struct CreateRoomOkEvent {
    pub room_id: String,
    pub virtual_ip: String,
}

/// 连接到服务端
#[tauri::command]
async fn connect_server(
    params: ConnectParams,
    state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<SimpleResult, String> {
    let addr_str = format!("{}:{}", params.server_ip, params.port);
    let server_addr = addr_str.parse().map_err(|e| format!("无效的服务器地址: {}", e))?;

    let client = Client::new(server_addr)
        .await
        .map_err(|e| format!("创建客户端失败: {}", e))?;

    client.connect(&params.nickname).await.map_err(|e| format!("发送连接请求失败: {}", e))?;

    let client = Arc::new(client);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<ClientEvent>();

    // 启动接收循环
    let client_clone = client.clone();
    tokio::spawn(async move {
        let _ = client_clone.run_receive_loop(event_tx).await;
    });

    // 等待连接确认
    let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(event) = event_rx.recv().await {
            match event {
                ClientEvent::ConnectOk => return Some(true),
                _ => continue,
            }
        }
        None
    }).await;

    match timeout_result {
        Ok(Some(true)) => {
            // 启动事件转发到前端
            let app_clone = app.clone();
            let state_clone = state.inner().clone();
            let client_for_tun = client.clone();
            tokio::spawn(async move {
                let mut tun_session: Option<Arc<wintun::Session>> = None;

                while let Some(event) = event_rx.recv().await {
                    match event {
                        ClientEvent::RoomList { rooms } => {
                            let _ = app_clone.emit("room-list", RoomListResult {
                                success: true,
                                rooms,
                                error: None,
                            });
                        }
                        ClientEvent::CreateRoomOk { room_id, virtual_ip } => {
                            let bytes = virtual_ip.to_be_bytes();
                            let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                            let _ = app_clone.emit("create-room-ok", CreateRoomOkEvent {
                                room_id,
                                virtual_ip: ip_str,
                            });
                            // 启动虚拟网卡
                            tun_session = start_tun(virtual_ip, &client_for_tun, &state_clone).await;
                        }
                        ClientEvent::CreateRoomFailed { reason } => {
                            let _ = app_clone.emit("create-room-failed", reason);
                        }
                        ClientEvent::JoinRoomOk { virtual_ip } => {
                            let bytes = virtual_ip.to_be_bytes();
                            let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                            let _ = app_clone.emit("join-room-ok", ip_str);
                            // 启动虚拟网卡
                            tun_session = start_tun(virtual_ip, &client_for_tun, &state_clone).await;
                        }
                        ClientEvent::JoinRoomFailed { reason } => {
                            let _ = app_clone.emit("join-room-failed", reason);
                        }
                        ClientEvent::LeaveRoomOk => {
                            stop_tun(&state_clone).await;
                            tun_session = None;
                            let _ = app_clone.emit("leave-room-ok", ());
                        }
                        ClientEvent::Kicked { reason } => {
                            stop_tun(&state_clone).await;
                            tun_session = None;
                            let _ = app_clone.emit("kicked", reason);
                        }
                        ClientEvent::RoomDisbanded => {
                            stop_tun(&state_clone).await;
                            tun_session = None;
                            let _ = app_clone.emit("room-disbanded", ());
                        }
                        ClientEvent::DataReceived { src_ip: _, payload } => {
                            // 将收到的数据写入虚拟网卡
                            if let Some(session) = &tun_session {
                                let session = session.clone();
                                let payload = payload.clone();
                                tokio::task::spawn_blocking(move || {
                                    if let Ok(mut packet) = session.allocate_send_packet(payload.len() as u16) {
                                        packet.bytes_mut().copy_from_slice(&payload);
                                        session.send_packet(packet);
                                    }
                                });
                            }
                        }
                        ClientEvent::ChatMessage { msg } => {
                            let _ = app_clone.emit("chat-message", ChatEvent {
                                sender: msg.sender,
                                content: msg.content,
                                timestamp: msg.timestamp,
                            });
                        }
                        ClientEvent::PlayerList { players } => {
                            let _ = app_clone.emit("player-list", PlayerListEvent { players });
                        }
                        ClientEvent::PeerJoined { nickname, virtual_ip } => {
                            let bytes = virtual_ip.to_be_bytes();
                            let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                            let _ = app_clone.emit("peer-joined", PeerEvent { nickname, virtual_ip: ip_str });
                        }
                        ClientEvent::PeerLeft { nickname, virtual_ip } => {
                            let bytes = virtual_ip.to_be_bytes();
                            let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                            let _ = app_clone.emit("peer-left", PeerEvent { nickname, virtual_ip: ip_str });
                        }
                        ClientEvent::Pong { timestamp } => {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;
                            let ping_ms = now.saturating_sub(timestamp);
                            let _ = app_clone.emit("ping-update", ping_ms);
                        }
                        _ => {}
                    }
                }
            });

            let mut app_state = state.lock().await;
            app_state.client = Some(client);
            app_state.connected = true;
            app_state.nickname = params.nickname;

            Ok(SimpleResult { success: true, error: None })
        }
        _ => Ok(SimpleResult {
            success: false,
            error: Some("连接超时".to_string()),
        }),
    }
}

/// 获取房间列表
#[tauri::command]
async fn list_rooms(state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.list_rooms().await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 创建房间
#[tauri::command]
async fn create_room(room_id: String, state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.create_room(&room_id).await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 加入房间
#[tauri::command]
async fn join_room(room_id: String, state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.join_room(&room_id).await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 离开房间
#[tauri::command]
async fn leave_room(state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.leave_room().await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 踢出玩家（仅 admin）
#[tauri::command]
async fn kick_player(virtual_ip: u32, state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.kick_player(virtual_ip).await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 解散房间（仅 admin）
#[tauri::command]
async fn disband_room(state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.disband_room().await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 发送聊天消息
#[tauri::command]
async fn send_chat(content: String, state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.send_chat(&content).await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 发送 Ping
#[tauri::command]
async fn send_ping(state: State<'_, SharedState>) -> Result<SimpleResult, String> {
    let app_state = state.lock().await;
    if let Some(client) = &app_state.client {
        client.send_ping().await.map_err(|e| e.to_string())?;
        Ok(SimpleResult { success: true, error: None })
    } else {
        Ok(SimpleResult { success: false, error: Some("未连接".to_string()) })
    }
}

/// 测试服务器延迟（不需要登录）
#[tauri::command]
async fn ping_server_latency(ip: String, port: String) -> Result<u32, String> {
    let addr = format!("{}:{}", ip, port);
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| e.to_string())?;
    socket.connect(&addr).await.map_err(|e| e.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let msg = protocol::encode(&protocol::ClientMessage::Ping { timestamp: now })
        .map_err(|e| e.to_string())?;
    socket.send(&msg).await.map_err(|e| e.to_string())?;

    let mut buf = [0u8; 1024];
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(3), socket.recv(&mut buf)).await;

    match timeout {
        Ok(Ok(_)) => {
            let elapsed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            Ok((elapsed.saturating_sub(now)) as u32)
        }
        _ => Err("超时".to_string()),
    }
}

/// 启动虚拟网卡，返回 session 引用（用于写入数据）
async fn start_tun(virtual_ip: u32, client: &Arc<Client>, state: &SharedState) -> Option<Arc<wintun::Session>> {
    let bytes = virtual_ip.to_be_bytes();
    let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
    let mask = Ipv4Addr::new(255, 255, 255, 0);

    let tun = match TunDevice::create(ip, mask) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[TUN] 创建虚拟网卡失败: {}", e);
            return None;
        }
    };

    let session = tun.session.clone();

    // 创建 shutdown 信号
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // 启动 tun 读取循环：从虚拟网卡读取 IP 包，发送给服务端
    let session_read = session.clone();
    let client_clone = client.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            // 检查 shutdown
            if *shutdown_rx.borrow() {
                break;
            }
            match session_read.receive_blocking() {
                Ok(packet) => {
                    let data = packet.bytes().to_vec();
                    // 从 IP 包头提取目标 IP（偏移 16~19）
                    if data.len() >= 20 {
                        let dst_ip = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
                        let client = client_clone.clone();
                        // 用 tokio runtime 发送
                        let _ = tokio::runtime::Handle::current().block_on(async {
                            client.send_data(dst_ip, data).await
                        });
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 保存 shutdown sender
    let mut app_state = state.lock().await;
    app_state.tun_shutdown = Some(shutdown_tx);

    // 保持 TunDevice 存活（移入后台任务）
    std::mem::forget(tun);

    println!("[TUN] 虚拟网卡已启动: {}", ip);
    Some(session)
}

/// 关闭虚拟网卡
async fn stop_tun(state: &SharedState) {
    let mut app_state = state.lock().await;
    if let Some(tx) = app_state.tun_shutdown.take() {
        let _ = tx.send(true);
        println!("[TUN] 虚拟网卡已关闭");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state: SharedState = Arc::new(Mutex::new(AppState {
        client: None,
        connected: false,
        nickname: String::new(),
        tun_shutdown: None,
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            connect_server,
            list_rooms,
            create_room,
            join_room,
            leave_room,
            kick_player,
            disband_room,
            send_chat,
            send_ping,
            ping_server_latency,
        ])
        .setup(|app| {
            // 创建托盘右键菜单
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            // 创建系统托盘图标
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("VLan Gaming")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 拦截窗口关闭事件，改为隐藏
            let window = app.get_webview_window("main").unwrap();
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
