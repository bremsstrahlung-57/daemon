import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type FormEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import Mascot from "./components/Mascot";
import ProviderToolbox, { type ToolboxSection } from "./components/ProviderToolbox";
import {
  showToolboxMenu,
  submitConversationTurn,
} from "./lib/daemon";
import {
  onMessageReady,
  onMessageDelta,
} from "./lib/events";
import "./App.css";

type CompanionPhase =
  | "idle"
  | "listening"
  | "speaking"
  | "waiting"
  | "completed"
  | "failed"
  | "sleeping"
  | "dismissed";

const PROMPT = "I’m here. What’s been on your mind?";
const visibleDuration = (text: string) => {
  if (text.length < 20) {
    return 3_000;
  }

  if (text.length < 50) {
    return 7_000;
  }

  return 20_000;
};
const PROMPT_VISIBLE_MS = 10_000;

const invocationErrorMessage = (error: unknown) => {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "I couldn’t reach the selected AI provider.";
};

function App() {
  const canUseTauriWindow = "__TAURI_INTERNALS__" in window;
  const [phase, setPhase] = useState<CompanionPhase>("idle");
  const [isRendered, setIsRendered] = useState(true);
  const [line, setLine] = useState("I’m here.");
  const [messageKey, setMessageKey] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const [input, setInput] = useState("");
  const [conversationId, setConversationId] = useState<string | undefined>();
  const [isJobRunning] = useState(false);
  const [toolboxSection, setToolboxSection] = useState<ToolboxSection | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const dragStartRef = useRef<{ x: number; y: number } | null>(null);
  const didDragRef = useRef(false);
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

  const resetPromptTimer = useCallback(() => {
    clearTimers();
    setTimer(() => setPhase("idle"), PROMPT_VISIBLE_MS);
  }, [clearTimers, setTimer]);

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
  }, [clearTimers]);

  const beginConversation = useCallback(() => {
    clearTimers();

    setIsRendered(true);
    setLine("I’m listening.");
    setPhase("listening");

    setTimer(() => {
      setPhase("waiting");
      setTimer(() => setPhase("idle"), PROMPT_VISIBLE_MS);
    }, 500);
  }, [clearTimers, setTimer]);

  const submitMessage = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const content = input.trim();
    if (!content) {
      return;
    }

    clearTimers();
    setPhase("listening");
    setLine("");
    setInput("");
    try {
      const result = await submitConversationTurn({
        content,
        conversationId,
      });
      setConversationId(result.conversation_id);
    } catch (error) {
      const message = invocationErrorMessage(error);
      setLine(message);
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration(message));
    }
  };

  const handleMessageReady = useCallback(
    (payload: { content: string }) => {
      clearTimers();
      setLine(payload.content);
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration(payload.content));
    },
    [clearTimers, setTimer],
  );

  const handleMessageDelta = useCallback((payload: { content: string }) => {
    setLine((current) => current + payload.content);
    setPhase("speaking");
  }, []);

  const handleMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button, input, summary")) {
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
    setIsDragging(true);
    event.preventDefault();
    void getCurrentWindow().startDragging();
  };

  const handleMouseUp = () => {
    dragStartRef.current = null;
    setIsDragging(false);
  };

  const handleDaemonClick = () => {
    if (didDragRef.current) {
      didDragRef.current = false;
      return;
    }

    if (phase === "idle" || phase === "sleeping" || phase === "dismissed") {
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

  useEffect(() => {
    if (phase === "idle") {
      const timer = window.setTimeout(() => setPhase("sleeping"), 10000);
      return () => window.clearTimeout(timer);
    }
    return undefined;
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

    void onMessageReady(handleMessageReady).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void onMessageDelta(handleMessageDelta).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void listen<ToolboxSection>("daemon://toolbox-open", (event) => {
      setToolboxSection(event.payload);
    }).then((unlisten) => cleanups.push(unlisten));

    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [
    beginConversation,
    canUseTauriWindow,
    dismiss,
    handleMessageReady,
    handleMessageDelta,
  ]);

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
          onContextMenu={(event) => {
            event.preventDefault();
            void showToolboxMenu();
          }}
        >
          <div
            aria-label="Daemon"
            className="daemon-core"
            role="button"
            tabIndex={0}
            onClick={handleDaemonClick}
            onKeyDown={handleDaemonKeyDown}
          >
            <Mascot
              isVisible={isRendered}
              state={
                isDragging
                  ? "dragged"
                  : phase === "speaking"
                    ? "speaking"
                    : phase === "listening"
                      ? "listening"
                    : phase === "sleeping"
                      ? "sleeping"
                      : isJobRunning
                        ? "working"
                        : phase === "completed"
                          ? "completed"
                          : phase === "failed"
                            ? "failed"
                        : "idle"
              }
            />
          </div>

          <div className="companion-panel">
            {(phase === "speaking" || phase === "completed" || phase === "failed") && (
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
              <form className="message-form" onSubmit={submitMessage}>
                <input
                  autoFocus
                  aria-label="Message Daemon"
                  value={input}
                  onFocus={resetPromptTimer}
                  onChange={(event) => {
                    setInput(event.target.value);
                    resetPromptTimer();
                  }}
                  placeholder="Say it however you want…"
                />
                <button type="submit" className="choice-button" aria-label="Send message">
                  ↵
                </button>
              </form>
            </div>
            )}

            {toolboxSection && (
              <ProviderToolbox section={toolboxSection} onClose={() => setToolboxSection(null)} />
            )}
          </div>
        </section>
      )}
    </main>
  );
}

export default App;
