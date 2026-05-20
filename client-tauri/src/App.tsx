import { useState } from "react";
import "./App.css";
import HomePage from "./pages/HomePage";
import RoomListPage from "./pages/RoomListPage";
import RoomPage from "./pages/RoomPage";

export type Page = "home" | "rooms" | "room";

function App() {
  const [page, setPage] = useState<Page>("home");
  const [nickname, setNickname] = useState("");
  const [currentRoom, setCurrentRoom] = useState("");
  const [virtualIp, setVirtualIp] = useState("");
  const [maxPlayers, setMaxPlayers] = useState(20);

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
          onJoinRoom={(roomId, vip, mp) => {
            setCurrentRoom(roomId);
            setVirtualIp(vip);
            setMaxPlayers(mp);
            setPage("room");
          }}
          onBack={() => setPage("home")}
        />
      )}
      {page === "room" && (
        <RoomPage
          nickname={nickname}
          roomId={currentRoom}
          virtualIp={virtualIp}
          maxPlayers={maxPlayers}
          onLeave={() => setPage("rooms")}
        />
      )}
    </div>
  );
}

export default App;
