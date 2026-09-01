<div align="center">
  <img src="public/wizard_hat/wizard_hat_sleeping.gif" alt="Daemon mascot" width="128" />
  <h1>Daemon</h1>
  <p>A local-first desktop companion that remembers, helps, and stays out of the way.</p>
  <p>
    <a href="https://github.com/bremsstrahlung-57/daemon/releases"><img src="https://img.shields.io/github/v/release/bremsstrahlung-57/daemon?display_name=tag&style=flat-square" alt="Latest release" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/github/license/bremsstrahlung-57/daemon?style=flat-square" alt="License" /></a>
    <img src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white" alt="Tauri 2" />
    <img src="https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black" alt="React 19" />
  </p>
  <p>
    <a href="#demo">Demo</a> ·
    <a href="#features">Features</a> ·
    <a href="#installation">Installation</a> ·
    <a href="#development">Development</a> ·
    <a href="#architecture">Architecture</a>
  </p>
</div>

Daemon can hold conversations, remember useful information, create notes, and describe what is on your screen without storing screenshots.

## Demo

https://github.com/user-attachments/assets/44813cde-77c4-4476-a415-e08838930ac1

## Features

- Persistent conversations with context for follow-up questions.
- Automatic note creation with duplicate detection, undo, and soft deletion.
- Durable memories with search for notes and saved context.
- Configurable OpenAI-compatible conversation providers.
- Screen Aware capture and description using the local Moondream2 model.
- Transparent, draggable mascot window with tray controls and Settings.
- Local SQLite storage and operating-system credential storage for provider API keys.

## Installation

Prebuilt packages are available on the [releases page](https://github.com/bremsstrahlung-57/daemon/releases).

### Windows

Download and run the Windows installer for your architecture.

### macOS

Download the `.dmg` package for Apple Silicon or Intel, open it, and move Daemon to your Applications folder.

### Linux

Download the package that matches your distribution (`.deb`, `.rpm`, or `.AppImage`). For an AppImage, make it executable before launching it:

```bash
chmod +x Daemon.AppImage
./Daemon.AppImage
```

## Development

### Prerequisites

- [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/tools/install)
- The platform prerequisites for [Tauri 2](https://v2.tauri.app/start/prerequisites/)
- WebView2 on Windows
- An API key for an OpenAI-compatible conversation provider

### Run locally

```bash
git clone https://github.com/bremsstrahlung-57/daemon.git
cd daemon
bun install
bun run tauri dev
```

To create a production frontend bundle:

```bash
bun run build
```

To package the desktop application:

```bash
bun run tauri build
```

## Configuration

Open **Settings** in Daemon and add a conversation provider. The default OpenAI configuration uses:

| Setting | Value |
| --- | --- |
| Provider name | `OpenAI` |
| Base URL | `https://api.openai.com/v1` |
| Model | Any compatible chat model |
| API key | Your provider API key |

Provider API keys are stored in the operating system's credential manager. Conversation data and application state are stored locally in SQLite.

On first use, Screen Aware downloads the pinned 4-bit Moondream2 model to Daemon's application-data directory, verifies its SHA-256 checksum, and stores the extracted model assets locally. Screen captures are processed in memory and discarded; only short descriptions are persisted.

## Architecture

Daemon is a Tauri 2 application with a React and TypeScript frontend and a Rust backend.

- `src/` contains the UI, mascot presentation, settings, notes, and typed Tauri event handling.
- `src-tauri/src/` contains persistence, provider requests, tool validation, screen capture, local model inference, and native application behavior.
- SQLite stores conversations, messages, notes, memories, observations, and audit data.
- The frontend communicates with the backend through typed Tauri commands and `daemon://` events.

Rust owns tool validation, policy, persistence, and execution. The frontend requests actions but does not enforce application policy.

## Privacy and security

- Screen pixels remain local by default and are not persisted.
- Sensitive application exclusions and perception controls are handled by the application.
- Provider credentials are not stored in frontend state, SQLite, logs, or screenshots.
- Model-generated tool proposals are validated by the Rust backend before execution.

## License

See [LICENSE](LICENSE) for the project license.

Moondream2 is distributed under the Apache License 2.0. Other application dependencies and their licenses are listed in `src-tauri/Cargo.toml` and `package.json`.
