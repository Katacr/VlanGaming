import { useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import "./App.css";
import HomePage from "./pages/HomePage";
import RoomListPage from "./pages/RoomListPage";
import RoomPage from "./pages/RoomPage";

export type Page = "home" | "rooms" | "room";

async function resizeWindow(width: number, height: number) {
  const win = getCurrentWindow();
  await win.setResizable(true);
  await win.setSize(new LogicalSize(width, height));
  await win.center();
  await win.setResizable(false);
}

function App() {
  const [page, setPage] = useState<Page>("home");
  const [nickname, setNickname] = useState("");
  const [currentRoom, setCurrentRoom] = useState("");
  const [virtualIp, setVirtualIp] = useState("");

  return (
    <div className="app">
      {page === "home" && (
        <HomePage
          onConnected={(nick) => {
            setNickname(nick);
            setPage("rooms");
          }}
        />
      )}
      {page === "rooms" && (
        <RoomListPage
          nickname={nickname}
          onJoinRoom={(roomId, vip) => {
            setCurrentRoom(roomId);
            setVirtualIp(vip);
            setPage("room");
            resizeWindow(900, 600);
          }}
          onBack={() => setPage("home")}
        />
      )}
      {page === "room" && (
        <RoomPage
          nickname={nickname}
          roomId={currentRoom}
          virtualIp={virtualIp}
          onLeave={() => {
            setPage("rooms");
            resizeWindow(400, 600);
          }}
        />
      )}
    </div>
  );
}

export default App;
