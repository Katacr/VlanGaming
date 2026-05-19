import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onConnected: (nickname: string) => void;
}

const BUILTIN_IP = "114.66.25.225";
const BUILTIN_PORT = "9876";

function generateNickname(): string {
  const saved = localStorage.getItem("last_nickname");
  if (saved) return saved;
  const num = Math.floor(1000 + Math.random() * 9000);
  return `玩家${num}`;
}

function pingColor(ms: number): string {
  if (ms <= 50) return "#4caf50";
  if (ms <= 150) return "#ff9800";
  return "#f44336";
}

export default function HomePage({ onConnected }: Props) {
  const [serverMode, setServerMode] = useState<"builtin" | "custom">("builtin");
  const [serverIp, setServerIp] = useState("");
  const [port, setPort] = useState("9876");
  const [nickname, setNickname] = useState(generateNickname);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [historyIps, setHistoryIps] = useState<string[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const [builtinPing, setBuiltinPing] = useState<number | null>(null);
  const [pingError, setPingError] = useState(false);

  useEffect(() => {
    const saved = localStorage.getItem("history_ips");
    if (saved) {
      setHistoryIps(JSON.parse(saved));
    }
    // 自动 ping 内置服务器
    measurePing();
    const interval = setInterval(measurePing, 5000);
    return () => clearInterval(interval);
  }, []);

  async function measurePing() {
    try {
      const ms: number = await invoke("ping_server_latency", { ip: BUILTIN_IP, port: BUILTIN_PORT });
      setBuiltinPing(ms);
      setPingError(false);
    } catch {
      setBuiltinPing(null);
      setPingError(true);
    }
  }

  function saveIpToHistory(ip: string) {
    const updated = [ip, ...historyIps.filter((h) => h !== ip)].slice(0, 10);
    setHistoryIps(updated);
    localStorage.setItem("history_ips", JSON.stringify(updated));
  }

  async function handleConnect() {
    const ip = serverMode === "builtin" ? BUILTIN_IP : serverIp.trim();
    const p = serverMode === "builtin" ? BUILTIN_PORT : port.trim();

    if (!ip) {
      setError("请输入服务器 IP");
      return;
    }
    if (!p) {
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
          server_ip: ip,
          port: p,
          nickname: nickname.trim(),
        },
      });

      if (result.success) {
        if (serverMode === "custom") {
          saveIpToHistory(ip);
        }
        localStorage.setItem("last_nickname", nickname.trim());
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
          <label>服务器</label>
          <div className="server-tabs">
            <button
              className={`server-tab ${serverMode === "builtin" ? "active" : ""}`}
              onClick={() => setServerMode("builtin")}
              disabled={connecting}
            >
              内置服务器{" "}
              {pingError ? (
                <span style={{ color: "#f44336" }}>离线</span>
              ) : builtinPing !== null ? (
                <span style={{ color: pingColor(builtinPing) }}>{builtinPing}ms</span>
              ) : (
                <span style={{ color: "#999" }}>测速中...</span>
              )}
            </button>
            <button
              className={`server-tab ${serverMode === "custom" ? "active" : ""}`}
              onClick={() => setServerMode("custom")}
              disabled={connecting}
            >
              自定义
            </button>
          </div>
        </div>

        {serverMode === "custom" && (
          <>
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
          </>
        )}

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
