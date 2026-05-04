// Types shared between the Rust backend and the Svelte frontend.
// Mirror src-tauri/src/commands.rs serde types (camelCase on the wire).

export type EngineKind = "api" | "local";

export interface Settings {
  default_engine: string;
  default_local_model: string;
  default_api_model: string;
  ui_language: string;
  prompt_templates: Record<string, string>;
}

export interface TranscribeRequest {
  videoPath: string;
  engine: EngineKind;
  apiModel?: string;
  localModel?: string;
  language: string;
  initialPrompt: string;
  outputDir: string;
}

export interface TranscribeResponse {
  txtPath: string;
  srtPath: string;
  totalChunks: number;
  durationSeconds: number;
  warnings: string[];
}

export interface TranscribeProgress {
  stage:
    | "starting"
    | "extracting_audio"
    | "audio_extracted"
    | "transcribing_chunk"
    | "writing_outputs"
    | "done";
  chunk: number | null;
  total: number | null;
  message: string | null;
}

export interface ModelDownloadProgress {
  modelId: string;
  downloaded: number;
  total: number;
}

export interface ApiModelOption {
  id: string;
  label: string;
  pricePerMinute: number;
}

export const API_MODELS: ApiModelOption[] = [
  { id: "gpt-4o-transcribe", label: "gpt-4o-transcribe (best quality)", pricePerMinute: 0.006 },
  { id: "gpt-4o-mini-transcribe", label: "gpt-4o-mini-transcribe (half price)", pricePerMinute: 0.003 },
  { id: "whisper-1", label: "whisper-1 (legacy)", pricePerMinute: 0.006 },
];

export const LOCAL_MODELS = [
  { id: "tiny", label: "tiny (75 MB, lowest quality)", sizeMb: 75 },
  { id: "small", label: "small (466 MB)", sizeMb: 466 },
  { id: "medium", label: "medium (1.5 GB)", sizeMb: 1500 },
  { id: "large-v3", label: "large-v3 (2.9 GB, recommended)", sizeMb: 2900 },
  { id: "large-v3-turbo", label: "large-v3-turbo (1.5 GB, faster)", sizeMb: 1500 },
  { id: "large-v3-q5_0", label: "large-v3-q5_0 (1.1 GB, quantized)", sizeMb: 1100 },
];

export const LANGUAGES = [
  { code: "es", label: "Español" },
  { code: "en", label: "English" },
  { code: "auto", label: "Auto-detect" },
];

export function estimateCostUsd(seconds: number, pricePerMinute: number): number {
  if (seconds <= 0) return 0;
  return (seconds / 60) * pricePerMinute;
}

export function formatBytes(bytes: number): string {
  const mb = bytes / 1024 / 1024;
  if (mb < 1024) return `${mb.toFixed(1)} MB`;
  return `${(mb / 1024).toFixed(2)} GB`;
}

export function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return `${m}m ${s.toString().padStart(2, "0")}s`;
}
