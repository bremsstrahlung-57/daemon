import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import idleDaemon from "./assets/clippy.png";
import speakingDaemon from "./assets/clippy2.png";
import waitingDaemon from "./assets/clippy3.png";
import "./App.css";

type CompanionPhase = "idle" | "speaking" | "waiting" | "dismissed";

const PROMPT = "Ask Daemon";
const SILENCE_MIN_MS = 18_000;
const SILENCE_MAX_MS = 70_000;
const SPEECH_VISIBLE_MS = 6_500;
const PROMPT_CHANCE = 0.35;
const PROMPT_IDLE_MS = 18_000;

const randomBetween = (min: number, max: number) =>
  Math.floor(Math.random() * (max - min + 1)) + min;

function App() {
  const canUseTauriWindow = "__TAURI_INTERNALS__" in window;
  const [phase, setPhase] = useState<CompanionPhase>("idle");
  const [isRendered, setIsRendered] = useState(true);
  const [line, setLine] = useState("");
  const [messageKey, setMessageKey] = useState(0);
  const [prompt, setPrompt] = useState("");
  const [isAsking, setIsAsking] = useState(false);
  const [silenceTick, setSilenceTick] = useState(0);

  const containerRef = useRef<HTMLDivElement>(null);
  const dragStartRef = useRef<{ x: number; y: number } | null>(null);
  const didDragRef = useRef(false);
  const phaseRef = useRef(phase);
  const isRenderedRef = useRef(isRendered);
  const isAskingRef = useRef(isAsking);
  const promptAfterSpeechRef = useRef(false);
  const requestIdRef = useRef(0);
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

  const isWindowAvailable = useCallback(async () => {
    if (!isRenderedRef.current) {
      return false;
    }

    if (!canUseTauriWindow) {
      return document.visibilityState === "visible";
    }

    try {
      const currentWindow = getCurrentWindow();
      const [isMinimized, isVisible] = await Promise.all([
        currentWindow.isMinimized(),
        currentWindow.isVisible(),
      ]);

      return isVisible && !isMinimized && isRenderedRef.current;
    } catch {
      return isRenderedRef.current;
    }
  }, [canUseTauriWindow]);

  const showAiLine = useCallback((nextLine: string, canAskAfter = false) => {
    const trimmedLine = nextLine.trim();
    if (!trimmedLine) {
      return;
    }

    promptAfterSpeechRef.current = canAskAfter;
    setLine(trimmedLine);
    setMessageKey((key) => key + 1);
    setPhase("speaking");
  }, []);

  const queueSilenceRetry = useCallback(() => {
    setTimer(
      () => {
        setSilenceTick((tick) => tick + 1);
      },
      randomBetween(SILENCE_MIN_MS, SILENCE_MAX_MS),
    );
  }, [setTimer]);

  const generateSpontaneousLine = useCallback(async () => {
    if (phaseRef.current !== "idle" || isAskingRef.current) {
      return;
    }

    if (!(await isWindowAvailable())) {
      queueSilenceRetry();
      return;
    }

    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    try {
      const response = await invoke<string>("next_daemon_line");
      if (
        requestId === requestIdRef.current &&
        phaseRef.current === "idle" &&
        !isAskingRef.current &&
        (await isWindowAvailable())
      ) {
        showAiLine(response, true);
      }
    } catch {
      // Proactive lines should fail silently; direct asks surface errors.
      queueSilenceRetry();
    }
  }, [isWindowAvailable, queueSilenceRetry, showAiLine]);

  const dismiss = useCallback(() => {
    clearTimers();
    requestIdRef.current += 1;
    setIsAsking(false);
    setPrompt("");
    setLine("");
    setPhase("dismissed");
    setIsRendered(false);
  }, [clearTimers]);

  const beginConversation = useCallback(() => {
    clearTimers();
    requestIdRef.current += 1;
    isRenderedRef.current = true;

    setIsRendered(true);
    setLine("");
    setPhase("idle");

    setTimer(() => {
      void generateSpontaneousLine();
    }, 220);
  }, [clearTimers, generateSpontaneousLine, setTimer]);

  const askDaemon = async () => {
    const nextPrompt = prompt.trim();
    if (!nextPrompt || isAsking) {
      return;
    }

    clearTimers();
    requestIdRef.current += 1;
    setIsAsking(true);

    try {
      const response = await invoke<string>("ask_ai", { prompt: nextPrompt });
      setPrompt("");
      showAiLine(response, false);
    } catch (error) {
      showAiLine(error instanceof Error ? error.message : String(error), false);
    } finally {
      setIsAsking(false);
    }
  };

  const handleMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    const target = event.target;
    if (
      target instanceof HTMLElement &&
      target.closest("button, input, textarea")
    ) {
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
    phaseRef.current = phase;
  }, [phase]);

  useEffect(() => {
    isRenderedRef.current = isRendered;
  }, [isRendered]);

  useEffect(() => {
    isAskingRef.current = isAsking;
  }, [isAsking]);

  useEffect(() => {
    if (phase !== "idle" || !isRendered || isAsking) {
      return;
    }

    const timer = window.setTimeout(
      () => {
        void generateSpontaneousLine();
      },
      randomBetween(SILENCE_MIN_MS, SILENCE_MAX_MS),
    );

    return () => window.clearTimeout(timer);
  }, [generateSpontaneousLine, isAsking, isRendered, phase, silenceTick]);

  useEffect(() => {
    if (phase !== "speaking") {
      return;
    }

    const timer = window.setTimeout(() => {
      setPhase(
        promptAfterSpeechRef.current && Math.random() < PROMPT_CHANCE
          ? "waiting"
          : "idle",
      );
    }, SPEECH_VISIBLE_MS);

    return () => window.clearTimeout(timer);
  }, [phase]);

  useEffect(() => {
    if (phase !== "waiting" || isAsking) {
      return;
    }

    const timer = window.setTimeout(() => {
      setPhase("idle");
    }, PROMPT_IDLE_MS);

    return () => window.clearTimeout(timer);
  }, [isAsking, phase]);

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
              <p>{line}</p>
            </div>
          )}

          {phase === "waiting" && (
            <div className="interactive-card">
              <div className="title-bar">
                <div className="title-bar-text">Daemon</div>
                <button
                  type="button"
                  className="title-bar-close"
                  aria-label="Dismiss"
                  onClick={dismiss}
                >
                  x
                </button>
              </div>
              <div className="window-body">
                <label className="card-prompt" htmlFor="daemon-prompt">
                  {PROMPT}
                </label>
                <form
                  className="prompt-form"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void askDaemon();
                  }}
                >
                  <textarea
                    id="daemon-prompt"
                    className="prompt-input"
                    value={prompt}
                    rows={3}
                    placeholder="What should I do next?"
                    onChange={(event) => setPrompt(event.target.value)}
                  />
                  <div className="button-row">
                    <button
                      type="submit"
                      className="ask-button"
                      disabled={!prompt.trim() || isAsking}
                    >
                      {isAsking ? "Thinking" : "Ask"}
                    </button>
                  </div>
                </form>
              </div>
            </div>
          )}
        </section>
      )}
    </main>
  );
}

export default App;
