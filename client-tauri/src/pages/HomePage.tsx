import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onConnected: (nickname: string) => void;
}

function generateNickname(): string {
  const num = Math.floor(1000 + Math.random() * 9000);
  return `玩家${num}`;
}

export default function HomePage({ onConnected }: Props) {
  const [serverIp, setServerIp] = useState("");
  const [port, setPort] = useState("9876");
  const [nickname, setNickname] = useState(generateNickname);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [historyIps, setHistoryIps] = useState<string[]>([]);
  const [showHistory, setShowHistory] = useState(false);

  useEffect(() => {
    const saved = localStorage.getItem("history_ips");
    if (saved) {
      setHistoryIps(JSON.parse(saved));
    }
  }, []);

  function saveIpToHistory(ip: string) {
    const updated = [ip, ...historyIps.filter((h) => h !== ip)].slice(0, 10);
    setHistoryIps(updated);
    localStorage.setItem("history_ips", JSON.stringify(updated));
  }

  async function handleConnect() {
    if (!serverIp.trim()) {
      setError("请输入服务器 IP");
      return;
    }
    if (!port.trim()) {
      setError("请输入端口");
      return;
    }
    if (!nickname.trim()) {
      setError("请输入昵称");
      return;
    }

    setConnecting(true);
    setError(null);

    try {
      const result: any = await invoke("connect_server", {
        params: {
          server_ip: serverIp.trim(),
          port: port.trim(),
          nickname: nickname.trim(),
        },
      });

      if (result.success) {
        saveIpToHistory(serverIp.trim());
        onConnected(nickname.trim());
      } else {
        setError(result.error || "连接失败");
      }
    } catch (e: any) {
      setError(e.toString());
    } finally {
      setConnecting(false);
    }
  }

  return (
    <div className="container">
      <h1 className="title">VLan Gaming</h1>
      <p className="subtitle">虚拟局域网游戏联机</p>

      <div className="form">
        <div className="input-group">
          <label>服务器 IP</label>
          <div className="input-with-history">
            <input
              type="text"
              value={serverIp}
              onChange={(e) => setServerIp(e.target.value)}
              placeholder="输入服务器 IP 地址"
              disabled={connecting}
            />
            {historyIps.length > 0 && (
              <button
                className="history-btn"
                onClick={() => setShowHistory(!showHistory)}
                title="历史记录"
              >
                ▼
              </button>
            )}
          </div>
          {showHistory && historyIps.length > 0 && (
            <ul className="history-list">
              {historyIps.map((ip) => (
                <li key={ip} onClick={() => { setServerIp(ip); setShowHistory(false); }}>
                  {ip}
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="input-group">
          <label>端口</label>
          <input
            type="text"
            value={port}
            onChange={(e) => setPort(e.target.value)}
            placeholder="9876"
            disabled={connecting}
          />
        </div>

        <div className="input-group">
          <label>昵称</label>
          <input
            type="text"
            value={nickname}
            onChange={(e) => setNickname(e.target.value)}
            placeholder="输入你的昵称"
            disabled={connecting}
          />
        </div>

        {error && <div className="error-msg">{error}</div>}

        <button
          className="connect-btn"
          onClick={handleConnect}
          disabled={connecting}
        >
          {connecting ? "连接中..." : "连接服务器"}
        </button>
      </div>
    </div>
  );
}
