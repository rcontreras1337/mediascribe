# Plan de acción — App de transcripción (Tauri + Rust)

> **Estado:** plan inicial, pendiente de aprobación para arrancar implementación.
> **Última actualización:** 2026-05-03

---

## 0. Contexto y objetivo

Aplicación de escritorio para transcribir videos de clases universitarias.
El flujo manual ya está validado en Python (`transcribir.py`, `transcribir_api.py`):
sabemos que `gpt-4o-transcribe` da calidad significativamente mejor que el motor
local en code-switching ES↔EN, pero el local sigue siendo útil para bulk gratis.
La app empaqueta ese flujo con UI, soporte cross-platform y sin necesidad de que
el usuario maneje terminales o entornos virtuales.

**Objetivo no es reescribir el flujo** — es empaquetarlo como producto reutilizable.

---

## 1. Decisiones técnicas

| Área | Decisión | Por qué |
|---|---|---|
| Framework | **Tauri 2.x** | Bundle pequeño, IPC simple, cross-platform real |
| Backend | **Rust nativo** (sin Python embebido) | Distribución limpia, sin runtime extra |
| Motor local | `whisper-rs` (whisper.cpp bindings) | Mismos modelos OpenAI, CUDA en Win + Metal en Mac |
| Motor API | `reqwest` + `serde_json` (HTTP) | Sin SDK, control total |
| Audio | **ffmpeg como sidecar binary** | Feature nativa de Tauri (`bundle.externalBin`) |
| Frontend | HTML/CSS/JS vanilla o Svelte (decidir en Fase 7) | Mantener simple — la UI no es el reto |
| Storage settings | TOML en `app_data_dir()` | Estándar |
| API key | **Keystore del SO** (Keychain Mac, Credential Manager Win) | Nunca en plain text |
| Targets | Windows x86_64 + macOS (universal: arm64 + x86_64) | Lo que pidió el usuario |

---

## 2. Casos de uso ordenados (cases of order)

### CU-1 — Onboarding (primera vez)
1. Usuario abre la app por primera vez.
2. App detecta que no hay modelo local descargado y muestra wizard:
   - Elegir modelo default a descargar (large-v3 recomendado, ~3 GB).
   - Opcional: configurar API key si quiere usar también motor API.
3. Descarga el modelo en background con barra de progreso.
4. Cuando termina, la app queda lista para uso.

### CU-2 — Transcripción local corta (≤ 9 min)
1. Usuario arrastra video a la app o pulsa "Elegir archivo".
2. App detecta duración con ffprobe (sidecar).
3. Usuario edita el `initial_prompt` (si quiere) y elige formato de salida.
4. Pulsa "Transcribir" → motor local procesa el audio entero en un solo pase.
5. Aparece preview del texto, opción de "Abrir carpeta" o "Copiar al portapapeles".

### CU-3 — Transcripción local larga (> 9 min)
1. Igual hasta paso 3.
2. App parte el audio en chunks de 8 min (igual que el script Python actual).
3. Procesa secuencialmente con barra de progreso por chunk.
4. Cada chunk se persiste a disco al terminar (resume-friendly).
5. Al final, concatena y entrega outputs.

### CU-4 — Transcripción con motor API
1. Usuario elige motor "API" antes de transcribir.
2. App verifica que haya API key en keystore. Si no, abre el modal de settings.
3. App estima costo (`duración × $0.006/min`) y **pide confirmación explícita**
   antes de mandar a la API. Esto evita gastos accidentales.
4. Procesa por chunks (mismo flujo que script actual), con filtro de eco del prompt.
5. Si algún chunk truena (timeout, rate limit, error 5xx), retry con backoff;
   tras 3 fallos consecutivos, pausa y permite al usuario reintentar a mano.

### CU-5 — Reanudar transcripción interrumpida
1. Usuario abrió la app después de que la PC se apagó / cerró la app a mitad.
2. App detecta que existe carpeta `<video>.chunks/transcripts/` con chunks parciales.
3. Pregunta "Detecté una transcripción a medias de `clase4.mp4`. ¿Reanudar o empezar de cero?".
4. Si reanudar: solo procesa los chunks faltantes, reusa los ya transcritos.

