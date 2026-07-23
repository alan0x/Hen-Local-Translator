import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';

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
  installedAppleVoices: AppleSystemVoice[];
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

export interface ModelStatus {
  coreReady: boolean;
  downloading: boolean;
  component: 'core' | null;
  progress: number;
  title: string;
  detail: string;
  coreDownloadBytes: number;
}

export interface UpdateStatus {
  currentVersion: string;
  available: boolean;
  version: string | null;
  notes: string | null;
  date: string | null;
  contentLength: number | null;
  downloaded: number;
  downloading: boolean;
  installed: boolean;
}

export interface UsageSnapshot {
  currentSessionSeconds: number;
  monthlySeconds: number;
  lifetimeSeconds: number;
  completedSessions: number;
  comparisonRatePerMinute: number;
  estimatedValue: number;
  running: boolean;
  monthKey: string;
}

export interface AccountDevice {
  id: string;
  friendlyName: string;
  activatedAt: string;
  lastUsedAt: string;
  deactivatedAt: string | null;
  current: boolean;
}

export interface AccountStatus {
  configured: boolean;
  signedIn: boolean;
  email: string | null;
  subscriptionState: string;
  entitlementSource: string;
  accessUntil: string | null;
  licenseValid: boolean;
  leaseExpiresAt: string | null;
  currentDeviceId: string | null;
  devices: AccountDevice[];
  message: string;
}

let pendingUpdate: Update | null = null;

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

export interface AppleSystemVoice {
  name: string;
  locale: string;
  sample: string;
}

