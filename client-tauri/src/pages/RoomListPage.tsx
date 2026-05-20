import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface RoomInfo {
  room_id: string;
  player_count: number;
  max_players: number;
  disband_at: number;
}

interface Props {
  nickname: string;
  onJoinRoom: (roomId: string, virtualIp: string, maxPlayers: number) => void;
  onBack: () => void;
}

export default function RoomListPage({ nickname, onJoinRoom, onBack }: Props) {
  const [rooms, setRooms] = useState<RoomInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [newRoomId, setNewRoomId] = useState(() => "游戏房间" + String(Math.floor(1000 + Math.random() * 9000)));
  const [maxPlayers, setMaxPlayers] = useState("20");
  const [error, setError] = useState<string | null>(null);
  const [joining, setJoining] = useState<string | null>(null);
  const [showToast, setShowToast] = useState(false);
  const [nowSec, setNowSec] = useState(() => Math.floor(Date.now() / 1000));
  const manualRefreshRef = useRef(false);
  const maxPlayersRef = useRef(20);
  const roomsRef = useRef<RoomInfo[]>([]);
  const joiningRef = useRef<string | null>(null);
  const onJoinRoomRef = useRef(onJoinRoom);
  onJoinRoomRef.current = onJoinRoom;

  useEffect(() => {
    joiningRef.current = joining;
  }, [joining]);

  useEffect(() => {
    const unlistenRoomList = listen<any>("room-list", (event) => {
      const newRooms = event.payload.rooms || [];
      setRooms(newRooms);
      roomsRef.current = newRooms;
      setLoading(false);
      if (manualRefreshRef.current) {
        manualRefreshRef.current = false;
        setShowToast(true);
        setTimeout(() => setShowToast(false), 1500);
      }
    });

    const unlistenCreateOk = listen<{ room_id: string; virtual_ip: string }>("create-room-ok", (event) => {
      setShowCreate(false);
      setNewRoomId("");
      onJoinRoomRef.current(event.payload.room_id, event.payload.virtual_ip, maxPlayersRef.current);
    });

    const unlistenCreateFailed = listen<string>("create-room-failed", (event) => {
      setError(event.payload);
    });

    const unlistenJoinOk = listen<string>("join-room-ok", (event) => {
      const roomId = joiningRef.current;
      if (roomId) {
        const room = roomsRef.current.find((r) => r.room_id === roomId);
        const mp = room?.max_players ?? 20;
        onJoinRoomRef.current(roomId, event.payload, mp);
      }
      setJoining(null);
    });

    const unlistenJoinFailed = listen<string>("join-room-failed", (event) => {
      setError(event.payload);
      setJoining(null);
    });

    refreshRooms();

    // 定时心跳保活
    const heartbeatInterval = setInterval(() => {
      invoke("send_ping").catch(() => {});
    }, 10000);

    // 每5秒自动刷新房间列表
    const autoRefreshInterval = setInterval(() => {
      invoke("list_rooms").catch(() => {});
    }, 5000);

    // 每秒更新时间（用于倒计时显示）
    const tickInterval = setInterval(() => {
      setNowSec(Math.floor(Date.now() / 1000));
    }, 1000);

    return () => {
      unlistenRoomList.then((f) => f());
      unlistenCreateOk.then((f) => f());
      unlistenCreateFailed.then((f) => f());
      unlistenJoinOk.then((f) => f());
      unlistenJoinFailed.then((f) => f());
      clearInterval(heartbeatInterval);
      clearInterval(autoRefreshInterval);
      clearInterval(tickInterval);
    };
  }, []);

  async function refreshRooms() {
    manualRefreshRef.current = true;
    setLoading(true);
    setError(null);
    try {
      await invoke("list_rooms");
    } catch (e: any) {
      setError(e.toString());
      setLoading(false);
    }
  }

  async function handleCreateRoom() {
    if (!newRoomId.trim()) {
      setError("请输入房间名称");
      return;
    }
    const mp = Math.min(50, Math.max(1, parseInt(maxPlayers) || 20));
    maxPlayersRef.current = mp;
    setError(null);
    try {
      await invoke("create_room", { roomId: newRoomId.trim(), maxPlayers: mp });
    } catch (e: any) {
      setError(e.toString());
    }
  }

  async function handleJoinRoom(roomId: string) {
    setJoining(roomId);
    setError(null);
    try {
      await invoke("join_room", { roomId });
    } catch (e: any) {
      setError(e.toString());
      setJoining(null);
    }
  }

  return (
    <div className="container">
      <div className="page-header">
        <button className="back-btn" onClick={onBack}>← 返回</button>
        <h2>房间列表</h2>
        <span className="nickname-badge">{nickname}</span>
      </div>

      <div className="toolbar">
        <button className="refresh-btn" onClick={refreshRooms} disabled={loading}>
          🔄 刷新
        </button>
        <button className="create-btn" onClick={() => setShowCreate(!showCreate)}>
          ＋ 创建房间
        </button>
      </div>

      {showCreate && (
        <div className="create-room-form">
          <div className="create-room-labels">
            <span style={{ flex: 4 }}>房间名称</span>
            <span style={{ flex: 1 }}>人数上限</span>
            <span style={{ minWidth: 70 }}></span>
          </div>
          <div className="create-room-inputs">
            <input
              type="text"
              value={newRoomId}
              onChange={(e) => setNewRoomId(e.target.value)}
              placeholder="输入房间名称"
              style={{ flex: 4 }}
            />
            <input
              type="number"
              value={maxPlayers}
              onChange={(e) => {
                const val = parseInt(e.target.value) || 0;
                setMaxPlayers(val > 50 ? "50" : e.target.value);
              }}
              placeholder="输入上限"
              min={1}
              max={50}
              style={{ flex: 1, minWidth: 0 }}
            />
            <button onClick={handleCreateRoom}>创建</button>
          </div>
        </div>
      )}

      {error && <div className="error-msg">{error}</div>}

      <div className="room-list">
        {rooms.length === 0 && !loading && (
          <div className="empty-state">暂无房间，点击上方创建一个吧</div>
        )}
        {rooms.map((room) => {
          const remaining = room.player_count === 0 && room.disband_at > 0
            ? Math.max(0, room.disband_at - nowSec)
            : -1;
          return (
            <div key={room.room_id} className="room-card">
              <div className="room-info">
                <span className="room-name">{room.room_id}</span>
                <span className="room-players">
                  {room.player_count} / {room.max_players} 人
                  {remaining >= 0 && <span className="room-disband-hint">（空房间将于{remaining}秒后自动解散）</span>}
                </span>
              </div>
              <button
                className="join-btn"
                onClick={() => handleJoinRoom(room.room_id)}
                disabled={joining !== null}
            >
              {joining === room.room_id ? "加入中..." : "加入"}
            </button>
          </div>
          );
        })}
      </div>

      {showToast && <div className="toast">刷新成功</div>}
    </div>
  );
}
