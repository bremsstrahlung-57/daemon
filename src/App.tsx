import type { MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import daemon from "./assets/clippy.png";
import "./App.css";

function App() {
  const startDragging = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    if (!("__TAURI_INTERNALS__" in window)) {
      return;
    }

    event.preventDefault();
    void getCurrentWindow().startDragging();
  };

  return (
    <main className="window">
      <div
        data-tauri-drag-region
        className="drag-region"
        onMouseDown={startDragging}
      >
        <img src={daemon} alt="Daemon" className="daemon-img" />
      </div>
    </main>
  );
}

export default App;
