import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface RoomInfo {
  room_id: string;
  player_count: number;
}

interface Props {
  nickname: string;
  onJoinRoom: (roomId: string, virtualIp: string) => void;
  onBack: () => void;
}

export default function RoomListPage({ nickname, onJoinRoom, onBack }: Props) {
  const [rooms, setRooms] = useState<RoomInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [newRoomId, setNewRoomId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [joining, setJoining] = useState<string | null>(null);
  const joiningRef = useRef<string | null>(null);
  const onJoinRoomRef = useRef(onJoinRoom);
  onJoinRoomRef.current = onJoinRoom;

  useEffect(() => {
    joiningRef.current = joining;
  }, [joining]);

  useEffect(() => {
    const unlistenRoomList = listen<any>("room-list", (event) => {
      setRooms(event.payload.rooms || []);
      setLoading(false);
    });

    const unlistenCreateOk = listen<{ room_id: string; virtual_ip: string }>("create-room-ok", (event) => {
      setShowCreate(false);
      setNewRoomId("");
      onJoinRoomRef.current(event.payload.room_id, event.payload.virtual_ip);
    });

    const unlistenCreateFailed = listen<string>("create-room-failed", (event) => {
      setError(event.payload);
    });

    const unlistenJoinOk = listen<string>("join-room-ok", (event) => {
      const roomId = joiningRef.current;
      if (roomId) {
        onJoinRoomRef.current(roomId, event.payload);
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

    return () => {
      unlistenRoomList.then((f) => f());
      unlistenCreateOk.then((f) => f());
      unlistenCreateFailed.then((f) => f());
      unlistenJoinOk.then((f) => f());
      unlistenJoinFailed.then((f) => f());
      clearInterval(heartbeatInterval);
    };
  }, []);

  async function refreshRooms() {
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
    setError(null);
    try {
      await invoke("create_room", { roomId: newRoomId.trim() });
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
          {loading ? "刷新中..." : "🔄 刷新"}
        </button>
        <button className="create-btn" onClick={() => setShowCreate(!showCreate)}>
          ＋ 创建房间
        </button>
      </div>

      {showCreate && (
        <div className="create-room-form">
          <input
            type="text"
            value={newRoomId}
            onChange={(e) => setNewRoomId(e.target.value)}
            placeholder="输入房间名称"
          />
          <button onClick={handleCreateRoom}>创建</button>
        </div>
      )}

      {error && <div className="error-msg">{error}</div>}

      <div className="room-list">
        {rooms.length === 0 && !loading && (
          <div className="empty-state">暂无房间，点击上方创建一个吧</div>
        )}
        {rooms.map((room) => (
          <div key={room.room_id} className="room-card">
            <div className="room-info">
              <span className="room-name">{room.room_id}</span>
              <span className="room-players">{room.player_count} 人在线</span>
            </div>
            <button
              className="join-btn"
              onClick={() => handleJoinRoom(room.room_id)}
              disabled={joining !== null}
            >
              {joining === room.room_id ? "加入中..." : "加入"}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
