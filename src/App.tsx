import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import Mascot from "./components/Mascot";
import ProviderToolbox, { type ToolboxSection } from "./components/ProviderToolbox";
import {
  showToolboxMenu,
  submitConversationTurn,
  undoNote,
  type JobRecord,
} from "./lib/daemon";
import {
  onJobCompleted,
  onJobFailed,
  onJobStarted,
  onMascotReaction,
  onMessageReady,
  onNoteCreated,
} from "./lib/events";
import "./App.css";

type CompanionPhase =
  | "idle"
  | "listening"
  | "thinking"
  | "speaking"
  | "waiting"
  | "completed"
  | "failed"
  | "sleeping"
  | "dismissed";

  const PROMPTS = [
    "What’s been on your mind?",
    "What would you like to talk about?",
    "How are you feeling?",
    "What’s happening?",
    "Want to tell me something?",
    "What’s been taking up your thoughts lately?",
    "Is there something you’d like to get off your chest?",
    "What would feel helpful to talk through?",
    "How has your day been going?",
    "Is anything weighing on you?",
    "What are you curious about right now?",
    "Where would you like to start?",
    "Want to think something through together?",
    "What’s something you’ve been noticing?",
    "Is there a decision on your mind?",
    "What would you like some company with?",
    "Anything you want to share?"
  ];
  const PROMPT = PROMPTS[Math.floor(Math.random() * PROMPTS.length)];
const PROMPT_VISIBLE_MS = 10_000;

const visibleDuration = (text: string) => Math.max(2_000, (text.length / 18) * 1_000);

