<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  import {
    getSettings,
    saveSettings,
    hasApiKey,
    setApiKey,
    deleteApiKey,
    transcribe,
    onTranscribeProgress,
  } from "$lib/api";
  import {
    API_MODELS,
    LANGUAGES,
    type EngineKind,
    type Settings,
    type TranscribeProgress,
    type TranscribeResponse,
    estimateCostUsd,
    formatDuration,
  } from "$lib/types";

  // ----- view state -----
  let view: "main" | "settings" = $state("main");

  // ----- settings (loaded from backend) -----
  let settings: Settings = $state({
    default_engine: "api",
    default_local_model: "large-v3",
    default_api_model: "gpt-4o-transcribe",
    ui_language: "es",
    prompt_templates: {},
  });
  let apiKeySet = $state(false);
  let apiKeyDraft = $state("");

  // ----- per-job inputs -----
  let videoPath: string | null = $state(null);
  let engine: EngineKind = $state("api");
  let apiModel = $state("gpt-4o-transcribe");
  let localModel = $state("large-v3");
  let language = $state("es");
  let initialPrompt = $state(
    "Clase de Python con pandas, NumPy, matplotlib, seaborn. " +
      "Conceptos: DataFrame, scatter, hist, bins, subplot, alpha, color.",
  );
  let templateName = $state("");

  // ----- job runtime state -----
  let isRunning = $state(false);
  let progress: TranscribeProgress | null = $state(null);
  let result: TranscribeResponse | null = $state(null);
  let errorMsg: string | null = $state(null);

  // Naive duration estimate: we don't probe the file before transcribe, so
  // until the backend reports it we just hide the cost estimate.
  let estimatedDurationSec: number | null = $state(null);

  let unlistenProgress: UnlistenFn | null = null;

  onMount(async () => {
    try {
      settings = await getSettings();
      apiKeySet = await hasApiKey();
      // Apply settings as form defaults
      engine = (settings.default_engine as EngineKind) ?? "api";
      apiModel = settings.default_api_model;
      localModel = settings.default_local_model;
      language = settings.ui_language;
    } catch (e) {
      errorMsg = `Failed to load settings: ${e}`;
    }
    unlistenProgress = await onTranscribeProgress((p) => {
      progress = p;
    });
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
  });

  async function pickVideo() {
    const file = await openDialog({
      multiple: false,
      directory: false,
      filters: [
        {
          name: "Video / Audio",
          extensions: ["mp4", "mkv", "mov", "avi", "webm", "mp3", "m4a", "wav", "flac", "ogg"],
        },
      ],
    });
    if (typeof file === "string") {
      videoPath = file;
      result = null;
      errorMsg = null;
    }
  }

  async function pickOutputDir(): Promise<string | null> {
    const dir = await openDialog({ directory: true, multiple: false });
    return typeof dir === "string" ? dir : null;
  }

  function defaultOutputDirFromVideo(path: string): string {
    // Place outputs alongside the source unless the user picks somewhere else.
    const slash = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
    return slash >= 0 ? path.substring(0, slash) : path;
  }

  async function startTranscribe() {
    if (!videoPath) return;
    if (engine === "api" && !apiKeySet) {
      errorMsg = "Configure your OpenAI API key in Settings first.";
      view = "settings";
      return;
    }
    isRunning = true;
    progress = { stage: "starting", chunk: null, total: null, message: null };
    errorMsg = null;
    result = null;
    try {
      const outputDir = defaultOutputDirFromVideo(videoPath);
      const response = await transcribe({
        videoPath,
        engine,
        apiModel: engine === "api" ? apiModel : undefined,
        localModel: engine === "local" ? localModel : undefined,
        language,
        initialPrompt,
        outputDir,
      });
      result = response;
      estimatedDurationSec = response.durationSeconds;
    } catch (e) {
      errorMsg = `${e}`;
    } finally {
      isRunning = false;
    }
  }

  async function openOutputFolder() {
    if (!result) return;
    const slash = Math.max(result.txtPath.lastIndexOf("\\"), result.txtPath.lastIndexOf("/"));
    if (slash < 0) return;
    await openPath(result.txtPath.substring(0, slash));
  }

  // ----- settings actions -----

  async function applySettingsToBackend() {
    await saveSettings(settings);
  }

  async function saveApiKey() {
    if (!apiKeyDraft.trim()) return;
    await setApiKey(apiKeyDraft.trim());
    apiKeyDraft = "";
    apiKeySet = true;
  }

  async function clearApiKey() {
    await deleteApiKey();
    apiKeySet = false;
  }

  async function saveTemplate() {
    if (!templateName.trim()) return;
    settings = {
      ...settings,
      prompt_templates: { ...settings.prompt_templates, [templateName.trim()]: initialPrompt },
    };
    await applySettingsToBackend();
    templateName = "";
  }

  function loadTemplate(name: string) {
    initialPrompt = settings.prompt_templates[name] ?? "";
  }

  async function deleteTemplate(name: string) {
    const next = { ...settings.prompt_templates };
    delete next[name];
    settings = { ...settings, prompt_templates: next };
    await applySettingsToBackend();
  }

  // Derived: cost estimate (visible only when we have a duration to estimate against)
  let costEstimate = $derived.by(() => {
    if (engine !== "api" || estimatedDurationSec === null) return null;
    const m = API_MODELS.find((m) => m.id === apiModel);
    if (!m) return null;
    return estimateCostUsd(estimatedDurationSec, m.pricePerMinute);
  });
