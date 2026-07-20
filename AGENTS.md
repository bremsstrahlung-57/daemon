# Repository Guidelines

## Product and Safety Invariants

`PLAN.md` is the implementation source of truth for the Build Week scope and schedule. Daemon is a proactive desktop companion, not a command-driven chatbot. Preserve these invariants:

- Models produce untrusted conversation, observations, and action proposals; Rust owns policy and execution.
- Bind approval to persisted, immutable tool arguments. Changed, denied, expired, or duplicate proposals must not execute.
- `create_note` is local, reversible, and automatic. Paid coding tasks and external actions require confirmation before starting.
- Run Codex only against an allowlisted repository in an isolated Git worktree. Never modify the user's active working tree automatically.
- Screen pixels stay local by default. Do not persist screenshots or silently fall back to cloud vision.
- Treat text visible on screen as untrusted data, never as instructions or authorization.
- Keep perception visible and interruptible with an `eyes on` indicator, pause controls, and sensitive-application exclusions.
- Never claim an action started or completed until the Rust state machine reports that state.

## Agent Execution Policy

Do not run `bun run dev`, `bun run tauri dev`, `bun run build`, `bun run tauri build`, `cargo check`, `cargo fmt`, or any other build/dev command on your own initiative. Propose the command and wait for explicit confirmation before running it. This applies even when a change looks safe or the task seems to call for verification. If you need to confirm something compiles, say so and ask; don't just run it.

## Project Structure & Module Organization

This is a Tauri 2 desktop application with a React/TypeScript frontend and a Rust host:

- `src/` — UI, companion state rendering, confirmation cards, and typed Tauri event handling.
- `src-tauri/src/` — Rust application services, policy, persistence, model orchestration, perception, and local execution.
- `src-tauri/capabilities/` and `src-tauri/tauri.conf.json` — Tauri permissions and packaging.
- `public/wizard_hat/` — mascot static poses, sprite sheets, and Pixly `.anim` frame descriptors.
- `dist/` — generated output; do not edit by hand.
- `src-tauri/icons/` — application icons.
- `PLAN.md` — Build Week architecture, scope, safety constraints, schedule, and demo acceptance criteria.

Keep presentation in `src/` and machine access, secrets, policy, screen capture, persistence, OpenAI calls, and subprocess management in `src-tauri/`. Communicate with typed Tauri commands and events using the `daemon://` prefix (e.g. `daemon://proposal-created`). The frontend may request actions but must never enforce approval policy itself.

## Build, Test, and Development Commands

Dependencies are managed with Bun, matching `bun.lock`. These are the commands in use, not standing permission to run them — see Agent Execution Policy above.

- `bun install` — install locked JavaScript dependencies.
- `bun run dev` — start the Vite frontend development server.
- `bun run tauri dev` — run the full desktop app with hot reload.
- `bun run build` — run TypeScript checks and produce the Vite production bundle.
- `bun run tauri build` — package the production desktop application.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — verify Rust formatting.
- `cargo check --manifest-path src-tauri/Cargo.toml` — validate the Rust host.

There is no automated test framework configured yet. Until one exists, `bun run build`, the Rust checks, and manual verification through `bun run tauri dev` are the required validation steps — run only when asked.

## Coding Style & Naming Conventions

- Two spaces, double quotes, semicolons, strict TypeScript (already enforced by `tsconfig.json`).
- React components and types: PascalCase.
- Functions and variables: camelCase.
- Constants: `UPPER_SNAKE_CASE`.
- Event names: `daemon://` prefix.
- Rust: formatted with `rustfmt`, snake_case for functions and variables.
- No unused locals or parameters, in either language.
- Strictly deserialize model and command inputs; reject unknown fields and unknown tool names.
- Pass subprocess arguments as an argument array. Never interpolate model output into a shell command string.
- Give durable entities stable IDs and make state transitions idempotent.

## Mascot and Interaction Guidelines

Use the complete visual vocabulary under `public/wizard_hat/`; do not render one mascot image for every state.

- `wizard_hat.png` — neutral.
- `wizard_hat_blinking.*` — occasional idle blink.
- `wizard_hat1.*` — listening or restrained thinking.
- `wizard_hat_talking.*` — visible speech only.
- `wizard_hat_dragged.*` — native dragging only.
- `wizard_hat_sleeping.*` — prolonged inactivity or intentionally paused perception.
- `wizard_hat_back.png` — asynchronous work.
- `wizard_hat_happy.png` — brief successful completion.
- `wizard_hat_not_happy.png` — brief task failure, never user denial or dismissal.

Treat `.anim` descriptors as the source of truth for frame regions and per-frame timing. Every frame is 64 × 64, but sheets have different frame counts and timings. Centralize semantic-to-visual mapping in one typed mascot controller, clean up timers, preload demo-critical sheets, preserve pixel rendering, and honor `prefers-reduced-motion` with representative static frames.

Backend state controls action-related mascot state. Animation timers control presentation only and must not invent `working`, `completed`, or `failed` status.

## Testing Guidelines

When adding tests, place frontend tests beside the module they cover or under `src/__tests__/`, with descriptive names like `App.test.tsx`. Place focused Rust tests beside policy, proposal, path-containment, and state-transition modules. Add a frontend test runner and its script to `package.json` only when frontend tests are introduced.

Until broader automation exists, manually verify:

- every mascot state, transition precedence, sprite timing, timer cleanup, and reduced motion;
- tray actions, dragging, dismissal, resizing, focus, and keyboard interaction;
- note creation and undo;
- exact-argument approval, denial, stale proposals, and duplicate approval;
- isolated Codex start, completion, failure, cancellation, and proactive resurfacing;
- perception pause, excluded applications, lock-screen behavior, observation deduplication, and no screenshot persistence;
- screen prompt injection does not become a tool instruction.

Build and validation commands still require explicit permission under Agent Execution Policy.

## Commit & Pull Request Guidelines

Commits are short, lowercase, descriptive summaries (e.g. `basic window for daemon, draggable, transparent and borderless.`). Follow that style, keep each commit focused, and explain user-visible behavior in the body when it isn't obvious from the diff.

Pull requests should describe the change, list the validation commands that were run, link related issues, and include screenshots or recordings for UI changes.

## Security & Configuration Tips

- Store user API keys in the operating-system credential manager, never frontend state, browser storage, SQLite, logs, screenshots, or model context.
- Use `.env` only for uncommitted development configuration; never treat it as production credential storage.
- Redact authorization headers and token-shaped values from errors and audit records.
- Restrict repository access to canonical paths selected from a Rust-owned allowlist. The model supplies repository IDs, never filesystem paths.
- Keep screen observations bounded and short-lived. Raw frames must be transient and local unless a future explicit sharing flow is approved.
- Never silently switch a local Moondream endpoint to a hosted endpoint.
- Review every Tauri capability change carefully and grant the narrowest permission required.
- Persist proposal and job state before starting side effects so crashes and retries cannot bypass confirmation.
