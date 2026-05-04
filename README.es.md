# mediascribe

> **Read this in another language:** [English](README.md)

App de escritorio multiplataforma para transcribir video / audio con **Whisper local** o la **API de OpenAI**, ajustada para code-switching español + inglés (clases universitarias, charlas técnicas).

> **Estado: alpha.** En desarrollo activo. No lista todavía para uso general.

## Por qué

Las herramientas de transcripción genéricas se equivocan en clases donde el orador mezcla narración en español con tecnicismos en inglés (`pandas`, `DataFrame`, `bins`, `subplot`, ...). `mediascribe` te permite:

- Usar un **motor local** (whisper.cpp / modelo `large-v3`): gratis, sin cuota, funciona offline una vez descargado el modelo.
- Usar la **API de OpenAI** (`gpt-4o-transcribe`): significativamente mejor en vocabulario técnico de audio mixto (~$0.30 por clase de 50 min).
- Ajustar la transcripción con un **`initial_prompt` editable** que sesga el modelo hacia tu vocabulario de dominio.
- Guardar **plantillas de prompt** reutilizables por tema.

Tú decides qué motor usar por video — no hay fallback automático que te sorprenda con costos en la API.

## Características

- Arrastra un archivo de video, obtén `.txt` y `.srt`.
- `initial_prompt` editable con plantillas reutilizables guardadas por tema.
- Detección de eco del prompt (un comportamiento conocido de `gpt-4o-transcribe` en chunks silenciosos / muy cortos).
- La API key se guarda en el keystore del SO (Credential Manager en Windows / Keychain en macOS), nunca en disco.
- Multiplataforma: Windows + macOS (soporte de macOS planeado, build actual apunta a Windows).

## Cómo usar la app

Después de instalar el MSI (o ejecutar `npm run tauri dev`):

1. **Abre Settings** (arriba a la derecha) y pega tu API key de OpenAI. Click en Save: queda en el keystore del SO, no en disco.
2. Vuelve a **Main**. Click en **"Choose video / audio file"** y elige tu video (mp4, mkv, mov, mp3, m4a, wav, ...).
3. Elige **motor**: `API` (recomendado por calidad) o `Local` (requiere build con feature `cuda` y LLVM instalado en la máquina de desarrollo).
4. Elige **modelo de API** e **idioma** (default: `gpt-4o-transcribe`, `es`).
5. **Edita el initial prompt**: lista las palabras de dominio específico que el orador va a usar (nombres de funciones, jerga, nombres propios). Opcionalmente guárdalo como plantilla para la próxima vez.
6. Click en **Transcribe**. Mira el progreso por chunk en vivo.
7. Cuando termine, la app muestra la ruta del `.txt` y el `.srt`. Click en **"Open folder"** para navegar ahí.

Las salidas quedan **al lado del video original**, con el nombre `<video>.api.txt` / `.api.srt` (o `.local.*` si usaste el motor local).

> **Nota sobre el `.srt` del motor API:** `gpt-4o-transcribe` sólo devuelve texto plano, sin timestamps por segmento. El `.srt` que emitimos es un solo bloque cubriendo todo el audio. Para timestamps reales por línea, usa el motor local (build con `--features cuda`).

## Build desde código fuente

Requisitos:
- Rust (stable, toolchain MSVC en Windows)
- Node.js 20+
- Visual Studio con el workload "Desktop development with C++" (Windows)
- Xcode Command Line Tools (macOS)

```bash
git clone git@github.com:rcontreras1337/mediascribe.git
cd mediascribe
npm install
```

### Descargar binarios sidecar de ffmpeg

La app llama a binarios `ffmpeg` y `ffprobe` empaquetados. No se commitean (~150 MB combinados). Descárgalos una vez por máquina:

**Windows** (PowerShell):
```powershell
.\scripts\fetch-ffmpeg.ps1
```

**macOS** (bash) — placeholder, no implementado todavía:
```bash
./scripts/fetch-ffmpeg.sh
```

Esto descarga los builds estáticos y los pone en `src-tauri/binaries/` con el naming target-triple que Tauri espera. Vuelve a correrlo si cambias de plataforma.

### Ejecutar

Modo dev:
```bash
npm run tauri dev
```

Build de producción (sólo motor API, sin motor local):
```bash
npm run tauri build
```

Los instaladores quedan en `src-tauri/target/release/bundle/`.

## Activar el motor local (opcional)

El motor local corre `whisper.cpp` en tu propia máquina: gratis, offline, acelerado por GPU. Es un build opt-in porque compilar `whisper-rs` necesita **LLVM/libclang** para que bindgen genere los bindings (whisper.cpp es C++, bindgen los traduce a Rust).

Setup una sola vez (Windows):

```powershell
# 1. Instalar LLVM (terminal admin, ~600 MB)
winget install --id LLVM.LLVM -e

# 2. Cierra todas las terminales, abre una nueva, verifica
clang --version
```

Después construye la app con motor local + GPU activados:

```powershell
cd mediascribe
npm run tauri:build:cuda
```

Eso produce un MSI nuevo en `src-tauri/target/release/bundle/msi/` con el motor local incluido. Dentro de la app:

- La opción **"Local"** del selector de motor pasa a funcionar.
- La primera vez que transcribes localmente, la app descarga el modelo Whisper que elijas (ej. `large-v3` son ~3 GB) desde HuggingFace a `%APPDATA%\mediascribe\models\`. Las siguientes corridas reusan el cache.
- La transcripción ocurre completamente en tu máquina. Sin red, sin costo por minuto.

CUDA en tiempo de build requiere el **CUDA Toolkit** (con `nvcc`). Si sólo tienes CPU, cambia `tauri:build:cuda` por un build `--features local-engine` (sólo CPU). Edita `package.json` para agregar el script si lo necesitas.

## Stack técnico

- [Tauri 2](https://tauri.app/) — shell de Rust + WebView.
- [SvelteKit](https://kit.svelte.dev/) + [TypeScript](https://www.typescriptlang.org/) — frontend.
- [whisper-rs](https://github.com/tazz4843/whisper-rs) — bindings de Rust a whisper.cpp.
- [OpenAI API](https://platform.openai.com/) — `gpt-4o-transcribe`.
- [ffmpeg](https://ffmpeg.org/) — empaquetado como binario sidecar.

## Roadmap

Ver [`docs/PLAN.md`](docs/PLAN.md) para el plan completo de implementación: casos de uso, comportamientos permitidos / no permitidos, arquitectura, fases TDD y riesgos.

## Licencia

[MIT](LICENSE) — © 2026 Ruben Contreras.
