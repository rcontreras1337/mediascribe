# mediascribe

> **Read this in another language:** [Español](README.es.md)

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
- Editable `initial_prompt` with reusable templates saved per topic.
- Echo-of-prompt detection (a known quirk of `gpt-4o-transcribe` on silent / very short chunks).
- API key stored in the OS keystore (Windows Credential Manager / macOS Keychain), never on disk.
- Cross-platform: Windows + macOS (macOS support is planned, current build targets Windows).

## Using the app

After installing the MSI (or running `npm run tauri dev`):

1. **Open Settings** (top-right) and paste your OpenAI API key. Click Save —
   it goes into the OS keystore, not disk.
2. Back to **Main**. Click **"Choose video / audio file"** and pick your video
   (mp4, mkv, mov, mp3, m4a, wav, ...).
3. Pick **engine**: `API` (recommended for quality) or `Local` (requires the
   `cuda` build feature and an LLVM install on the dev machine).
4. Pick the **API model** and **language** (default: `gpt-4o-transcribe`, `es`).
5. **Edit the initial prompt** — list domain-specific words the speaker will
   use (function names, jargon, proper nouns). Optionally save it as a
   template for next time.
6. Click **Transcribe**. Watch the progress per chunk.
7. When done, the app shows the path of the `.txt` and `.srt`. Click **"Open
   folder"** to navigate there.

The outputs land **next to the source video**, named `<video>.api.txt` /
`.api.srt` (or `.local.*` if you used the local engine).

> **Note on `.srt` for the API engine:** `gpt-4o-transcribe` only returns
> plain text, no per-segment timestamps. The `.srt` we emit is a single
> block spanning the whole audio. For real per-line timestamps, use the
> local engine (`--features cuda` build).

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

Production build (API engine only, no local engine):
```bash
npm run tauri build
```

Installers land in `src-tauri/target/release/bundle/`.

## Enable the local engine (optional)

The local engine runs `whisper.cpp` on your own machine — free, offline,
GPU-accelerated. It's an opt-in build because compiling `whisper-rs`
needs **LLVM/libclang** to drive bindgen (whisper.cpp is C++, bindgen
generates the Rust FFI bindings).

One-time setup (Windows):

```powershell
# 1. Install LLVM (admin terminal, ~600 MB)
winget install --id LLVM.LLVM -e

# 2. Close all terminals, open a new one, verify
clang --version
```

Then build the app with the local engine + GPU enabled:

```powershell
cd mediascribe
npm run tauri:build:cuda
```

That produces a new MSI under `src-tauri/target/release/bundle/msi/`
with the local engine wired up. Inside the app:

- The **"Local"** option in the engine selector becomes functional.
- The first time you transcribe locally, the app downloads the chosen
  Whisper model (e.g. `large-v3` is ~3 GB) from HuggingFace into
  `%APPDATA%\mediascribe\models\`. Subsequent runs reuse the cache.
- Transcription happens entirely on your machine. No network, no cost
  per minute.

CUDA at build time requires the **CUDA Toolkit** (with `nvcc`). If you
only have CPU available, swap `tauri:build:cuda` for a `--features local-engine`
build (CPU only). Edit `package.json` to add a script if needed.

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
