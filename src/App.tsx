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
import ConfirmationCard from "./components/ConfirmationCard";
import Mascot from "./components/Mascot";
import {
  approveProposal,
  denyProposal,
  pendingProposals,
  submitConversationTurn,
  undoNote,
  type NoteReceipt,
  type ProposalRecord,
} from "./lib/daemon";
import {
  onMessageReady,
  onJobCompleted,
  onJobFailed,
  onJobStarted,
  onProposalCreated,
  onProposalResolved,
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
const visibleDuration = (text: string) => (text.length / 15) * 1000;
const PROMPT_VISIBLE_MS = 10_000;

function App() {
  const canUseTauriWindow = "__TAURI_INTERNALS__" in window;
  const [phase, setPhase] = useState<CompanionPhase>("idle");
  const [isRendered, setIsRendered] = useState(true);
  const [line, setLine] = useState("I’m here.");
  const [messageKey, setMessageKey] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const [input, setInput] = useState("");
  const [fixtureMode, setFixtureMode] = useState(false);
  const [conversationId, setConversationId] = useState<string | undefined>();
  const [receipt, setReceipt] = useState<NoteReceipt | null>(null);
  const [proposal, setProposal] = useState<ProposalRecord | null>(null);
  const [isResolvingProposal, setIsResolvingProposal] = useState(false);
  const [isJobRunning, setIsJobRunning] = useState(false);

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
    setInput("");
    try {
      const result = await submitConversationTurn({
        content,
        conversationId,
        fixtureId: fixtureMode ? "rehearsal-and-login-bug" : undefined,
      });
      setConversationId(result.conversation_id);
      const nextReceipt = result.notes[0];
      if (nextReceipt) {
        setReceipt(nextReceipt);
        setTimer(() => setReceipt(null), 6000);
      }
    } catch {
      setLine("I couldn’t save that locally.");
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("I couldn’t save that locally."));
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

  const handleUndo = async () => {
    if (!receipt) {
      return;
    }
    await undoNote(receipt.note.id);
    setReceipt(null);
  };

  const refreshPendingProposals = useCallback(async () => {
    if (!canUseTauriWindow) {
      return;
    }
    try {
      const pending = await pendingProposals();
      setProposal(pending[0] ?? null);
    } catch {
      setProposal(null);
    }
  }, [canUseTauriWindow]);

  const handleProposalApprove = async () => {
    if (!proposal) {
      return;
    }
    setIsResolvingProposal(true);
    try {
      await approveProposal(proposal);
      setProposal(null);
      setLine("Preparing the isolated task.");
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("Preparing the isolated task."));
    } catch {
      setLine("I couldn’t record that approval.");
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("I couldn’t record that approval."));
    } finally {
      setIsResolvingProposal(false);
    }
  };

  const handleProposalDeny = async () => {
    if (!proposal) {
      return;
    }
    setIsResolvingProposal(true);
    try {
      await denyProposal(proposal);
      setProposal(null);
      setLine("Okay. I won’t touch it.");
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("Okay. I won’t touch it."));
    } catch {
      setLine("I couldn’t record that decision.");
      setMessageKey((key) => key + 1);
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("I couldn’t record that decision."));
    } finally {
      setIsResolvingProposal(false);
    }
  };

  const handleJobStarted = useCallback(() => {
    setIsJobRunning(true);
  }, []);

  const handleJobCompleted = useCallback(() => {
    clearTimers();
    setIsJobRunning(false);
    setLine("The isolated task completed.");
    setMessageKey((key) => key + 1);
    setPhase("completed");
    setTimer(() => {
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("The isolated task completed."));
    }, 1200);
  }, [clearTimers, setTimer]);

  const handleJobFailed = useCallback(() => {
    clearTimers();
    setIsJobRunning(false);
    setLine("The isolated task couldn’t complete.");
    setMessageKey((key) => key + 1);
    setPhase("failed");
    setTimer(() => {
      setPhase("speaking");
      setTimer(() => setPhase("idle"), visibleDuration("The isolated task couldn’t complete."));
    }, 1200);
  }, [clearTimers, setTimer]);

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
    void refreshPendingProposals();
  }, [refreshPendingProposals]);

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

    void onProposalCreated(() => {
      void refreshPendingProposals();
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        cleanups.push(unlisten);
      }
    });

    void onProposalResolved(() => {
      void refreshPendingProposals();
    }).then((unlisten) => {
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

    return () => {
      disposed = true;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [
    beginConversation,
    canUseTauriWindow,
    dismiss,
    handleMessageReady,
    handleJobCompleted,
    handleJobFailed,
    handleJobStarted,
    refreshPendingProposals,
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
              <label className="fixture-toggle">
                <input
                  type="checkbox"
                  checked={fixtureMode}
                  onChange={(event) => setFixtureMode(event.target.checked)}
                />
                Use local fixture data
              </label>
              {receipt && (
                <div className="note-receipt" role="status">
                  <span>
                    {receipt.duplicate ? "Already remembered" : "Remembered locally"}: {receipt.note.content}
                  </span>
                  <button type="button" onClick={handleUndo}>
                    Undo
                  </button>
                </div>
              )}
              </div>
            )}

            {proposal && (
              <ConfirmationCard
                proposal={proposal}
                busy={isResolvingProposal}
                onApprove={handleProposalApprove}
                onDeny={handleProposalDeny}
              />
            )}
          </div>
        </section>
      )}
    </main>
  );
}

export default App;
