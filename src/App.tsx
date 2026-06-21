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

type WindowSize = {
  width: number;
  height: number;
};

type DragStart = {
  x: number;
  y: number;
};

const PROMPT = "Ask Daemon";
const SILENCE_MIN_MS = 18_000;
const SILENCE_MAX_MS = 70_000;
const SPEECH_VISIBLE_MS = 6_500;
const PROMPT_CHANCE = 0.35;
const PROMPT_IDLE_MS = 18_000;
const WORD_STREAM_MS = 120;
const DRAG_THRESHOLD_PX = 5;
const INITIAL_LINE_RESIZE_MS = 50;

const SPEAKING_WINDOW: WindowSize = {
  width: 368,
  height: 392,
};

const randomBetween = (min: number, max: number) =>
  Math.floor(Math.random() * (max - min + 1)) + min;

const canUseTauriWindow = () => "__TAURI_INTERNALS__" in window;

function measureElement(element: HTMLElement): WindowSize {
  const rect = element.getBoundingClientRect();

  return {
    width: Math.max(1, Math.ceil(rect.width), Math.ceil(element.scrollWidth)),
    height: Math.max(
      1,
      Math.ceil(rect.height),
      Math.ceil(element.scrollHeight),
    ),
  };
}

async function resizeWindowFromBottomRight(
  nextSize: WindowSize,
  previousSize: WindowSize | null,
) {
  const win = getCurrentWindow();

  if (previousSize) {
    const factor = await win.scaleFactor();
    const pos = await win.outerPosition();
    pos.x -= Math.round((nextSize.width - previousSize.width) * factor);
    pos.y -= Math.round((nextSize.height - previousSize.height) * factor);
    await win.setPosition(pos);
  }

  await win.setSize(new LogicalSize(nextSize.width, nextSize.height));
}

async function reserveSpeakingWindow(previousSize: WindowSize | null) {
  if (!canUseTauriWindow()) {
    return;
  }

  const win = getCurrentWindow();
  const factor = await win.scaleFactor();
  const outerSize = await win.outerSize();
  const currentSize = previousSize ?? {
    width: outerSize.width / factor,
    height: outerSize.height / factor,
  };

  if (
    currentSize.width >= SPEAKING_WINDOW.width &&
    currentSize.height >= SPEAKING_WINDOW.height
  ) {
    return;
  }

  await resizeWindowFromBottomRight(SPEAKING_WINDOW, currentSize);
}

