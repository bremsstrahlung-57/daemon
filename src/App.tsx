import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import idleDaemon from "./assets/clippy.png";
import speakingDaemon from "./assets/clippy2.png";
import waitingDaemon from "./assets/clippy3.png";
import "./App.css";

type CompanionPhase = "idle" | "speaking" | "waiting" | "dismissed";

type Choice = {
  id: string;
  label: string;
  response: string;
};

const AMBIENT_LINES = [
  "I found a loose thread in the room.",
  "Small check-in. Nothing dramatic.",
  "Your desk has entered thoughtful weather.",
];

const PROMPT = "What should I keep an eye on next?";

const CHOICES: Choice[] = [
  {
    id: "focus",
    label: "Focus",
    response: "Good. I will keep the edges quiet for a while.",
  },
  {
    id: "remind",
    label: "Remind me",
    response: "Noted. I will bring it back when the moment feels less crowded.",
  },
  {
    id: "ignore",
    label: "Ignore",
    response: "Understood. I will let this one drift away.",
  },
];

function App() {
  const canUseTauriWindow = "__TAURI_INTERNALS__" in window;
  const [phase, setPhase] = useState<CompanionPhase>("idle");
  const [isRendered, setIsRendered] = useState(true);
  const [line, setLine] = useState(AMBIENT_LINES[0]);
  const [messageKey, setMessageKey] = useState(0);
  const [selectedChoice, setSelectedChoice] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const dragStartRef = useRef<{ x: number; y: number } | null>(null);
  const didDragRef = useRef(false);
  const lineIndexRef = useRef(0);
  const timersRef = useRef<number[]>([]);

  const clearTimers = useCallback(() => {
    timersRef.current.forEach((timer) => window.clearTimeout(timer));
    timersRef.current = [];
  }, []);

  const setTimer = useCallback((callback: () => void, delay: number) => {
    const timer = window.setTimeout(() => {
      timersRef.current = timersRef.current.filter((item) => item !== timer);
      callback();
    }, delay);

    timersRef.current.push(timer);
  }, []);

  const resizeToContent = useCallback(() => {
    if (!containerRef.current || !canUseTauriWindow) {
      return;
    }

    const rect = containerRef.current.getBoundingClientRect();
    const width = Math.max(1, Math.ceil(rect.width));
    const height = Math.max(1, Math.ceil(rect.height));

    void getCurrentWindow()
      .setSize(new LogicalSize(width, height))
      .catch(() => undefined);
  }, [canUseTauriWindow]);

  const dismiss = useCallback(() => {
    clearTimers();
    setPhase("idle");
    setSelectedChoice(null);
  }, [clearTimers]);

  const beginConversation = useCallback(() => {
    clearTimers();

    const nextLine = AMBIENT_LINES[lineIndexRef.current % AMBIENT_LINES.length];
    lineIndexRef.current += 1;

    setIsRendered(true);
    setSelectedChoice(null);
    setLine(nextLine);
    setMessageKey((key) => key + 1);
    setPhase("idle");

    setTimer(() => {
      setPhase("speaking");
      setMessageKey((key) => key + 1);
    }, 220);

    setTimer(() => {
      setPhase("waiting");
    }, 2700);
  }, [clearTimers, setTimer]);

  const respondToChoice = (choice: Choice) => {
    clearTimers();
    setSelectedChoice(choice.id);
    setLine(choice.response);
    setMessageKey((key) => key + 1);
    setPhase("speaking");

    setTimer(() => {
      setPhase("idle");
    }, 2600);
  };

  const handleMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button")) {
      return;
    }

    didDragRef.current = false;
    dragStartRef.current = { x: event.clientX, y: event.clientY };
  };

  const handleMouseMove = (event: MouseEvent<HTMLElement>) => {
    if (!dragStartRef.current || (event.buttons & 1) !== 1) {
      return;
    }

    const deltaX = event.clientX - dragStartRef.current.x;
    const deltaY = event.clientY - dragStartRef.current.y;

    if (Math.hypot(deltaX, deltaY) < 5 || !canUseTauriWindow) {
      return;
    }

    didDragRef.current = true;
    dragStartRef.current = null;
    event.preventDefault();
    void getCurrentWindow().startDragging();
  };

  const handleMouseUp = () => {
    dragStartRef.current = null;
  };

  const handleDaemonClick = () => {
    if (didDragRef.current) {
      didDragRef.current = false;
      return;
    }

    if (phase === "idle" || phase === "dismissed") {
      beginConversation();
    }
  };

  const handleDaemonKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }

    event.preventDefault();
    beginConversation();
  };

  const daemonImage =
    phase === "waiting"
      ? waitingDaemon
      : phase === "speaking"
        ? speakingDaemon
        : idleDaemon;

  useEffect(() => {
    let timer: number;
    if (phase === "idle") {
      timer = window.setTimeout(() => {
        setPhase("dismissed");
        setTimer(() => setIsRendered(false), 260);
      }, 10000);
    }
    return () => window.clearTimeout(timer);
  }, [phase, setTimer]);

  useEffect(() => {
    if (!containerRef.current || !canUseTauriWindow) {
      return;
    }

    const observer = new ResizeObserver(() => resizeToContent());
    observer.observe(containerRef.current);

    return () => observer.disconnect();
  }, [canUseTauriWindow, resizeToContent]);

  useEffect(() => {
    const frame = window.requestAnimationFrame(resizeToContent);
    return () => window.cancelAnimationFrame(frame);
  }, [isRendered, line, phase, resizeToContent]);

  useEffect(() => {
    if (!canUseTauriWindow) {
      return;
    }

    let disposed = false;
    const cleanups: Array<() => void> = [];

    void listen("daemon://trigger", beginConversation).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void listen("daemon://dismiss", dismiss).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [beginConversation, canUseTauriWindow, dismiss]);

  return (
    <main
      ref={containerRef}
      className={`daemon-container phase-${phase} ${
        isRendered ? "is-rendered" : "is-hidden"
      }`}
    >
      {isRendered && (
        <section
          className="companion-shell"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
        >
          <div
            aria-label="Daemon"
            className="daemon-core"
            role="button"
            tabIndex={0}
            onClick={handleDaemonClick}
            onKeyDown={handleDaemonKeyDown}
          >
            <img src={daemonImage} alt="" className="daemon-img" />
          </div>

          {phase === "speaking" && (
            <div key={messageKey} className="ambient-line">
              {line}
            </div>
          )}

          {phase === "waiting" && (
            <div className="interactive-card">
              <button
                type="button"
                className="dismiss-button"
                aria-label="Dismiss"
                onClick={dismiss}
              >
                x
              </button>
              <p className="card-prompt">{PROMPT}</p>
              <div className="choice-list">
                {CHOICES.map((choice) => (
                  <button
                    key={choice.id}
                    type="button"
                    className={`choice-button ${
                      selectedChoice === choice.id ? "is-selected" : ""
                    }`}
                    onClick={() => respondToChoice(choice)}
                  >
                    {choice.label}
                  </button>
                ))}
              </div>
            </div>
          )}
        </section>
      )}
    </main>
  );
}

export default App;