</script>

<header>
  <h1>mediascribe</h1>
  <button onclick={() => (view = view === "main" ? "settings" : "main")}>
    {view === "main" ? "Settings" : "Back"}
  </button>
</header>

{#if view === "main"}
  <main>
    <section class="card">
      <button class="big" onclick={pickVideo} disabled={isRunning}>
        {videoPath ? "Change file" : "Choose video / audio file"}
      </button>
      {#if videoPath}
        <p class="path">{videoPath}</p>
      {/if}
    </section>

    <section class="card grid">
      <label>
        Engine
        <select bind:value={engine} disabled={isRunning}>
          <option value="api">API (OpenAI)</option>
          <option value="local">Local (whisper-rs)</option>
        </select>
      </label>

      {#if engine === "api"}
        <label>
          API model
          <select bind:value={apiModel} disabled={isRunning}>
            {#each API_MODELS as m}
              <option value={m.id}>{m.label}</option>
            {/each}
          </select>
        </label>
      {:else}
        <label>
          Local model
          <input bind:value={localModel} disabled={isRunning} />
        </label>
      {/if}

      <label>
        Language
        <select bind:value={language} disabled={isRunning}>
          {#each LANGUAGES as l}
            <option value={l.code}>{l.label}</option>
          {/each}
        </select>
      </label>
    </section>

    <section class="card">
      <label class="full">
        Initial prompt
        <textarea
          rows="6"
          bind:value={initialPrompt}
          disabled={isRunning}
          placeholder="List domain-specific words the speaker will use (function names, jargon, proper nouns). Whisper biases its vocabulary toward these."
        ></textarea>
      </label>

      {#if Object.keys(settings.prompt_templates).length > 0}
        <div class="templates">
          <span>Load template:</span>
          {#each Object.keys(settings.prompt_templates) as name}
            <button class="chip" onclick={() => loadTemplate(name)} disabled={isRunning}
              >{name}</button
            >
          {/each}
        </div>
      {/if}
    </section>

    <section class="card actions">
      <button
        class="primary big"
        onclick={startTranscribe}
        disabled={isRunning || !videoPath}
      >
        {isRunning ? "Transcribing..." : "Transcribe"}
      </button>
      {#if costEstimate !== null}
        <span class="cost">≈ ${costEstimate.toFixed(2)} USD</span>
      {/if}
    </section>

    {#if isRunning && progress}
      <section class="card progress">
        <div class="stage">{progress.stage.replace(/_/g, " ")}</div>
        {#if progress.chunk && progress.total}
          <div class="bar">
            <div
              class="bar-fill"
              style="width: {(progress.chunk / progress.total) * 100}%"
            ></div>
          </div>
          <div class="step">chunk {progress.chunk} / {progress.total}</div>
        {/if}
        {#if progress.message}
          <div class="msg">{progress.message}</div>
        {/if}
      </section>
    {/if}

    {#if errorMsg}
      <section class="card error">
        <strong>Error:</strong>
        {errorMsg}
      </section>
    {/if}

    {#if result}
      <section class="card success">
        <h3>Done</h3>
        <p>Duration transcribed: {formatDuration(result.durationSeconds)}</p>
        <p>Chunks processed: {result.totalChunks}</p>
        <p>
          Output:<br />
          <code>{result.txtPath}</code><br />
          <code>{result.srtPath}</code>
        </p>
        {#if result.warnings.length > 0}
          <details>
            <summary>{result.warnings.length} warnings</summary>
            <ul>
              {#each result.warnings as w}<li>{w}</li>{/each}
            </ul>
          </details>
        {/if}
        <button onclick={openOutputFolder}>Open folder</button>
      </section>
    {/if}
  </main>
{:else}
  <main>
    <section class="card">
      <h2>OpenAI API key</h2>
      <p class="muted">
        Stored in your OS keystore (Windows Credential Manager). Never written to disk in plain
        text.
      </p>
      <p>Status: <strong>{apiKeySet ? "✓ Configured" : "Not set"}</strong></p>
      <div class="row">
        <input
          type="password"
          placeholder="sk-..."
          bind:value={apiKeyDraft}
          autocomplete="off"
        />
        <button onclick={saveApiKey} disabled={!apiKeyDraft.trim()}>Save</button>
        {#if apiKeySet}
          <button onclick={clearApiKey}>Remove</button>
        {/if}
      </div>
    </section>

    <section class="card">
      <h2>Defaults</h2>
      <label>
        Default engine
        <select
          bind:value={settings.default_engine}
          onchange={applySettingsToBackend}
        >
          <option value="api">API</option>
          <option value="local">Local</option>
        </select>
      </label>
      <label>
        Default API model
        <select
          bind:value={settings.default_api_model}
          onchange={applySettingsToBackend}
        >
          {#each API_MODELS as m}
            <option value={m.id}>{m.label}</option>
          {/each}
        </select>
      </label>
      <label>
        Default local model
        <input
          bind:value={settings.default_local_model}
          onchange={applySettingsToBackend}
        />
      </label>
    </section>

    <section class="card">
      <h2>Prompt templates</h2>
      <p class="muted">Save the current prompt with a name so you can reload it on the next job.</p>
      <div class="row">
        <input placeholder="Template name" bind:value={templateName} />
        <button onclick={saveTemplate} disabled={!templateName.trim()}
          >Save current prompt as template</button
        >
      </div>
      {#if Object.keys(settings.prompt_templates).length === 0}
        <p class="muted">No templates yet.</p>
      {:else}
        <ul class="tpl-list">
          {#each Object.entries(settings.prompt_templates) as [name, body]}
            <li>
              <strong>{name}</strong>
              <p class="tpl-body">{body}</p>
              <button onclick={() => deleteTemplate(name)}>Delete</button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </main>
{/if}

<style>
  :global(:root) {
    color-scheme: dark;
    font-family: Inter, "Segoe UI", system-ui, sans-serif;
    color: #e6e6e6;
    background: #1a1a1a;
  }
  :global(body) {
    margin: 0;
    background: #1a1a1a;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    border-bottom: 1px solid #2a2a2a;
    background: #141414;
  }
  header h1 {
    font-size: 1.4rem;
    margin: 0;
  }

  main {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 800px;
    margin: 0 auto;
  }

  .card {
    background: #232323;
    border: 1px solid #2f2f2f;
    border-radius: 8px;
    padding: 16px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.85rem;
    color: #b0b0b0;
  }
  label.full {
    width: 100%;
  }

  input,
  select,
  textarea,
  button {
    font-family: inherit;
    font-size: 0.95rem;
    color: #e6e6e6;
    background: #141414;
    border: 1px solid #333;
    border-radius: 6px;
    padding: 8px 10px;
  }
  textarea {
    resize: vertical;
    min-height: 100px;
    line-height: 1.4;
  }

  button {
    cursor: pointer;
    background: #2d2d2d;
    transition: background 0.15s;
  }
  button:hover:not(:disabled) {
    background: #3a3a3a;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  button.primary {
    background: #2563eb;
    border-color: #2563eb;
    color: #fff;
  }
  button.primary:hover:not(:disabled) {
    background: #1d4ed8;
  }
  button.big {
    padding: 12px 24px;
    font-size: 1rem;
  }
  button.chip {
    padding: 4px 10px;
    font-size: 0.85rem;
    border-radius: 12px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .cost {
    color: #b0b0b0;
    font-size: 0.9rem;
  }

  .templates {
    margin-top: 12px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    color: #b0b0b0;
    font-size: 0.85rem;
  }

  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .row input {
    flex: 1;
    min-width: 200px;
  }

  .path {
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
    color: #b0b0b0;
    margin-top: 8px;
    word-break: break-all;
  }

  .progress .bar {
    height: 8px;
    background: #1a1a1a;
    border-radius: 4px;
    overflow: hidden;
    margin: 8px 0;
  }
  .progress .bar-fill {
    height: 100%;
    background: #2563eb;
    transition: width 0.3s;
  }
  .progress .stage {
    text-transform: capitalize;
    font-weight: 500;
  }
  .progress .step {
    font-size: 0.85rem;
    color: #b0b0b0;
  }

  .error {
    border-color: #b91c1c;
    background: #2a1a1a;
  }
  .success {
    border-color: #166534;
    background: #1a2a1a;
  }
  .success code {
    font-size: 0.85rem;
    color: #b0b0b0;
    word-break: break-all;
  }

  .muted {
    color: #888;
    font-size: 0.85rem;
  }

  .tpl-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .tpl-list li {
    padding: 8px;
    border-bottom: 1px solid #2a2a2a;
  }
  .tpl-list li:last-child {
    border-bottom: none;
  }
  .tpl-body {
    color: #888;
    font-size: 0.85rem;
    margin: 4px 0;
    white-space: pre-wrap;
  }
</style>