function App() {
  const hasTauriWindow = canUseTauriWindow();
  const [phase, setPhase] = useState<CompanionPhase>("idle");
  const [isRendered, setIsRendered] = useState(true);
  const [line, setLine] = useState("");
  const [displayedLine, setDisplayedLine] = useState("");
  const [messageKey, setMessageKey] = useState(0);
  const [prompt, setPrompt] = useState("");
  const [isAsking, setIsAsking] = useState(false);
  const [silenceTick, setSilenceTick] = useState(0);

  const containerRef = useRef<HTMLDivElement>(null);
  const dragStartRef = useRef<DragStart | null>(null);
  const didDragRef = useRef(false);
  const phaseRef = useRef(phase);
  const isRenderedRef = useRef(isRendered);
  const isAskingRef = useRef(isAsking);
  const promptAfterSpeechRef = useRef(false);
  const requestIdRef = useRef(0);
  const timersRef = useRef<number[]>([]);
  const windowSizeRef = useRef<WindowSize | null>(null);

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

  const resizeToContent = useCallback(async () => {
    if (!containerRef.current || !hasTauriWindow) {
      return;
    }

    const nextSize = measureElement(containerRef.current);

    try {
      await resizeWindowFromBottomRight(nextSize, windowSizeRef.current);
      windowSizeRef.current = nextSize;
    } catch (error) {
      console.error("resize failed", error);
    }
  }, [hasTauriWindow]);

  const resizeAfterPaint = useCallback(() => {
    const frame = window.requestAnimationFrame(() => {
      void resizeToContent();
    });

    return () => window.cancelAnimationFrame(frame);
  }, [resizeToContent]);

  const isWindowAvailable = useCallback(async () => {
    if (!isRenderedRef.current) {
      return false;
    }

    if (!hasTauriWindow) {
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
  }, [hasTauriWindow]);

  const showAiLine = useCallback(
    async (nextLine: string, canAskAfter = false) => {
      const trimmedLine = nextLine.trim();
      if (!trimmedLine) {
        return;
      }

      try {
        await reserveSpeakingWindow(windowSizeRef.current);
        windowSizeRef.current = SPEAKING_WINDOW;
      } catch (error) {
        console.error("reserve speaking window failed", error);
      }

      promptAfterSpeechRef.current = canAskAfter;
      setDisplayedLine("");
      setLine(trimmedLine);
      setMessageKey((key) => key + 1);
      setPhase("speaking");
    },
    [],
  );

  const queueSilenceRetry = useCallback(() => {
    setTimer(
      () => setSilenceTick((tick) => tick + 1),
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
      const canShowResponse =
        requestId === requestIdRef.current &&
        phaseRef.current === "idle" &&
        !isAskingRef.current &&
        (await isWindowAvailable());

      if (canShowResponse) {
        await showAiLine(response, true);
      }
    } catch {
      queueSilenceRetry();
    }
  }, [isWindowAvailable, queueSilenceRetry, showAiLine]);

  const dismiss = useCallback(() => {
    clearTimers();
    requestIdRef.current += 1;
    setIsAsking(false);
    setPrompt("");
    setLine("");
    setDisplayedLine("");
    setPhase("dismissed");
    setIsRendered(false);
  }, [clearTimers]);

  const beginConversation = useCallback(() => {
    clearTimers();
    requestIdRef.current += 1;
    isRenderedRef.current = true;

    setIsRendered(true);
    setLine("");
    setDisplayedLine("");
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
      await showAiLine(response, false);
    } catch (error) {
      await showAiLine(
        error instanceof Error ? error.message : String(error),
        false,
      );
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

    if (Math.hypot(deltaX, deltaY) < DRAG_THRESHOLD_PX || !hasTauriWindow) {
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
    if (!containerRef.current || !hasTauriWindow) {
      return;
    }

    const observer = new ResizeObserver(() => void resizeToContent());
    observer.observe(containerRef.current);

    return () => observer.disconnect();
  }, [hasTauriWindow, resizeToContent]);

  useEffect(resizeAfterPaint, [
    isRendered,
    line,
    phase,
    displayedLine,
    resizeAfterPaint,
  ]);

  useEffect(() => {
    if (phase !== "speaking") {
      return;
    }

    const timer = window.setTimeout(() => {
      void resizeToContent();
    }, INITIAL_LINE_RESIZE_MS);

    return () => window.clearTimeout(timer);
  }, [phase, displayedLine, resizeToContent]);

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
    if (phase !== "speaking" || !line) {
      setDisplayedLine("");
      return;
    }

    const words = line.split(" ");
    let currentWordIndex = 0;
    setDisplayedLine(words[0] || "");

    const interval = window.setInterval(() => {
      currentWordIndex += 1;

      if (currentWordIndex >= words.length) {
        window.clearInterval(interval);
        return;
      }

      setDisplayedLine((prev) => `${prev} ${words[currentWordIndex]}`);
    }, WORD_STREAM_MS);

    return () => window.clearInterval(interval);
  }, [line, phase]);

  useEffect(() => {
    if (phase !== "speaking") {
      return;
    }

    const typingTime = line.split(" ").length * WORD_STREAM_MS;
    const totalVisibleTime = Math.max(SPEECH_VISIBLE_MS, typingTime + 2000);

    const timer = window.setTimeout(() => {
      setPhase(
        promptAfterSpeechRef.current && Math.random() < PROMPT_CHANCE
          ? "waiting"
          : "idle",
      );
    }, totalVisibleTime);

    return () => window.clearTimeout(timer);
  }, [line, phase]);

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
    if (!hasTauriWindow) {
      return;
    }

    let disposed = false;
    const cleanups: Array<() => void> = [];

    void listen("daemon://trigger", beginConversation).then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }

      cleanups.push(unlisten);
    });

    void listen("daemon://dismiss", dismiss).then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }

      cleanups.push(unlisten);
    });

    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [beginConversation, dismiss, hasTauriWindow]);

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
          {phase === "speaking" && (
            <p key={messageKey} className="ambient-line">
              {displayedLine}
            </p>
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
        </section>
      )}
    </main>
  );
}

export default App;