### CU-6 — Reprocesar con prompt distinto
1. Usuario ya transcribió un video pero quedó con errores en términos.
2. Edita el `initial_prompt` y pulsa "Reprocesar".
3. App **borra los transcripts cacheados** (no los chunks de audio) y vuelve a
   correr el motor con el prompt nuevo. Costo: igual que la primera vez.

### CU-7 — Cambio de motor a mitad
**No soportado.** Si el usuario cambia de motor con una transcripción en curso,
la app pide confirmación de cancelar y empezar de cero.

### CU-8 — Errores explícitos
- Video sin pista de audio → error claro, no transcribe.
- Formato no soportado → error con lista de formatos válidos.
- Sin internet (motor API) → error "Sin conexión, prueba motor local o reintenta".
- API key inválida (401) → modal "Tu API key no es válida, revisa settings".
- Modelo local no descargado → ofrecer descarga inmediata.
- Disco lleno → mensaje claro mencionando el path donde falló.

---

## 3. Cosas que se DEBEN permitir

- ✅ Cargar video por drag & drop **o** botón "Elegir archivo".
- ✅ Múltiples formatos de entrada: mp4, mkv, mov, avi, webm, mp3, m4a, wav, flac.
- ✅ Editar `initial_prompt` en un campo de texto multi-línea con contador de tokens
   (límite ~224 para Whisper).
- ✅ Guardar plantillas de `initial_prompt` con nombre (ej. "Python ciencia datos",
   "Estadística", "ML"); cargar una con un click.
- ✅ Elegir motor por transcripción: Local / API. **Sin default automático que
   gaste créditos**.
- ✅ Elegir modelo local por transcripción (tiny/base/small/medium/large-v3/large-v3-turbo).
- ✅ Elegir formato de salida (TXT, SRT, ambos).
- ✅ Configurar API key en settings, guardada en keystore del SO.
- ✅ Cancelar transcripción en curso (con confirmación).
- ✅ Reanudar transcripción interrumpida.
- ✅ Ver progreso por chunk (% completado, ETA).
- ✅ Ver warnings inline (eco del prompt filtrado, baja densidad sospechosa).
- ✅ Estimación de costo antes de mandar a la API (en USD).
- ✅ Abrir carpeta de output al terminar.
- ✅ Copiar transcripción al portapapeles.
- ✅ Conservar audio extraído + chunks en disco hasta que el usuario los borre
   (sirve para reprocesar y debugging).
- ✅ Cambiar idioma (ES default, EN, auto).
- ✅ Mostrar versión de la app y de los binarios sidecar (ffmpeg) en About.
- ✅ Logs locales: la app escribe a un archivo `logs/<fecha>.log` para diagnóstico.

---

## 4. Cosas que NO se deben permitir

- ❌ **Hardcodear API key en el código fuente o en el binario distribuido.**
- ❌ Mandar la API key al frontend (debe vivir solo en backend Rust).
- ❌ Guardar la API key en plain text accesible (solo keystore del SO).
- ❌ Borrar el video original automáticamente, ni siquiera con confirmación
   ambigua (riesgo de lost work).
- ❌ Transcribir con motor API **sin confirmación explícita** del costo estimado.
- ❌ Sobrescribir un `.txt` o `.srt` existente sin confirmación.
- ❌ Mandar telemetría / analytics / phone-home de cualquier tipo.
- ❌ Subir el audio a servicios distintos de los que el usuario eligió
   (ej. nada de reportes a un dashboard nuestro).
- ❌ Bundlear los modelos Whisper en el instalador (pesan 1.5–3 GB; descarga
   on-demand desde HuggingFace al primer uso).
- ❌ Permitir prompts > 224 tokens (límite de Whisper, hay que validar y truncar
   con aviso).
- ❌ Usar APIs de terceros que no estén explícitamente en la lista (Local /
   OpenAI). Si en el futuro queremos ElevenLabs, se agrega como motor con su
   propia opción y confirmación.
