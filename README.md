# mediascribe

Cross-platform desktop app to transcribe video / audio with **local Whisper** or **OpenAI API**, tuned for Spanish + English code-switching (university lectures, technical talks).

> **Status: alpha.** Under active development. Not yet ready for general use.

## Why

Off-the-shelf transcription tools struggle with classes where the speaker mixes Spanish narration with English technical terms (`pandas`, `DataFrame`, `bins`, `subplot`, ...). `mediascribe` lets you:

- Use a **local engine** (whisper.cpp / `large-v3` model) — free, no quota, works offline once the model is downloaded.
- Use the **OpenAI API** (`gpt-4o-transcribe`) — significantly better on technical vocabulary in mixed-language audio (~$0.30 per 50-min lecture).
- Tune transcription with an **editable `initial_prompt`** that biases the model toward your domain vocabulary.
- Save reusable **prompt templates** per topic.

You decide which engine to use per video — no automatic fallback that surprises you with API costs.

## Features

- Drag & drop a video file, get `.txt` and `.srt`.
- Resume interrupted transcriptions without re-paying for completed chunks.
- Echo-of-prompt detection (a known quirk of `gpt-4o-transcribe` on silent / very short chunks).
- Cross-platform: Windows + macOS.

## Build from source

Requirements:
- Rust (stable, MSVC toolchain on Windows)
- Node.js 20+
- Visual Studio with "Desktop development with C++" workload (Windows)
- Xcode Command Line Tools (macOS)

```bash
git clone git@github.com:rcontreras1337/mediascribe.git
cd mediascribe
npm install
```

### Fetch ffmpeg sidecar binaries

The app calls bundled `ffmpeg` and `ffprobe` binaries. They are not committed
(~150 MB combined). Fetch them once per machine:

**Windows** (PowerShell):
```powershell
.\scripts\fetch-ffmpeg.ps1
```

**macOS** (bash) — placeholder, not wired up yet:
```bash
./scripts/fetch-ffmpeg.sh
```

This downloads static builds and places them at `src-tauri/binaries/`
with the target-triple naming Tauri expects. Re-run any time you change
host platforms.

### Run

Dev mode:
```bash
npm run tauri dev
```

Production build:
```bash
npm run tauri build
```

Installers land in `src-tauri/target/release/bundle/`.

## Tech stack

- [Tauri 2](https://tauri.app/) — Rust + WebView shell.
- [SvelteKit](https://kit.svelte.dev/) + [TypeScript](https://www.typescriptlang.org/) — frontend.
- [whisper-rs](https://github.com/tazz4843/whisper-rs) — Rust bindings to whisper.cpp.
- [OpenAI API](https://platform.openai.com/) — `gpt-4o-transcribe`.
- [ffmpeg](https://ffmpeg.org/) — bundled as sidecar binary.

## Roadmap

See [`docs/PLAN.md`](docs/PLAN.md) for the full implementation plan: use cases,
allowed/disallowed behaviors, architecture, TDD phases, and risks.

## License

[MIT](LICENSE) — © 2026 Ruben Contreras.