export const previewSettings: SettingsPayload = {
  settings: {
    appLanguage: 'zh',
    accentTheme: 'neon-blue',
    sourceLanguage: 'zh',
    targetLanguage: 'en',
    inputDevice: '__system_audio__',
    overlayFullscreen: true,
    subtitleSplit: false,
    overlayOpacity: 1,
    fontSizePreset: '24',
    anchorPositionPreset: '50',
    spokenTranslationEnabled: false,
    spokenTranslationOutputDevice: null,
    spokenTranslationVoice: 'apple-voice-1',
    autoSaveTranscript: false,
    periodicSaveTranscript: false,
    transcriptFileName: 'transcript.md',
    transcriptSaveDir: null
  },
  inputDevices: ['__system_audio__', '__default_microphone__', 'MacBook Pro Microphone'],
  outputDevices: ['MacBook Pro Speakers'],
  installedAppleVoices: [
    { name: 'Yue (Premium)', locale: 'zh_CN', sample: '你好！我叫月。' },
    { name: 'Tingting', locale: 'zh_CN', sample: '你好！我叫婷婷。' },
    { name: 'Voice 1', locale: 'en_US', sample: 'Hi, I’m Siri!' },
    { name: 'Voice 2', locale: 'en_US', sample: 'Hi, I’m Siri!' },
    { name: 'Voice 3', locale: 'en_US', sample: 'Hi, I’m Siri!' },
    { name: 'Voice 4', locale: 'en_US', sample: 'Hi, I’m Siri!' },
    { name: 'Voice 5', locale: 'en_US', sample: 'Hi, I’m Siri!' }
  ],
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
  subtitleSplit: false,
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

export async function getModelStatus(): Promise<ModelStatus> {
  if (isTauri()) return invoke<ModelStatus>('get_model_status');
  return {
    coreReady: true,
    downloading: false,
    component: null,
    progress: 1,
    title: 'Ready',
    detail: 'Models installed',
    coreDownloadBytes: 4_222_472_192
  };
}

export async function startModelDownload(component: 'core'): Promise<ModelStatus> {
  if (isTauri()) return invoke<ModelStatus>('start_model_download', { component });
  return getModelStatus();
}

export async function checkForUpdates(): Promise<UpdateStatus> {
  const currentVersion = isTauri() ? await getVersion() : '1.2.0-beta.1';
  if (!isTauri()) {
    return { currentVersion, available: false, version: null, notes: null, date: null, contentLength: null, downloaded: 0, downloading: false, installed: false };
  }
  pendingUpdate?.close();
  pendingUpdate = await check({ timeout: 30_000 });
  return {
    currentVersion,
    available: Boolean(pendingUpdate),
    version: pendingUpdate?.version ?? null,
    notes: pendingUpdate?.body ?? null,
    date: pendingUpdate?.date ?? null,
    contentLength: null,
    downloaded: 0,
    downloading: false,
    installed: false
  };
}

export async function downloadUpdate(onProgress: (status: UpdateStatus) => void): Promise<UpdateStatus> {
  if (!pendingUpdate) throw new Error('No update is ready to download');
  let downloaded = 0;
  let contentLength: number | null = null;
  const base: UpdateStatus = {
    currentVersion: pendingUpdate.currentVersion,
    available: true,
    version: pendingUpdate.version,
    notes: pendingUpdate.body ?? null,
    date: pendingUpdate.date ?? null,
    contentLength,
    downloaded,
    downloading: true,
    installed: false
  };
  await pendingUpdate.download((event) => {
    if (event.event === 'Started') contentLength = event.data.contentLength ?? null;
    if (event.event === 'Progress') downloaded += event.data.chunkLength;
    onProgress({ ...base, contentLength, downloaded });
  }, { timeout: 30 * 60_000 });
  return { ...base, contentLength, downloaded, downloading: false };
}

export async function installDownloadedUpdate(restartNow: boolean): Promise<void> {
  if (!pendingUpdate) throw new Error('No downloaded update is ready to install');
  await pendingUpdate.install();
  if (restartNow) await relaunch();
}

export async function updateSettings(settings: TranslationSettings): Promise<void> {
  if (isTauri()) await invoke('update_settings', { settings });
}

export async function getUsage(): Promise<UsageSnapshot> {
  if (isTauri()) return invoke<UsageSnapshot>('get_usage');
  return {
    currentSessionSeconds: 0,
    monthlySeconds: 4_380,
    lifetimeSeconds: 18_240,
    completedSessions: 12,
    comparisonRatePerMinute: 1.5,
    estimatedValue: 456,
    running: false,
    monthKey: new Date().toISOString().slice(0, 7)
  };
}

export async function setUsageComparisonRate(rate: number): Promise<UsageSnapshot> {
  if (isTauri()) return invoke<UsageSnapshot>('set_usage_comparison_rate', { rate });
  const usage = await getUsage();
  return { ...usage, comparisonRatePerMinute: rate, estimatedValue: usage.lifetimeSeconds / 60 * rate };
}

const previewAccount: AccountStatus = {
  configured: false,
  signedIn: false,
  email: null,
  subscriptionState: 'not_configured',
  entitlementSource: 'not_configured',
  accessUntil: null,
  licenseValid: false,
  leaseExpiresAt: null,
  currentDeviceId: null,
  devices: [],
  message: 'Account service is not configured in this internal build'
};

const previewSignedInAccount: AccountStatus = {
  configured: true,
  signedIn: true,
  email: 'hello@henlocal.test',
  subscriptionState: 'trialing',
  entitlementSource: 'trial',
  accessUntil: '2026-07-28T12:00:00.000Z',
  licenseValid: true,
  leaseExpiresAt: '2026-07-28T12:00:00.000Z',
  currentDeviceId: 'preview-device-1',
  devices: [
    { id: 'preview-device-1', friendlyName: 'Haochen’s MacBook Pro', activatedAt: '2026-07-21T12:00:00.000Z', lastUsedAt: '2026-07-21T12:00:00.000Z', deactivatedAt: null, current: true },
    { id: 'preview-device-2', friendlyName: 'Studio Mac', activatedAt: '2026-07-20T12:00:00.000Z', lastUsedAt: '2026-07-20T12:00:00.000Z', deactivatedAt: null, current: false }
  ],
  message: 'License is ready for offline translation'
};

function browserPreviewAccount(): AccountStatus {
  const signedIn = typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('account') === '1';
  return structuredClone(signedIn ? previewSignedInAccount : previewAccount);
}

export async function getAccountStatus(): Promise<AccountStatus> {
  return isTauri() ? invoke<AccountStatus>('get_account_status') : browserPreviewAccount();
}

export async function beginAccountSignIn(): Promise<AccountStatus> {
  return isTauri() ? invoke<AccountStatus>('begin_account_sign_in') : browserPreviewAccount();
}

export async function refreshAccount(): Promise<AccountStatus> {
  return isTauri() ? invoke<AccountStatus>('refresh_account') : browserPreviewAccount();
}

export async function openAccountCheckout(): Promise<void> {
  if (isTauri()) await invoke('open_account_checkout');
}

export async function openAccountPortal(): Promise<void> {
  if (isTauri()) await invoke('open_account_portal');
}

export async function deactivateAccountDevice(deviceId: string): Promise<AccountStatus> {
  if (isTauri()) return invoke<AccountStatus>('deactivate_account_device', { deviceId });
  return browserPreviewAccount();
}

export async function signOutAccount(): Promise<AccountStatus> {
  return isTauri() ? invoke<AccountStatus>('sign_out_account') : browserPreviewAccount();
}

export async function listenAccountStatus(handler: (status: AccountStatus) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<AccountStatus>('account-status', ({ payload }) => handler(payload));
}

export async function listenAccountError(handler: (message: string) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<string>('account-error', ({ payload }) => handler(payload));
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

export async function previewSpokenVoice(voice: string, language: string): Promise<void> {
  if (isTauri()) await invoke('preview_spoken_voice', { voice, language });
}

export async function stopSpokenVoicePreview(): Promise<void> {
  if (isTauri()) await invoke('stop_spoken_voice_preview');
}

export async function listAppleVoices(): Promise<AppleSystemVoice[]> {
  return isTauri() ? invoke<AppleSystemVoice[]>('list_apple_voices') : [];
}

export async function openAppleVoiceSettings(): Promise<void> {
  if (isTauri()) await invoke('open_apple_voice_settings');
}

export async function listOutputDevices(): Promise<string[]> {
  return isTauri() ? invoke<string[]>('list_output_devices') : previewSettings.outputDevices;
}

export async function previewAppleVoice(name: string, locale: string): Promise<void> {
  if (isTauri()) await invoke('preview_apple_voice', { name, locale });
}

export async function openVoiceLab(): Promise<void> {
  if (isTauri()) await invoke('open_voice_lab');
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