const jobSummary = (resultJson: string | null) => {
  if (!resultJson) {
    return "";
  }

  try {
    const result = JSON.parse(resultJson) as { summary?: unknown };
    return typeof result.summary === "string" ? result.summary : "";
  } catch {
    return "";
  }
};

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
  const [isJobRunning, setIsJobRunning] = useState(false);
  const [toolboxSection, setToolboxSection] = useState<ToolboxSection | null>(null);
  const [noteReceipt, setNoteReceipt] = useState<{ id: string; content: string } | null>(null);
  const [isUndoingNote, setIsUndoingNote] = useState(false);
  const [mascotReaction, setMascotReaction] = useState<"happy" | "not_happy" | null>(null);

  const shellRef = useRef<HTMLElement>(null);
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
    if (!shellRef.current || !canUseTauriWindow) {
      return;
    }

    const rect = shellRef.current.getBoundingClientRect();
    const width = Math.max(1, Math.ceil(rect.width + 20));
    const height = Math.max(1, Math.ceil(rect.height + 20));

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

  const replyToDaemon = useCallback(() => {
    clearTimers();
    setLine("");
    setPhase("waiting");
    setTimer(() => setPhase("idle"), PROMPT_VISIBLE_MS);
  }, [clearTimers, setTimer]);

  const submitMessage = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const content = input.trim();
    if (!content) {
      return;
    }

    clearTimers();
    setPhase("thinking");
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

  const handleNoteCreated = useCallback((payload: { id: string; content: string }) => {
    setIsUndoingNote(false);
    setNoteReceipt(payload);
  }, []);

  const handleMascotReaction = useCallback((payload: { reaction: "happy" | "not_happy" }) => {
    setMascotReaction(payload.reaction);
  }, []);

  const handleJobStarted = useCallback(() => {
    setIsJobRunning(true);
  }, []);

  const handleJobCompleted = useCallback(
    (payload: { job: JobRecord }) => {
      setIsJobRunning(false);
      const summary = jobSummary(payload.job.result_json);
      if (!summary) {
        return;
      }
      clearTimers();
      setLine(summary);
      setMessageKey((key) => key + 1);
      setPhase("completed");
      setTimer(() => setPhase("idle"), visibleDuration(summary));
    },
    [clearTimers, setTimer],
  );

  const handleJobFailed = useCallback(
    (payload: { job: JobRecord }) => {
      setIsJobRunning(false);
      const message = payload.job.error_message ?? "Codex did not complete the task.";
      clearTimers();
      setLine(message);
      setMessageKey((key) => key + 1);
      setPhase("failed");
      setTimer(() => setPhase("idle"), visibleDuration(message));
    },
    [clearTimers, setTimer],
  );

  const undoNoteReceipt = async () => {
    if (!noteReceipt || isUndoingNote) {
      return;
    }

    setIsUndoingNote(true);
    try {
      if (await undoNote(noteReceipt.id)) {
        setNoteReceipt(null);
      }
    } finally {
      setIsUndoingNote(false);
    }
  };

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

    if (phase === "speaking" || phase === "completed" || phase === "failed") {
      replyToDaemon();
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
      const timer = window.setTimeout(() => setPhase("sleeping"), 10_000);
      return () => window.clearTimeout(timer);
    }
    return undefined;
  }, [phase]);

  useEffect(() => {
    if (!noteReceipt) {
      return undefined;
    }

    const timer = window.setTimeout(() => setNoteReceipt(null), 5_000);
    return () => window.clearTimeout(timer);
  }, [noteReceipt]);

  useEffect(() => {
    if (!mascotReaction) {
      return undefined;
    }

    const timer = window.setTimeout(() => setMascotReaction(null), 7_000);
    return () => window.clearTimeout(timer);
  }, [mascotReaction]);

  useEffect(() => {
    if (!shellRef.current || !canUseTauriWindow) {
      return;
    }

    const observer = new ResizeObserver(resizeToContent);
    observer.observe(shellRef.current);
    return () => observer.disconnect();
  }, [canUseTauriWindow, resizeToContent]);

  useEffect(() => {
    const frame = window.requestAnimationFrame(resizeToContent);
    return () => window.cancelAnimationFrame(frame);
  }, [isRendered, line, noteReceipt, phase, resizeToContent, toolboxSection]);

  useEffect(() => {
    if (!canUseTauriWindow) {
      return undefined;
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

    void onNoteCreated(handleNoteCreated).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void onMascotReaction(handleMascotReaction).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void onJobStarted(handleJobStarted).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void onJobCompleted(handleJobCompleted).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void onJobFailed(handleJobFailed).then((unlisten) => {
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
    handleJobCompleted,
    handleJobFailed,
    handleJobStarted,
    handleMessageReady,
    handleMascotReaction,
    handleNoteCreated,
  ]);

  const hasPanel = phase === "speaking"
    || phase === "completed"
    || phase === "failed"
    || phase === "waiting"
    || noteReceipt
    || toolboxSection;

  return (
    <main className={`daemon-container phase-${phase} ${isRendered ? "is-rendered" : "is-hidden"}`}>
      {isRendered && (
        <section
          ref={shellRef}
          className={`companion-shell ${hasPanel ? "has-panel" : ""}`}
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
                  : mascotReaction
                    ? mascotReaction
                    : phase === "speaking"
                    ? "speaking"
                    : phase === "listening"
                      ? "listening"
                      : phase === "thinking"
                        ? "thinking"
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
              <button
                key={messageKey}
                type="button"
                className="ambient-line"
                aria-label="Reply to Daemon"
                onClick={replyToDaemon}
              >
                {line}
              </button>
            )}

            {noteReceipt && (
              <aside className="note-receipt" aria-label="Note saved">
                <span>{noteReceipt.content}</span>
                <div className="note-receipt-actions">
                  <button type="button" disabled={isUndoingNote} onClick={() => void undoNoteReceipt()}>
                    Undo
                  </button>
                  <button type="button" aria-label="Dismiss saved note" onClick={() => setNoteReceipt(null)}>
                    ×
                  </button>
                </div>
              </aside>
            )}

            {phase === "waiting" && (
              <div className="interactive-card">
                <button type="button" className="dismiss-button" aria-label="Dismiss" onClick={dismiss}>
                  ×
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
