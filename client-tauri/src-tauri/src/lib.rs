use client_core::{Client, ClientEvent};
use protocol::{PlayerInfo, RoomInfo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, State};
use tokio::sync::{mpsc, Mutex};

/// 应用状态
struct AppState {
    client: Option<Arc<Client>>,
    connected: bool,
    nickname: String,
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
            tokio::spawn(async move {
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
                        }
                        ClientEvent::CreateRoomFailed { reason } => {
                            let _ = app_clone.emit("create-room-failed", reason);
                        }
                        ClientEvent::JoinRoomOk { virtual_ip } => {
                            let bytes = virtual_ip.to_be_bytes();
                            let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                            let _ = app_clone.emit("join-room-ok", ip_str);
                        }
                        ClientEvent::JoinRoomFailed { reason } => {
                            let _ = app_clone.emit("join-room-failed", reason);
                        }
                        ClientEvent::LeaveRoomOk => {
                            let _ = app_clone.emit("leave-room-ok", ());
                        }
                        ClientEvent::Kicked { reason } => {
                            let _ = app_clone.emit("kicked", reason);
                        }
                        ClientEvent::RoomDisbanded => {
                            let _ = app_clone.emit("room-disbanded", ());
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state: SharedState = Arc::new(Mutex::new(AppState {
        client: None,
        connected: false,
        nickname: String::new(),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