- ❌ Ejecutar en background sin UI visible (la app es desktop-app, no servicio).
- ❌ Auto-actualizar sin permiso (si ponemos updater, requiere confirmación).
- ❌ Acceso a archivos fuera del scope (Tauri permission system: solo permite
   leer el video que el usuario eligió y escribir en la carpeta de salida; nada
   de FS access wildcard).

---

## 5. Arquitectura

```
┌────────────────────────────────────────────────────────┐
│ Frontend (WebView)                                     │
│   ├─ Vista: selector video, motor, prompt, outputs     │
│   ├─ Vista: progreso + logs en vivo                    │
│   └─ Vista: settings (API key, plantillas, modelo def) │
│                                                        │
│   IPC vía tauri::invoke                                │
└────────────────┬───────────────────────────────────────┘
                 │
┌────────────────▼───────────────────────────────────────┐
│ Backend Rust                                           │
│  ┌──────────────────────────────────────────────────┐  │
│  │ commands.rs (Tauri commands expuestos)           │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │ orchestrator.rs   ← capa que une todo            │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌─────────────┬─────────────┬────────────────────┐   │
│  │ engines/    │ audio.rs    │ chunk.rs           │   │
│  │ ├ trait     │ (ffmpeg)    │ (particionado)     │   │
│  │ ├ local.rs  │             │                    │   │
│  │ └ api.rs    │             │                    │   │
│  └─────────────┴─────────────┴────────────────────┘   │
│  ┌─────────────┬─────────────┬────────────────────┐   │
│  │ prompt.rs   │ srt.rs      │ settings.rs        │   │
│  │ (filter eco)│ (formato)   │ (TOML + keystore)  │   │
│  └─────────────┴─────────────┴────────────────────┘   │
└────────────────────────────────────────────────────────┘
                 │
                 ▼
        ┌────────────────────┐
        │ ffmpeg sidecar bin │  (incluido por target)
        └────────────────────┘
```

### Trait Engine

```rust
trait TranscriptionEngine {
    fn name(&self) -> &str;
    fn estimate_cost(&self, duration_seconds: f64) -> Option<f64>;
    fn transcribe_chunk(&self, audio: &Path, prompt: &str, lang: &str)
        -> Result<TranscriptionResult>;
}
```

Implementaciones: `LocalEngine` (whisper-rs) y `ApiEngine` (HTTP a OpenAI).
Permite agregar `ElevenLabsEngine` después sin tocar nada más.

### Estructura de carpetas

```
trans-app/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs
│   │   ├── orchestrator.rs
│   │   ├── engines/{mod,local,api}.rs
│   │   ├── audio.rs
│   │   ├── chunk.rs
│   │   ├── prompt.rs
│   │   ├── srt.rs
│   │   └── settings.rs
│   ├── tests/
│   │   ├── integration_local.rs
│   │   ├── integration_api.rs    # opt-in (TRANS_RUN_API_TESTS=1)
│   │   └── fixtures/
│   │       ├── short_es.mp3      # 5s, frase conocida
│   │       ├── medium_mixed.mp3  # 30s, ES+EN
│   │       └── silence.mp3       # 5s solo silencio
│   ├── binaries/
│   │   ├── ffmpeg-x86_64-pc-windows-msvc.exe
│   │   ├── ffmpeg-aarch64-apple-darwin
│   │   └── ffmpeg-x86_64-apple-darwin
│   ├── tauri.conf.json
│   └── Cargo.toml
├── src/   (frontend)
│   ├── index.html
│   ├── main.ts
│   ├── styles.css
│   └── components/
├── README.md
└── PLAN.md  ← este archivo
```

---

## 6. Plan TDD por fases

Cada fase **comienza escribiendo el test que falla** y termina cuando pasa
todo (red → green → refactor).

### Fase 0 — Decisiones y bootstrap (no-código)
- ✅ Confirmar arquitectura con usuario.
- Crear repo, configurar CI mínimo (cargo test en push).

### Fase 1 — Esqueleto Tauri
- `cargo tauri init`, app vacía abre y cierra en Win + Mac.
- **Test smoke**: la app levanta sin panics. (manual al inicio, automatizado
  con WebDriver más tarde si vale la pena).

