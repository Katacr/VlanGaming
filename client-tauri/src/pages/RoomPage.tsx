import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface PlayerInfo {
  nickname: string;
  virtual_ip: number;
  ping_ms: number;
  is_admin: boolean;
}

interface ChatMessage {
  sender: string;
  content: string;
  timestamp: number;
}

interface Props {
  nickname: string;
  roomId: string;
  virtualIp: string;
  onLeave: () => void;
}

function formatIp(ip: number): string {
  const bytes = [
    (ip >> 24) & 0xff,
    (ip >> 16) & 0xff,
    (ip >> 8) & 0xff,
    ip & 0xff,
  ];
  return `${bytes[0]}.${bytes[1]}.${bytes[2]}.${bytes[3]}`;
}

export default function RoomPage({ nickname, roomId, virtualIp, onLeave }: Props) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [players, setPlayers] = useState<PlayerInfo[]>([]);
  const [inputMsg, setInputMsg] = useState("");
  const [myPing, setMyPing] = useState(0);
  const [isAdmin, setIsAdmin] = useState(false);
  const chatEndRef = useRef<HTMLDivElement>(null);
  const onLeaveRef = useRef(onLeave);
  onLeaveRef.current = onLeave;

  useEffect(() => {
    const unlistenChat = listen<ChatMessage>("chat-message", (event) => {
      setMessages((prev) => [...prev, event.payload]);
    });

    const unlistenPlayers = listen<{ players: PlayerInfo[] }>("player-list", (event) => {
      setPlayers(event.payload.players);
      // 检查自己是否是 admin
      const me = event.payload.players.find((p) => formatIp(p.virtual_ip) === virtualIp);
      if (me) {
        setIsAdmin(me.is_admin);
      }
    });

    const unlistenPeerJoined = listen<{ nickname: string; virtual_ip: string }>("peer-joined", (event) => {
      setMessages((prev) => [
        ...prev,
        {
          sender: "系统",
          content: `${event.payload.nickname} 加入了房间`,
          timestamp: Date.now() / 1000,
        },
      ]);
    });

    const unlistenPeerLeft = listen<{ nickname: string; virtual_ip: string }>("peer-left", (event) => {
      setMessages((prev) => [
        ...prev,
        {
          sender: "系统",
          content: `${event.payload.nickname} 离开了房间`,
          timestamp: Date.now() / 1000,
        },
      ]);
    });

    const unlistenPing = listen<number>("ping-update", (event) => {
      setMyPing(event.payload);
    });

    const unlistenKicked = listen<string>("kicked", (event) => {
      alert(`你被踢出房间: ${event.payload}`);
      onLeaveRef.current();
    });

    const unlistenDisbanded = listen<void>("room-disbanded", () => {
      alert("房间已被解散");
      onLeaveRef.current();
    });

    // 立即发一次 ping，触发服务端返回玩家列表
    invoke("send_ping").catch(() => {});

    // 定时发送 ping
    const pingInterval = setInterval(() => {
      invoke("send_ping").catch(() => {});
    }, 3000);

    return () => {
      unlistenChat.then((f) => f());
      unlistenPlayers.then((f) => f());
      unlistenPeerJoined.then((f) => f());
      unlistenPeerLeft.then((f) => f());
      unlistenPing.then((f) => f());
      unlistenKicked.then((f) => f());
      unlistenDisbanded.then((f) => f());
      clearInterval(pingInterval);
    };
  }, []);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  async function handleSendChat() {
    if (!inputMsg.trim()) return;
    try {
      await invoke("send_chat", { content: inputMsg.trim() });
      setInputMsg("");
    } catch (e) {
      console.error(e);
    }
  }

  async function handleLeave() {
    try {
      await invoke("leave_room");
      onLeave();
    } catch (e) {
      console.error(e);
      onLeave();
    }
  }

  async function handleKick(playerIp: number) {
    try {
      await invoke("kick_player", { virtualIp: playerIp });
    } catch (e) {
      console.error(e);
    }
  }

  async function handleDisband() {
    if (confirm("确定要解散房间吗？所有玩家将被移出。")) {
      try {
        await invoke("disband_room");
        onLeave();
      } catch (e) {
        console.error(e);
      }
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSendChat();
    }
  }

  return (
    <div className="room-container">
      <div className="room-header">
        <button className="back-btn" onClick={handleLeave}>← 离开房间</button>
        <h2>房间: {roomId}</h2>
        <span className="my-info">
          {nickname} | {virtualIp} | Ping: {myPing}ms
          {isAdmin && <span className="admin-badge">房主</span>}
        </span>
        {isAdmin && (
          <button className="disband-btn" onClick={handleDisband}>解散房间</button>
        )}
      </div>

      <div className="room-body">
        {/* 左侧聊天区域 */}
        <div className="chat-area">
          <div className="chat-messages">
            {messages.map((msg, i) => (
              <div
                key={i}
                className={`chat-msg ${msg.sender === "系统" ? "system" : msg.sender === nickname ? "self" : ""}`}
              >
                <span className="msg-sender">{msg.sender}</span>
                <span className="msg-content">{msg.content}</span>
              </div>
            ))}
            <div ref={chatEndRef} />
          </div>
          <div className="chat-input">
            <input
              type="text"
              value={inputMsg}
              onChange={(e) => setInputMsg(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="输入消息，按 Enter 发送"
            />
            <button onClick={handleSendChat}>发送</button>
          </div>
        </div>

        {/* 右侧玩家列表 */}
        <div className="player-panel">
          <h3>玩家列表 ({players.length})</h3>
          <div className="player-list">
            {players.map((player) => (
              <div key={player.virtual_ip} className="player-item">
                <div className="player-name">
                  {player.nickname}
                  {player.is_admin && <span className="admin-badge">房主</span>}
                  {formatIp(player.virtual_ip) === virtualIp && <span className="me-badge">我</span>}
                </div>
                <div className="player-details">
                  <span className="player-ip">{formatIp(player.virtual_ip)}</span>
                  <span className={`player-ping ${player.ping_ms < 50 ? "good" : player.ping_ms < 100 ? "ok" : "bad"}`}>
                    {player.ping_ms}ms
                  </span>
                  {isAdmin && formatIp(player.virtual_ip) !== virtualIp && (
                    <button className="kick-btn" onClick={() => handleKick(player.virtual_ip)}>踢出</button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
