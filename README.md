<div align='center'>
    <img src='public\wizard_hat\wizard_hat_blinking.gif' alt="wizard_hat"/>

<h1>Daemon</h1>
<p>
Daemon is your human like ai companion like microsoft clippy. It can remember things about you, write short notes for you, even look at your screen and talk about it. 
</p>
</div>

---

## What Daemon Is

Daemon is a Windows desktop companion built with Tauri and React. It keeps local conversation data in SQLite, can call a small set of Rust-owned local tools, and presents its replies through a transparent mascot window.

## Installation

Download **Daemon v1.0.0** from its [release page](https://github.com/bremsstrahlung-57/daemon/releases/tag/v1.0.0), or install it directly below.

### Windows (64-bit)

Run in PowerShell:

```powershell
Invoke-WebRequest "https://github.com/bremsstrahlung-57/daemon/releases/download/v1.0.0/daemon_1.0.0_x64-setup.exe" -OutFile "Daemon-setup.exe"
Start-Process ".\Daemon-setup.exe"
```

### macOS

Apple Silicon:

```bash
curl -fL "https://github.com/bremsstrahlung-57/daemon/releases/download/v1.0.0/daemon_1.0.0_aarch64.dmg" -o Daemon.dmg
open Daemon.dmg
```

Intel:

```bash
curl -fL "https://github.com/bremsstrahlung-57/daemon/releases/download/v1.0.0/daemon_1.0.0_x64.dmg" -o Daemon.dmg
open Daemon.dmg
```

### Ubuntu or Debian (64-bit)

```bash
curl -fLO "https://github.com/bremsstrahlung-57/daemon/releases/download/v1.0.0/daemon_1.0.0_amd64.deb"
sudo apt install ./daemon_1.0.0_amd64.deb
```

### Fedora, RHEL, or openSUSE (64-bit)

Download the RPM once:

```bash
curl -fLO "https://github.com/bremsstrahlung-57/daemon/releases/download/v1.0.0/daemon-1.0.0-1.x86_64.rpm"
```

Install it with your distribution's package manager:

```bash
sudo dnf install ./daemon-1.0.0-1.x86_64.rpm     # Fedora or RHEL
sudo zypper install ./daemon-1.0.0-1.x86_64.rpm  # openSUSE
```

### Other Linux distributions (64-bit)

```bash
curl -fLO "https://github.com/bremsstrahlung-57/daemon/releases/download/v1.0.0/daemon_1.0.0_amd64.AppImage"
chmod +x daemon_1.0.0_amd64.AppImage
./daemon_1.0.0_amd64.AppImage
```

See [all releases](https://github.com/bremsstrahlung-57/daemon/releases) for newer versions and portable `.tar.gz` archives.

## Architecture

- **Desktop shell and UI:** Tauri 2 hosts a React 19 / TypeScript interface. The UI renders the mascot, prompt and reply surfaces, Settings, notes receipts, and event-driven status updates.
- **Backend:** Rust owns persistence, tool validation and execution, provider requests, native window and tray behavior, screen capture, and local model inference. The frontend asks for actions through Tauri commands and receives `daemon://` events; it does not enforce tool policy.
- **Storage:** SQLite stores conversations, messages, notes, memories, screen observations, proposals, jobs, audit events, and provider metadata. Provider API keys are stored in the Windows credential manager rather than SQLite.
- **Live conversational tools:** The model currently receives exactly these function tools: `create_note`, `create_memory`, `search_memories`, `search_notes`, `show_mascot_reaction`, and `capture_screen`. Rust validates every tool call before it is executed.
- **Conversation continuation:** Replying to a Daemon message sends the quoted user/Daemon exchange as bounded context alongside the new message. The normal local conversation history remains available as well.

## Models and Providers

- **Conversation provider:** Daemon sends OpenAI-compatible chat-completions requests to the configured provider.
- **Local screen model:** On first launch, Screen Aware downloads the pinned 4-bit Moondream2 archive into Daemon's app-data directory, verifies its SHA-256 checksum, and extracts the ONNX assets locally. Rust reuses the loaded ONNX Runtime for subsequent captures. Moondream2 is licensed under Apache 2.0.

## Features

- Local conversation persistence with reply context for follow-up questions.
- Automatic note creation for note-worthy messages, duplicate detection, a short-lived Undo receipt, soft deletion, and matching audit events.
- Durable user memories, plus tool-based note and memory search when the user asks for saved context.
- Brief happy or not-happy mascot reactions for clearly positive or negative messages. This is a presentation reaction, not persistent mood tracking.
- Screen Aware: configurable automatic interval capture, toolbox capture, and explicit “look at my screen” requests. Screenshots are captured and processed in memory by the local Moondream2 runtime, then discarded; only concise text descriptions are written to SQLite. Automatic capture pauses while Daemon is dismissed.
- A transparent, draggable mascot window with tray controls, Settings, and event-driven completion receipts.

## What We Tried and Cut

`describe_repo` and `run_codex_task` were built and tested, including an allowlisted repository lookup, proposal approval lifecycle, job/audit records, a read-only `codex exec --sandbox read-only` executor, timeout handling, and repository-change checks. They are intentionally descoped from the conversational path: neither tool is in the live conversational tools array, and conversational dispatch rejects them as unavailable. They are not a current user-facing Daemon feature.

## Setup

### Prerequisites

- Windows with the Rust toolchain and Bun installed.
- The standard Tauri Windows build prerequisites for the installed Rust toolchain and WebView2 runtime.
- An OpenAI-compatible provider API key. The app stores it in Windows Credential Manager when it is saved in Settings.

### Run from a clean clone

```powershell
git clone <your-repository-url>
cd daemon
bun install
bun run tauri dev
```

The Screen Aware model is downloaded once at first launch from the pinned Moondream2 release and stored in Daemon's app-data directory. The app verifies the downloaded archive before using it; after that first download, Screen Aware works offline.

When the app opens, use **Settings** in the toolbox to add or select a provider. For the configuration used in this project, enter:

| Setting | Value |
| --- | --- |
| Provider name | `OpenAI` |
| Base URL | `https://api.openai.com/v1` |
| Model | `any model of your choice` |
| API key | Your OpenAI API key |

No `.env` file is required for conversation-provider configuration. The only optional runtime environment variables are `DAEMON_REPOSITORY_ROOT`, `DAEMON_DEMO_REPOSITORY`, and `DAEMON_CODEX_TIMEOUT_SECONDS`; they belong to the intentionally descoped Codex proposal path.

## Pre-existing Work vs Submission Period Work

The repository records the original Daemon concept and early mascot-window work before the submission implementation period: `basic window for daemon, draggable, transparent and borderless` was committed on **2026-05-17**, the text-box and state work on **2026-05-21**, and dynamic window sizing on **2026-06-21**.

The implementation that makes this submission work was committed during **2026-07-16 through 2026-07-18**: the project plan and mascot assets on July 16; the Tauri window and provider/chat foundation later that day; memory tooling and mascot reactions on July 18; and local screen capture, ONNX Moondream2 wiring, persistence, and UI settings on July 18. This distinction is visible in commits `97efe37`, `cfa8fe3`, `4b43011`, `b5fec7d`, and `0fa4a91`.

## Third-party Licensing and Services

- **Moondream2:** Apache License 2.0. The project downloads the pinned 4-bit Moondream2 model archive once for local Screen Aware inference.
- **Application dependencies:** Tauri, React, ONNX Runtime bindings, SQLite bindings, and the other Rust and JavaScript dependencies are listed in `src-tauri/Cargo.toml` and `package.json`.

## How We Used Codex and GPT-5.6

Codex accelerated implementation and review across the Rust host, Tauri events, SQLite transactions, tool schemas, and React surfaces. It was especially useful for turning the intended behavior into working Rust in areas that would otherwise have taken much longer to build alone. The planning/escalation model was used for planning and for diagnosing a Moondream pipeline bug; product boundaries, tool policy, and UI behavior remained project decisions.

One important result was a feature that did not ship in Daemon's conversational surface. `describe_repo` and `run_codex_task` were built fully: allowlisted repository resolution, immutable approval-bound proposal arguments, job and audit records, a read-only `codex exec --sandbox read-only` invocation, timeout and failure handling, and working-tree checks. The path was tested against a real repository.

It was then deliberately removed from the model-facing tools array. An agentic coding layer did not fit the ambient-companion vision: Daemon should remain present, local, and interruptible rather than turn ordinary conversation into a coding-agent control plane. The reusable approval and job lifecycle remains in the Rust backend, while the model cannot currently invoke repository inspection or Codex tasks.

The app accepts an OpenAI-compatible provider configured in Settings, so GPT-5.6 is not required. GPT-5.6-class models provide the strongest tool-following behavior for the current interaction design.