### Fase 2 — Lógica pura (TDD estricto)
Funciones sin I/O, sin red. **Cobertura objetivo: 100 %**.

| Función | Tests |
|---|---|
| `prompt::filter_echo(text, prompt) -> (clean, did_filter)` | (a) sin eco devuelve igual y `false`; (b) eco al final filtrado; (c) eco al inicio filtrado; (d) texto idéntico al prompt → vacío + `true`; (e) eco parcial (primeros 60 chars) detectado. |
| `prompt::validate(prompt) -> Result` | (a) prompt válido OK; (b) > 224 tokens → error con count; (c) vacío OK. |
| `srt::format_timestamp(secs) -> String` | (a) `0.0 → "00:00:00,000"`; (b) `61.234 → "00:01:01,234"`; (c) `3661.5 → "01:01:01,500"`. |
| `srt::format(segments) -> String` | (a) un segmento; (b) varios; (c) lista vacía → string vacío. |
| `chunk::plan(duration, chunk_size, threshold) -> Vec<Chunk>` | (a) duración corta no chunk; (b) duración exacta múltiplo; (c) duración con resto pequeño; (d) duración con resto grande. |
| `chunk::detect_truncation(text, duration, threshold) -> bool` | (a) ratio normal → false; (b) ratio bajo → true; (c) duración cero → false. |
| `settings::parse_toml(s) -> Settings` | (a) TOML válido; (b) campos faltantes usan defaults; (c) TOML inválido → error. |
| `api::estimate_cost(duration_s, model) -> f64` | gpt-4o-transcribe @ $0.006/min, gpt-4o-mini @ $0.003/min. |
| `api::validate_key_format(key) -> bool` | (a) `sk-...` ok; (b) vacío no; (c) random no. |

### Fase 3 — Audio (ffmpeg sidecar)
- `audio::extract(video, output, opts)` → mp3 mono 16kHz 48kbps.
- `audio::probe_duration(path)` → segundos.
- **Tests**: con fixture `short_es.mp3` (que copiamos a `output.mp3` y verificamos size).
- **Test de error**: archivo inexistente → error con path.

### Fase 4 — Local engine (whisper-rs)
- `LocalEngine::new(model_path)` carga el modelo.
- `transcribe_chunk(audio, prompt, lang)` retorna texto + segmentos.
- **Tests**: con `short_es.mp3` (5s con frase fija conocida), verificar que el
  texto contiene palabras esperadas (allí está la incertidumbre del modelo,
  asserts laxos: `assert text.contains("hola")` o similar).
- **Tests downloads**: con modelo `tiny` (75 MB) descargado en CI para no
  inflar el repo. El usuario en su máquina usará `large-v3`.

### Fase 5 — API engine (OpenAI HTTP)
- `ApiEngine::new(api_key)`.
- `transcribe_chunk(audio, prompt, lang)` → texto.
- **Tests con mock** (mockito o wiremock): verificar headers, body, parseo de respuesta.
- **Test real opt-in**: si `TRANS_RUN_API_TESTS=1` y hay key, manda `short_es.mp3`
  a la API real (~$0.001), valida que devuelve algo no-vacío.
- **Tests de eco del prompt**: stub que devuelve el prompt como respuesta →
  filter_echo lo detecta.

### Fase 6 — Settings + persistencia
- `settings::load() / save()` en TOML en `app_data_dir`.
- `settings::api_key_get / set` usa `keyring` crate (cross-platform: Win Credential Manager, Mac Keychain).
- **Tests**: round-trip TOML; mock keystore.

### Fase 7 — Frontend mínimo (UI funcional, sin pulir)
- Pantalla principal con selector de archivo, motor, idioma, prompt, "Transcribir".
- Pantalla de progreso (suscrita a eventos Tauri).
- Pantalla de settings.
- **Tests E2E manuales** en Win y Mac al cierre de fase.

### Fase 8 — Resume + warnings + UX final
- Detección de transcripts existentes en disco → diálogo "Reanudar?".
- Mostrar warnings inline (eco filtrado, baja densidad).
- Estimación de costo modal antes de motor API.
- **Tests integración** del resume: simular falla a mitad, re-correr, verificar
  que solo se reprocesan los faltantes.

