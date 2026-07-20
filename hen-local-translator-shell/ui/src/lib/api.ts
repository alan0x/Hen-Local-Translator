import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export type LanguageCode = 'zh' | 'en' | 'ja' | 'fr' | 'none';

export interface TranslationSettings {
  appLanguage: 'zh' | 'en';
  accentTheme: AccentTheme;
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
  inputDevice: string;
  overlayFullscreen: boolean;
  subtitleSplit: boolean;
  overlayOpacity: number;
  fontSizePreset: string;
  anchorPositionPreset: string;
  spokenTranslationEnabled: boolean;
  spokenTranslationOutputDevice: string | null;
  spokenTranslationVoice: string | null;
  autoSaveTranscript: boolean;
  periodicSaveTranscript: boolean;
  transcriptFileName: string;
  transcriptSaveDir: string | null;
}

export interface SettingsPayload {
  settings: TranslationSettings;
  inputDevices: string[];
  outputDevices: string[];
  subtitlePreviewVisible: boolean;
  running: boolean;
  runtimeStatus: string;
  runtimeMessage: string;
}

export interface RuntimeState {
  running: boolean;
  status: string;
  message: string;
}

export interface Sentence {
  sourceText: string;
  translation: string;
}

export interface OverlayState {
  active: boolean;
  status: string;
  sourceLanguage: string;
  targetLanguage: string;
  subtitleSplit: boolean;
  fontSize: number;
  anchorPosition: number;
  accentTheme: AccentTheme;
  history: Sentence[];
  pendingSourceText: string;
}

export type AccentTheme = 'neon-blue' | 'neon-orange' | 'neon-pink' | 'neon-green';

export const previewSettings: SettingsPayload = {
  settings: {
    appLanguage: 'zh',
    accentTheme: 'neon-blue',
    sourceLanguage: 'zh',
    targetLanguage: 'en',
    inputDevice: '__system_audio__',
    overlayFullscreen: true,
    subtitleSplit: true,
    overlayOpacity: 1,
    fontSizePreset: '24',
    anchorPositionPreset: '50',
    spokenTranslationEnabled: false,
    spokenTranslationOutputDevice: null,
    spokenTranslationVoice: 'vivian',
    autoSaveTranscript: false,
    periodicSaveTranscript: false,
    transcriptFileName: 'transcript.md',
    transcriptSaveDir: null
  },
  inputDevices: ['__system_audio__', '__default_microphone__', 'MacBook Pro Microphone'],
  outputDevices: ['System Default', 'Haochen’s AirPods Pro'],
  subtitlePreviewVisible: true,
  running: false,
  runtimeStatus: 'idle',
  runtimeMessage: 'Local AI is ready'
};

export const previewOverlay: OverlayState = {
  active: false,
  status: 'idle',
  sourceLanguage: 'zh',
  targetLanguage: 'en',
  subtitleSplit: true,
  fontSize: 24,
  anchorPosition: 50,
  accentTheme: 'neon-blue',
  history: [
    {
      sourceText: '这是一段用于调整字幕大小和布局的测试内容。',
      translation: 'This sample helps you adjust subtitle size and layout.'
    },
    {
      sourceText: '请确认每句话都清晰、易读，并适合现场屏幕。',
      translation: 'Check that every sentence is clear, readable, and suitable for the venue screen.'
    }
  ],
  pendingSourceText: ''
};

function previewSentences(count: number): Sentence[] {
  return Array.from({ length: count }, (_, index) => ({
    sourceText: `第 ${index + 1} 句原文会作为独立段落显示，便于观众跟随现场内容。`,
    translation: `Sentence ${index + 1} appears as a separate passage so the audience can follow the live presentation clearly.`
  }));
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);
}

export async function getSettings(): Promise<SettingsPayload> {
  return isTauri() ? invoke<SettingsPayload>('get_settings') : structuredClone(previewSettings);
}

export async function updateSettings(settings: TranslationSettings): Promise<void> {
  if (isTauri()) await invoke('update_settings', { settings });
}

export async function startTranslation(settings: TranslationSettings): Promise<RuntimeState> {
  if (!isTauri()) return { running: true, status: 'warming', message: 'Starting local translation…' };
  return invoke<RuntimeState>('start_translation', { settings });
}

export async function stopTranslation(): Promise<RuntimeState> {
  if (!isTauri()) return { running: false, status: 'idle', message: 'Translation stopped' };
  return invoke<RuntimeState>('stop_translation');
}

export async function getOverlayState(): Promise<OverlayState> {
  if (isTauri()) return invoke<OverlayState>('get_overlay_state');
  const previewMode = new URLSearchParams(window.location.search);
  if (previewMode.get('idle') === '1') {
    return { ...structuredClone(previewOverlay), active: false, status: 'idle', history: [], pendingSourceText: '' };
  }
  const preview = structuredClone(previewOverlay);
  if (previewMode.get('long') === '1') {
    preview.history = previewSentences(12);
    preview.pendingSourceText = '';
  }
  if (previewMode.get('flow') === '1') {
    preview.history = previewSentences(3);
    preview.pendingSourceText = '';
  }
  if (previewMode.get('stacked') === '1') {
    preview.subtitleSplit = false;
  }
  return preview;
}

export async function openTranscriptHistory(): Promise<void> {
  if (isTauri()) await invoke('open_transcript_history');
}

let browserSubtitlePreviewVisible = true;

export async function toggleSubtitlePreview(): Promise<boolean> {
  if (isTauri()) return invoke<boolean>('toggle_subtitle_preview');
  browserSubtitlePreviewVisible = !browserSubtitlePreviewVisible;
  return browserSubtitlePreviewVisible;
}

export async function previewSpokenVoice(voice: string): Promise<void> {
  if (isTauri()) await invoke('preview_spoken_voice', { voice });
}

export async function stopSpokenVoicePreview(): Promise<void> {
  if (isTauri()) await invoke('stop_spoken_voice_preview');
}

export async function startWindowDrag(): Promise<void> {
  if (isTauri()) await getCurrentWindow().startDragging();
}

export async function listenRuntime(handler: (state: RuntimeState) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<RuntimeState>('runtime-state', ({ payload }) => handler(payload));
}

export async function listenOverlay(handler: (state: OverlayState) => void): Promise<UnlistenFn> {
  if (!isTauri()) {
    const previewMode = new URLSearchParams(window.location.search);
    if (previewMode.get('flow') === '1') {
      const flowState = (count: number): OverlayState => ({
        ...structuredClone(previewOverlay),
        subtitleSplit: previewMode.get('stacked') !== '1',
        history: previewSentences(count),
        pendingSourceText: ''
      });
      const firstTimer = window.setTimeout(() => {
        handler(flowState(6));
      }, 450);
      const secondTimer = window.setTimeout(() => {
        handler(flowState(12));
      }, 1050);
      return () => {
        window.clearTimeout(firstTimer);
        window.clearTimeout(secondTimer);
      };
    }
    return () => undefined;
  }
  return listen<OverlayState>('overlay-state', ({ payload }) => handler(payload));
}
