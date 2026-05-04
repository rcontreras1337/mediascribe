// Typed wrappers around tauri::invoke. Keep this file as the single boundary
// where untyped strings turn into typed function calls; Svelte components
// import from here.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ModelDownloadProgress,
  Settings,
  TranscribeProgress,
  TranscribeRequest,
  TranscribeResponse,
} from "./types";

// === Settings ===

export const getSettings = (): Promise<Settings> => invoke("get_settings");

export const saveSettings = (settings: Settings): Promise<void> =>
  invoke("save_settings", { settingsValue: settings });

// === API key (keystore) ===

export const setApiKey = (key: string): Promise<void> => invoke("set_api_key", { key });

export const hasApiKey = (): Promise<boolean> => invoke("has_api_key");

export const deleteApiKey = (): Promise<void> => invoke("delete_api_key");

// === Models ===

export const downloadModel = (modelId: string): Promise<string> =>
  invoke("download_model", { modelId });

export const onModelDownloadProgress = (
  cb: (p: ModelDownloadProgress) => void,
): Promise<UnlistenFn> =>
  listen<ModelDownloadProgress>("model-download-progress", (event) => cb(event.payload));

// === Transcribe ===

export const transcribe = (request: TranscribeRequest): Promise<TranscribeResponse> =>
  invoke("transcribe", { request });

export const onTranscribeProgress = (
  cb: (p: TranscribeProgress) => void,
): Promise<UnlistenFn> =>
  listen<TranscribeProgress>("transcribe-progress", (event) => cb(event.payload));