### Fase 9 — Packaging + distribución
- Windows: `tauri build` → MSI vía wix bundler.
- macOS: `tauri build` con `--target universal-apple-darwin` → .dmg.
- Notarización macOS: requiere **Apple Developer Program ($99/año)**. Si el
  usuario no quiere pagar, distribuir la app sin firmar con instrucciones de
  "ejecutar comando en terminal para autorizarla". Documentar.
- ffmpeg sidecar: descargar binarios oficiales por target y agregarlos al bundle.
- Modelos Whisper: NO bundlear; el wizard de onboarding los baja.
- README con instrucciones de instalación + screenshots.

---

## 7. Distribución

### Windows
- Instalador **MSI** (recomendado para empresas/universidades).
- Tamaño esperado: 15–30 MB (sin modelos).
- Primer arranque baja modelo elegido (~3 GB para large-v3).

### macOS
- **.dmg universal** (Apple Silicon + Intel).
- Tamaño esperado: 20–40 MB.
- Sin Apple Developer Program: la app se puede distribuir, pero el usuario
  tiene que hacer "click derecho → Abrir" la primera vez, o quitar el quarantine
  flag. Documentar bien.
- Con Apple Developer Program: notarización + firma → experiencia limpia.

### Modelos Whisper
- Descargados desde HuggingFace bajo demanda al primer uso.
- Cacheados en `app_data_dir()/models/`.
- El usuario puede borrarlos desde Settings.

---

## 8. Riesgos y limitaciones conocidas

| Riesgo | Mitigación |
|---|---|
| `whisper-rs` puede tener gaps de paridad con `faster-whisper` en algún parámetro fino | Validar en Fase 4 con la misma clase4.mp4 que ya conocemos |
| Metal en macOS solo en Apple Silicon (M1+) — Intel Mac iría a CPU | Aceptable: Intel Mac es minoría en 2026, modo CPU sigue funcionando aunque más lento |
| Notarización macOS requiere Apple Dev ($99/año) | Plan B: distribuir sin firmar con instrucciones; Plan A: pagar si la app se va a usar mucho |
| Modelos grandes (3 GB) en disco — usuarios con poco espacio | Onboarding ofrece elegir modelo más liviano (small ~500 MB) si confirman |
| API key en keystore puede no funcionar en algunos Linux (no es target, pero por si acaso) | Linux fuera de scope inicial |
| Costo API impredecible si el usuario abusa | Confirmación explícita por video con costo estimado en USD |
| Si OpenAI cambia precios de gpt-4o-transcribe, el cálculo se desactualiza | Constante en código + nota en UI "precios pueden cambiar, verificá en openai.com" |

---

## 9. Decisiones confirmadas por el usuario (2026-05-03)

| Tema | Decisión |
|---|---|
| Embebido de Python | **NO** — usar `whisper-rs` (whisper.cpp) en su lugar |
| Targets | Windows + macOS (Linux postpuesto) |
| Distribución macOS | **Sin firmar / sin Apple Developer Program** — uso personal del usuario, distribuir con instrucciones manuales para abrir la app sin firma |
| Frontend | **Svelte** (pendiente confirmación final del usuario por error de transcripción de voz) |
| Modelos Whisper | **On-demand con wizard** — la app no bundlea modelos. Wizard al primer uso muestra lista de modelos con tamaño/calidad y baja el elegido desde HuggingFace `ggerganov/whisper.cpp`, con verificación de checksum y barra de progreso |
| Idiomas UI | Solo ES en v1 (postergar EN) |
| Batch mode | No en v1 |
| Telemetría / phone-home | NO |
| Auto-update | No en v1 |

---

## 10. Próximo paso

**Arrancar Fase 0** (bootstrap del repo + esqueleto Tauri) y **Fase 2** (lógica
pura con TDD). Estas dos fases se pueden hacer en paralelo: Fase 0 monta el
proyecto vacío y Fase 2 va escribiendo módulos puros (sin I/O) testeados.
