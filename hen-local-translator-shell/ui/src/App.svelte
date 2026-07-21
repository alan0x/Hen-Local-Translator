<script lang="ts">
  import { onMount } from 'svelte';
  import logoUrl from '../../icons/logo-mark.png';
  import {
    getSettings,
    getModelStatus,
    getUsage,
    checkForUpdates,
    downloadUpdate,
    installDownloadedUpdate,
    listenRuntime,
    openTranscriptHistory,
    previewSpokenVoice,
    startTranslation,
    startModelDownload,
    setUsageComparisonRate,
    stopSpokenVoicePreview,
    stopTranslation,
    toggleSubtitlePreview,
    updateSettings,
    type AccentTheme,
    type RuntimeState,
    type ModelStatus,
    type UpdateStatus,
    type UsageSnapshot,
    type SettingsPayload,
    type TranslationSettings
  } from './lib/api';

  const languages = [
    { code: 'zh', zh: '中文', en: 'Chinese' },
    { code: 'en', zh: '英语', en: 'English' },
    { code: 'ja', zh: '日语', en: 'Japanese' },
    { code: 'fr', zh: '法语', en: 'French' }
  ] as const;
  const fontSizes = ['16', '20', '24', '30', '36', '44', '52', '64', '80', '96', '120', '160'];
  const spokenVoices = [
    { id: 'vivian', language: 'zh', zh: '薇薇安', en: 'Vivian' },
    { id: 'serena', language: 'zh', zh: '赛琳娜', en: 'Serena' },
    { id: 'baiyang', language: 'zh', zh: '白杨', en: 'Baiyang' },
    { id: 'yangyang', language: 'zh', zh: '杨阳', en: 'Yangyang' },
    { id: 'ryan', language: 'en', zh: 'Ryan', en: 'Ryan' },
    { id: 'aiden', language: 'en', zh: 'Aiden', en: 'Aiden' },
    { id: 'maple', language: 'en', zh: 'Maple', en: 'Maple' },
    { id: 'juniper', language: 'en', zh: 'Juniper', en: 'Juniper' }
  ] as const;
  const accentThemes: Array<{ id: AccentTheme; zh: string; en: string; color: string }> = [
    { id: 'neon-blue', zh: '电光蓝', en: 'BLUE', color: '#0003FE' },
    { id: 'neon-orange', zh: '霓虹橙', en: 'ORANGE', color: '#FF5705' },
    { id: 'neon-pink', zh: '霓虹粉', en: 'PINK', color: '#FF0073' },
    { id: 'neon-green', zh: '霓虹绿', en: 'GREEN', color: '#51F91B' }
  ];

  let payload: SettingsPayload | null = null;
  let settings: TranslationSettings | null = null;
  let running = false;
  let runtimeStatus = 'idle';
  let runtimeMessage = 'Local AI is ready';
  let advancedOpen = false;
  let appSettingsOpen = false;
  let subtitlePreviewVisible = true;
  let subtitlePreviewBusy = false;
  let previewingVoice = '';
  let previewTimer: number | null = null;
  let busy = false;
  let errorMessage = '';
  let modelStatus: ModelStatus | null = null;
  let modelTimer: number | null = null;
  let updateStatus: UpdateStatus | null = null;
  let updateBusy = false;
  let updateError = '';
  let updateDialogOpen = false;
  let updateDownloaded = false;
  let usage: UsageSnapshot | null = null;
  let usageTimer: number | null = null;
  let comparisonRateDraft = '1.50';

  const isEnglish = () => settings?.appLanguage === 'en';
  const tr = (zh: string, en: string) => (isEnglish() ? en : zh);
  const appDisplayName = () => tr('很 Local 实时翻译', 'Hen Local Live Translator');

  function applyAccentTheme(theme: AccentTheme): void {
    document.documentElement.dataset.accentTheme = theme;
  }

  async function selectAccentTheme(theme: AccentTheme): Promise<void> {
    if (!settings) return;
    settings.accentTheme = theme;
    settings = { ...settings };
    applyAccentTheme(theme);
    await persist();
  }

  function languageName(code: string): string {
    if (code === 'none') return tr('不翻译', 'No translation');
    const language = languages.find((item) => item.code === code);
    return language ? (isEnglish() ? language.en : language.zh) : code.toUpperCase();
  }

  function deviceName(value: string): string {
    if (value === '__system_audio__') return tr('系统音频', 'System Audio');
    if (value === '__default_microphone__') return tr('默认麦克风', 'Default Microphone');
    return value;
  }

  function voicesForTarget(target: string) {
    if (target === 'none') return spokenVoices;
    const matched = spokenVoices.filter((voice) => voice.language === target);
    return matched.length > 0 ? matched : spokenVoices;
  }

  function syncVoiceToTarget(): boolean {
    if (!settings) return false;
    const available = voicesForTarget(settings.targetLanguage);
    const current = settings.spokenTranslationVoice;
    if (current && available.some((voice) => voice.id === current)) return false;
    settings.spokenTranslationVoice = available[0]?.id ?? 'vivian';
    return true;
  }

  async function persist(): Promise<void> {
    if (!settings) return;
    errorMessage = '';
    try {
      await updateSettings(settings);
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function swapLanguages(): Promise<void> {
    if (!settings || settings.targetLanguage === 'none') return;
    await stopVoicePreview();
    const source = settings.sourceLanguage;
    settings.sourceLanguage = settings.targetLanguage;
    settings.targetLanguage = source;
    syncVoiceToTarget();
    settings = { ...settings };
    await persist();
  }

  async function changeTargetLanguage(): Promise<void> {
    await stopVoicePreview();
    syncVoiceToTarget();
    settings = { ...settings! };
    await persist();
  }

  async function adjustFont(direction: number): Promise<void> {
    if (!settings) return;
    const current = Math.max(0, fontSizes.indexOf(settings.fontSizePreset));
    const next = Math.min(fontSizes.length - 1, Math.max(0, current + direction));
    settings.fontSizePreset = fontSizes[next];
    settings = { ...settings };
    await persist();
  }

  async function toggleTranslation(): Promise<void> {
    if (!settings || busy) return;
    busy = true;
    errorMessage = '';
    try {
      const wasRunning = running;
      const state = wasRunning ? await stopTranslation() : await startTranslation(settings);
      applyRuntime(state);
      if (!wasRunning) subtitlePreviewVisible = false;
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  function clearPreviewTimer(): void {
    if (previewTimer !== null) {
      window.clearTimeout(previewTimer);
      previewTimer = null;
    }
  }

  async function stopVoicePreview(): Promise<void> {
    clearPreviewTimer();
    previewingVoice = '';
    try {
      await stopSpokenVoicePreview();
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function setSpokenTranslation(enabled: boolean): Promise<void> {
    if (!settings) return;
    if (enabled && modelStatus && !modelStatus.speechReady) {
      appSettingsOpen = true;
      await downloadModels('speech');
      return;
    }
    if (!enabled) await stopVoicePreview();
    settings.spokenTranslationEnabled = enabled;
    settings = { ...settings };
    await persist();
  }

  async function selectSpokenVoice(): Promise<void> {
    await stopVoicePreview();
    await persist();
  }

  async function playVoicePreview(): Promise<void> {
    if (!settings?.spokenTranslationEnabled) return;
    const voice = settings.spokenTranslationVoice ?? 'vivian';
    if (previewingVoice === voice) {
      await stopVoicePreview();
      return;
    }

    errorMessage = '';
    clearPreviewTimer();
    try {
      await previewSpokenVoice(voice);
      previewingVoice = voice;
      previewTimer = window.setTimeout(() => {
        previewingVoice = '';
        previewTimer = null;
      }, 4600);
    } catch (error) {
      previewingVoice = '';
      errorMessage = String(error);
    }
  }

  async function toggleTestSubtitles(): Promise<void> {
    if (running || subtitlePreviewBusy) return;
    subtitlePreviewBusy = true;
    errorMessage = '';
    try {
      subtitlePreviewVisible = await toggleSubtitlePreview();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      subtitlePreviewBusy = false;
    }
  }

  function applyRuntime(state: RuntimeState): void {
    running = state.running;
    runtimeStatus = state.status;
    runtimeMessage = state.message;
  }

  function formatDownloadSize(bytes: number): string {
    return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  }

  function formatDuration(seconds: number): string {
    const rounded = Math.max(0, Math.floor(seconds));
    const hours = Math.floor(rounded / 3600);
    const minutes = Math.floor((rounded % 3600) / 60);
    const remainder = rounded % 60;
    return hours > 0
      ? `${hours}:${String(minutes).padStart(2, '0')}:${String(remainder).padStart(2, '0')}`
      : `${minutes}:${String(remainder).padStart(2, '0')}`;
  }

  async function refreshUsage(): Promise<void> {
    try {
      const latest = await getUsage();
      if (document.activeElement?.id === 'comparison-rate') {
        const draftRate = Number(comparisonRateDraft);
        usage = Number.isFinite(draftRate) && draftRate >= 0 && draftRate <= 100
          ? { ...latest, comparisonRatePerMinute: draftRate, estimatedValue: latest.lifetimeSeconds / 60 * draftRate }
          : latest;
      } else {
        usage = latest;
        comparisonRateDraft = latest.comparisonRatePerMinute.toFixed(2);
      }
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function saveComparisonRate(): Promise<void> {
    const rate = Number(comparisonRateDraft);
    if (!Number.isFinite(rate)) return;
    try {
      usage = await setUsageComparisonRate(rate);
      comparisonRateDraft = usage.comparisonRatePerMinute.toFixed(2);
    } catch (error) {
      errorMessage = String(error);
    }
  }

  function previewComparisonRate(): void {
    const rate = Number(comparisonRateDraft);
    if (!usage || !Number.isFinite(rate) || rate < 0 || rate > 100) return;
    usage = {
      ...usage,
      comparisonRatePerMinute: rate,
      estimatedValue: usage.lifetimeSeconds / 60 * rate
    };
  }

  async function refreshModelStatus(): Promise<void> {
    try {
      modelStatus = await getModelStatus();
      if (modelStatus.coreReady && modelStatus.speechReady && modelTimer !== null) {
        window.clearInterval(modelTimer);
        modelTimer = null;
      }
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function downloadModels(component: 'core' | 'speech'): Promise<void> {
    errorMessage = '';
    try {
      modelStatus = await startModelDownload(component);
      if (modelTimer === null) {
        modelTimer = window.setInterval(() => void refreshModelStatus(), 750);
      }
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function checkUpdates(manual = true): Promise<void> {
    if (updateBusy) return;
    updateBusy = true;
    updateError = '';
    try {
      updateStatus = await checkForUpdates();
      localStorage.setItem('hen-local-last-update-check', String(Date.now()));
      if (updateStatus.available) updateDialogOpen = true;
    } catch (error) {
      if (manual) updateError = String(error);
    } finally {
      updateBusy = false;
    }
  }

  async function prepareUpdate(): Promise<boolean> {
    if (!updateStatus?.available) return false;
    if (updateDownloaded) return true;
    updateBusy = true;
    updateError = '';
    try {
      updateStatus = await downloadUpdate((status) => { updateStatus = status; });
      updateDownloaded = true;
      return true;
    } catch (error) {
      updateError = String(error);
      return false;
    } finally {
      updateBusy = false;
    }
  }

  async function applyUpdate(restartNow: boolean): Promise<void> {
    if (running) {
      updateError = tr('请先停止实时翻译，再安装更新。', 'Stop live translation before installing the update.');
      return;
    }
    if (!(await prepareUpdate())) return;
    updateBusy = true;
    try {
      await installDownloadedUpdate(restartNow);
      if (!restartNow) updateDialogOpen = false;
    } catch (error) {
      updateError = String(error);
    } finally {
      updateBusy = false;
    }
  }

  function runtimeDisplayMessage(): string {
    if (runtimeStatus === 'error') return runtimeMessage;
    if (runtimeStatus === 'listening') return tr('实时翻译进行中', 'Live translation active');
    if (runtimeStatus === 'warming') return tr('正在启动本地翻译…', 'Starting local translation…');
    return tr('本地 AI 已就绪', 'Local AI is ready');
  }

  onMount(() => {
    let unlisten: () => void = () => undefined;
    void getSettings().then((data) => {
      payload = data;
      settings = { ...data.settings };
      applyAccentTheme(settings.accentTheme);
      subtitlePreviewVisible = data.subtitlePreviewVisible;
      if (syncVoiceToTarget()) {
        settings = { ...settings };
        void updateSettings(settings);
      }
      running = data.running;
      runtimeStatus = data.runtimeStatus;
      runtimeMessage = data.runtimeMessage;
    }).catch((error) => {
      errorMessage = String(error);
    });
    void listenRuntime(applyRuntime).then((cleanup) => { unlisten = cleanup; });
    void refreshModelStatus();
    void refreshUsage();
    usageTimer = window.setInterval(() => void refreshUsage(), 1000);
    const lastUpdateCheck = Number(localStorage.getItem('hen-local-last-update-check') ?? '0');
    if (Date.now() - lastUpdateCheck > 24 * 60 * 60 * 1000) {
      window.setTimeout(() => void checkUpdates(false), 2500);
    }
    return () => {
      clearPreviewTimer();
      if (modelTimer !== null) window.clearInterval(modelTimer);
      if (usageTimer !== null) window.clearInterval(usageTimer);
      void stopSpokenVoicePreview();
      unlisten();
    };
  });
</script>

<svelte:head>
  <title>{appDisplayName()}</title>
</svelte:head>

{#if settings && payload}
  <main class="app-shell">
    <header class="topbar">
      <div class="brand">
        <span class="brand-logo-tile"><span class="brand-logo" style={`--brand-mark:url("${logoUrl}")`} aria-hidden="true"></span></span>
        <div class="brand-copy">
          <h1>{tr('很 LOCAL 实时翻译', 'HEN LOCAL LIVE TRANSLATOR')}</h1>
          <p>{tr('离线 · 私密 · 本地处理', 'OFFLINE · PRIVATE · ON-DEVICE')}</p>
        </div>
      </div>
      <div class="header-actions">
        <div class:active={runtimeStatus === 'listening'} class="status-line">
          <span></span>{tr(
            runtimeStatus === 'listening' ? '翻译中' : runtimeStatus === 'warming' ? '正在准备' : '本地 AI 就绪',
            runtimeStatus === 'listening' ? 'TRANSLATING' : runtimeStatus === 'warming' ? 'WARMING UP' : 'LOCAL AI READY'
          )}
        </div>
        <button class="outline-button" on:click={() => openTranscriptHistory()}>{tr('转录记录', 'TRANSCRIPTS')}</button>
        <button
          class="outline-button square settings-button"
          aria-label={tr('设置', 'Settings')}
          title={tr('设置', 'Settings')}
          on:click={() => appSettingsOpen = true}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 7h3M11 7h9M4 17h9M17 17h3"></path>
            <circle cx="9" cy="7" r="2"></circle>
            <circle cx="15" cy="17" r="2"></circle>
          </svg>
        </button>
      </div>
    </header>

    <section class="control-grid">
      <div class="grid-row route-row">
        <div class="row-number">01</div>
        <div class="row-title">
          <strong>{tr('语言与音频', 'LANGUAGE + AUDIO')}</strong>
          <span>{tr('输入路线', 'INPUT ROUTE')}</span>
        </div>
        <div class="row-controls route-controls">
          <label class="route-field">
            <span class="route-heading"><strong>{tr('源语言', 'SOURCE LANGUAGE')}</strong></span>
            <select bind:value={settings.sourceLanguage} on:change={persist}>
              {#each languages as language}
                <option value={language.code}>{isEnglish() ? language.en : language.zh}</option>
              {/each}
            </select>
          </label>

          <button class="swap-button" aria-label={tr('交换语言', 'Swap languages')} on:click={swapLanguages}>⇄</button>

          <label class="route-field">
            <span class="route-heading"><strong>{tr('目标语言', 'TARGET LANGUAGE')}</strong></span>
            <select bind:value={settings.targetLanguage} on:change={changeTargetLanguage}>
              {#each languages as language}
                <option value={language.code}>{isEnglish() ? language.en : language.zh}</option>
              {/each}
              <option value="none">{tr('不翻译', 'No translation')}</option>
            </select>
          </label>

          <label class="route-field audio-route-field">
            <span class="route-heading"><strong>{tr('输入音频', 'AUDIO INPUT')}</strong></span>
            <select bind:value={settings.inputDevice} on:change={persist}>
              {#each payload.inputDevices as device}
                <option value={device}>{deviceName(device)}</option>
              {/each}
            </select>
          </label>
        </div>
      </div>

      <div class="grid-row subtitle-row">
        <div class="row-number">02</div>
        <div class="row-title">
          <strong>{tr('字幕窗口', 'SUBTITLE WINDOW')}</strong>
          <span>{tr('显示方式', 'DISPLAY')}</span>
        </div>
        <div class="row-controls subtitle-controls">
          <div class="control-block">
            <span class="control-label">{tr('窗口模式', 'WINDOW MODE')}</span>
            <div class="segmented two">
              <button class:active={!settings.overlayFullscreen} on:click={async () => { settings!.overlayFullscreen = false; settings = { ...settings! }; await persist(); }}>{tr('浮窗', 'FLOAT')}</button>
              <button class:active={settings.overlayFullscreen} on:click={async () => { settings!.overlayFullscreen = true; settings = { ...settings! }; await persist(); }}>{tr('全屏', 'FULLSCREEN')}</button>
            </div>
          </div>

          <div class="control-block font-control">
            <span class="control-label">{tr('字体大小', 'TYPE SIZE')}</span>
            <div class="stepper">
              <button aria-label={tr('减小字体', 'Decrease type size')} on:click={() => adjustFont(-1)}>−</button>
              <output>{settings.fontSizePreset} PT</output>
              <button aria-label={tr('增大字体', 'Increase type size')} on:click={() => adjustFont(1)}>+</button>
            </div>
          </div>

          <div class="control-block layout-control">
            <span class="control-label">{tr('内容样式', 'LAYOUT')}</span>
            <div class="segmented two">
              <button class:active={!settings.subtitleSplit} on:click={async () => { settings!.subtitleSplit = false; settings = { ...settings! }; await persist(); }}>{tr('上下对照', 'STACKED')}</button>
              <button class:active={settings.subtitleSplit} on:click={async () => { settings!.subtitleSplit = true; settings = { ...settings! }; await persist(); }}>{tr('逐句双行', 'SENTENCE PAIRS')}</button>
            </div>
          </div>

          <div class="control-block subtitle-action-control">
            <span class="control-label">{tr('字幕调整', 'SUBTITLE TOOLS')}</span>
            <div class="subtitle-action-pair">
              <button class:active={subtitlePreviewVisible && !running} disabled={running || subtitlePreviewBusy} class="subtitle-preview-button" on:click={toggleTestSubtitles}>
                {running ? tr('实时字幕中', 'LIVE') : subtitlePreviewVisible ? tr('清空字幕', 'CLEAR') : tr('测试字幕', 'TEST')}
              </button>
              <button class="wide-outline" on:click={() => advancedOpen = true}>{tr('高级设置', 'ADVANCED')} <span>↗</span></button>
            </div>
          </div>
        </div>
      </div>

      <div class="grid-row voice-row">
        <div class="row-number">03</div>
        <div class="row-title">
          <strong>{tr('译文播报', 'SPOKEN TRANSLATION')}</strong>
          <span>{tr('音色与输出设备', 'VOICE + OUTPUT')}</span>
        </div>
        <div class="row-controls voice-controls">
          <div class="control-block voice-toggle-control">
            <span class="control-label">{tr('播报开关', 'SPEECH OUTPUT')}</span>
            <div class="segmented two">
              <button class:active={!settings.spokenTranslationEnabled} on:click={() => setSpokenTranslation(false)}>{tr('关', 'OFF')}</button>
              <button class:active={settings.spokenTranslationEnabled} on:click={() => setSpokenTranslation(true)}>{tr('开', 'ON')}</button>
            </div>
          </div>

          <div class:is-disabled={!settings.spokenTranslationEnabled} class="control-block voice-picker-control">
            <span class="control-label">{tr('播报音色', 'VOICE')} · {languageName(settings.targetLanguage)}</span>
            <div class="voice-picker-row">
              <select disabled={!settings.spokenTranslationEnabled} bind:value={settings.spokenTranslationVoice} on:change={selectSpokenVoice}>
                {#each voicesForTarget(settings.targetLanguage) as voice}
                  <option value={voice.id}>{isEnglish() ? voice.en : voice.zh}</option>
                {/each}
              </select>
              <button aria-label={previewingVoice === (settings.spokenTranslationVoice ?? 'vivian') ? tr('停止试听', 'Stop preview') : tr('试听音色', 'Preview voice')} title={previewingVoice === (settings.spokenTranslationVoice ?? 'vivian') ? tr('停止试听', 'Stop preview') : tr('试听音色', 'Preview voice')} class:playing={previewingVoice === (settings.spokenTranslationVoice ?? 'vivian')} class="preview-button" type="button" disabled={!settings.spokenTranslationEnabled} on:click={playVoicePreview}>
                <span aria-hidden="true">{previewingVoice === (settings.spokenTranslationVoice ?? 'vivian') ? '■' : '▶'}</span>
              </button>
            </div>
          </div>

          <label class:is-disabled={!settings.spokenTranslationEnabled} class="control-block output-device-control">
            <span class="control-label">{tr('输出设备', 'OUTPUT DEVICE')}</span>
            <select disabled={!settings.spokenTranslationEnabled} bind:value={settings.spokenTranslationOutputDevice} on:change={persist}>
              <option value={null}>{tr('系统默认', 'System Default')}</option>
              {#each payload.outputDevices as device}<option value={device}>{device}</option>{/each}
            </select>
          </label>
        </div>
      </div>
    </section>

    <footer class="launch-area">
      <div class="launch-meta">
        <span>{languageName(settings.sourceLanguage)} → {languageName(settings.targetLanguage)}</span>
        <span>{deviceName(settings.inputDevice)}</span>
        {#if errorMessage}<strong class="error">{errorMessage}</strong>{:else}<span>{runtimeDisplayMessage()}</span>{/if}
        {#if usage && running}<strong class="session-timer">{formatDuration(usage.currentSessionSeconds)}</strong>{/if}
      </div>
      <button class:running class="launch-button" disabled={busy} on:click={toggleTranslation}>
        <span>{running ? '■' : '▶'}</span>
        {busy ? tr('请稍候…', 'PLEASE WAIT…') : running ? tr('停止实时翻译', 'STOP LIVE TRANSLATION') : tr('启动实时翻译', 'START LIVE TRANSLATION')}
      </button>
    </footer>
  </main>

  {#if advancedOpen}
    <div class="modal-backdrop">
      <dialog open class="modal" aria-label={tr('高级字幕设置', 'Advanced subtitle settings')}>
        <header><div><span>02.A</span><h2>{tr('高级字幕设置', 'ADVANCED SUBTITLES')}</h2></div><button on:click={() => advancedOpen = false}>×</button></header>
        <label class="modal-field"><span>{tr('窗口不透明度', 'WINDOW OPACITY')}</span><input type="range" min="0.35" max="1" step="0.05" bind:value={settings.overlayOpacity} on:change={persist} /><output>{Math.round(settings.overlayOpacity * 100)}%</output></label>
        <label class="modal-field"><span>{tr('字幕垂直位置', 'VERTICAL ANCHOR')}</span><select bind:value={settings.anchorPositionPreset} on:change={persist}><option value="35">35%</option><option value="50">50%</option><option value="70">70%</option><option value="100">100%</option></select></label>
        <div class="modal-field choice-field">
          <span>{tr('自动保存转录', 'AUTO-SAVE TRANSCRIPT')}</span>
          <div class="segmented two modal-choice">
            <button class:active={!settings.autoSaveTranscript} on:click={async () => { settings!.autoSaveTranscript = false; settings = { ...settings! }; await persist(); }}>{tr('关', 'OFF')}</button>
            <button class:active={settings.autoSaveTranscript} on:click={async () => { settings!.autoSaveTranscript = true; settings = { ...settings! }; await persist(); }}>{tr('开', 'ON')}</button>
          </div>
        </div>
      </dialog>
    </div>
  {/if}

  {#if appSettingsOpen}
    <div class="modal-backdrop">
      <dialog open class="modal small-modal" aria-label={tr('设置', 'Settings')}>
        <header><div><span>SYS</span><h2>{tr('应用设置', 'APPLICATION')}</h2></div><button on:click={() => appSettingsOpen = false}>×</button></header>
        <div class="modal-field"><span>{tr('界面语言', 'INTERFACE LANGUAGE')}</span><div class="segmented two language-toggle"><button class:active={settings.appLanguage === 'zh'} on:click={async () => { settings!.appLanguage = 'zh'; settings = { ...settings! }; await persist(); }}>中文</button><button class:active={settings.appLanguage === 'en'} on:click={async () => { settings!.appLanguage = 'en'; settings = { ...settings! }; await persist(); }}>EN</button></div></div>
        <div class="modal-field theme-field">
          <span>{tr('主题颜色', 'ACCENT THEME')}</span>
          <div class="theme-options">
            {#each accentThemes as theme}
              <button
                class:active={settings.accentTheme === theme.id}
                aria-label={isEnglish() ? theme.en : theme.zh}
                title={isEnglish() ? theme.en : theme.zh}
                style={`--theme-swatch:${theme.color}`}
                on:click={() => selectAccentTheme(theme.id)}
              >
                <span class="theme-swatch"><span class="theme-mark" style={`--brand-mark:url("${logoUrl}")`}></span></span>
                <small>{isEnglish() ? theme.en : theme.zh}</small>
              </button>
            {/each}
          </div>
        </div>
        {#if modelStatus}
          <section class="model-settings">
            <div class="model-row">
              <div><strong>{tr('核心翻译模型', 'CORE TRANSLATION MODELS')}</strong><small>{formatDownloadSize(modelStatus.coreDownloadBytes)}</small></div>
              <button disabled={modelStatus.coreReady || modelStatus.downloading} on:click={() => downloadModels('core')}>{modelStatus.coreReady ? tr('已安装', 'INSTALLED') : tr('下载', 'DOWNLOAD')}</button>
            </div>
            <div class="model-row">
              <div><strong>{tr('译文播报模型', 'SPOKEN TRANSLATION MODEL')}</strong><small>{formatDownloadSize(modelStatus.speechDownloadBytes)} · {tr('按需下载', 'OPTIONAL')}</small></div>
              <button disabled={modelStatus.speechReady || modelStatus.downloading} on:click={() => downloadModels('speech')}>{modelStatus.speechReady ? tr('已安装', 'INSTALLED') : tr('下载', 'DOWNLOAD')}</button>
            </div>
            {#if modelStatus.downloading}
              <div class="model-progress"><span style={`width:${Math.round(modelStatus.progress * 100)}%`}></span></div>
              <p class="model-detail">{modelStatus.title} · {modelStatus.detail} · {Math.round(modelStatus.progress * 100)}%</p>
            {/if}
          </section>
        {/if}
        <section class="update-settings">
          <div>
            <strong>{tr('软件更新', 'SOFTWARE UPDATE')}</strong>
            <small>{updateStatus ? `${tr('当前版本', 'CURRENT')} ${updateStatus.currentVersion}` : tr('每天自动检查一次', 'CHECKED ONCE A DAY')}</small>
          </div>
          <button disabled={updateBusy} on:click={() => checkUpdates(true)}>{updateBusy ? tr('检查中…', 'CHECKING…') : tr('检查更新', 'CHECK FOR UPDATES')}</button>
          {#if updateStatus && !updateStatus.available && !updateBusy}<p>{tr('已经是最新版本。', 'You are up to date.')}</p>{/if}
          {#if updateError}<p class="update-error">{updateError}</p>{/if}
        </section>
        {#if usage}
          <section class="usage-settings">
            <div class="usage-heading">
              <div><strong>{tr('本机使用统计', 'LOCAL USAGE')}</strong><small>{tr('只保存在这台电脑，不影响订阅费用', 'STORED ONLY ON THIS MAC · NEVER USED FOR BILLING')}</small></div>
              <b>{formatDuration(usage.lifetimeSeconds)}</b>
            </div>
            <div class="usage-grid">
              <div><small>{tr('本月', 'THIS MONTH')}</small><strong>{formatDuration(usage.monthlySeconds)}</strong></div>
              <div><small>{tr('完成会话', 'SESSIONS')}</small><strong>{usage.completedSessions}</strong></div>
              <div><small>{tr('估算价值', 'ESTIMATED VALUE')}</small><strong>${usage.estimatedValue.toFixed(0)}</strong></div>
            </div>
            <label class="rate-setting" for="comparison-rate">
              <span>{tr('云端同类服务比较价', 'CLOUD COMPARISON RATE')}</span>
              <span>$ <input id="comparison-rate" inputmode="decimal" bind:value={comparisonRateDraft} on:input={previewComparisonRate} on:change={saveComparisonRate} on:blur={saveComparisonRate} /> / {tr('分钟', 'MIN')}</span>
            </label>
            <p>{tr('估算公式：累计翻译分钟 × 比较价。这里只是方便了解本地处理的价值，不代表保证节省金额。', 'Estimate = lifetime translation minutes × comparison rate. This is a value illustration, not guaranteed savings.')}</p>
          </section>
        {/if}
        <p class="privacy-note">{tr('语音、字幕和偏好设置均保留在本机。', 'Audio, subtitles, and preferences remain on this device.')}</p>
      </dialog>
    </div>
  {/if}
  {#if modelStatus && !modelStatus.coreReady}
    <div class="modal-backdrop model-setup-backdrop">
      <section class="model-setup" aria-label={tr('下载本地模型', 'Download local models')}>
        <span class="brand-logo-tile"><span class="brand-logo" style={`--brand-mark:url("${logoUrl}")`} aria-hidden="true"></span></span>
        <div>
          <p class="setup-kicker">{tr('首次使用设置', 'FIRST-TIME SETUP')}</p>
          <h2>{tr('下载本地翻译模型', 'DOWNLOAD LOCAL TRANSLATION MODELS')}</h2>
          <p>{tr('模型约 4.2 GB，只需下载一次。语音和字幕始终在这台电脑上处理。', 'The models are about 4.2 GB and download once. Audio and subtitles stay on this Mac.')}</p>
        </div>
        {#if modelStatus.downloading && modelStatus.component === 'core'}
          <div class="setup-progress">
            <div><span style={`width:${Math.round(modelStatus.progress * 100)}%`}></span></div>
            <strong>{Math.round(modelStatus.progress * 100)}%</strong>
            <small>{modelStatus.title} · {modelStatus.detail}</small>
          </div>
        {:else}
          <button class="launch-button" on:click={() => downloadModels('core')}>{tr('下载并继续', 'DOWNLOAD AND CONTINUE')}</button>
        {/if}
        {#if errorMessage}<p class="setup-error">{errorMessage}</p>{/if}
      </section>
    </div>
  {/if}
  {#if updateDialogOpen && updateStatus?.available}
    <div class="modal-backdrop update-backdrop">
      <dialog open class="modal update-modal" aria-label={tr('软件更新', 'Software update')}>
        <header><div><span>UPDATE</span><h2>{tr('发现新版本', 'UPDATE AVAILABLE')}</h2></div><button disabled={updateBusy} on:click={() => updateDialogOpen = false}>×</button></header>
        <div class="update-content">
          <div class="update-version"><span>{updateStatus.currentVersion}</span><b>→</b><strong>{updateStatus.version}</strong></div>
          {#if updateStatus.notes}<p class="update-notes">{updateStatus.notes}</p>{/if}
          {#if updateStatus.downloading || updateDownloaded}
            <div class="model-progress"><span style={`width:${updateStatus.contentLength ? Math.min(100, Math.round(updateStatus.downloaded / updateStatus.contentLength * 100)) : updateDownloaded ? 100 : 12}%`}></span></div>
            <p class="model-detail">{updateDownloaded ? tr('下载完成，可以安装。', 'Download complete. Ready to install.') : tr('正在安全下载更新…', 'Downloading update securely…')}</p>
          {/if}
          {#if running}<p class="update-warning">{tr('实时翻译进行中。停止翻译后才能安装，当前会话不会被打断。', 'Live translation is active. Stop it before installing; this session will not be interrupted.')}</p>{/if}
          {#if updateError}<p class="update-error">{updateError}</p>{/if}
        </div>
        <footer class="update-actions">
          <button disabled={updateBusy || running} on:click={() => applyUpdate(false)}>{tr('退出后生效', 'INSTALL WHEN I QUIT')}</button>
          <button class="primary" disabled={updateBusy || running} on:click={() => applyUpdate(true)}>{updateBusy ? tr('请稍候…', 'PLEASE WAIT…') : tr('重新启动并更新', 'RESTART AND UPDATE')}</button>
        </footer>
      </dialog>
    </div>
  {/if}
{:else}
  <main class="loading-screen"><span class="brand-logo-tile"><span class="brand-logo" style={`--brand-mark:url("${logoUrl}")`} aria-hidden="true"></span></span><p>LOADING LOCAL TRANSLATOR</p></main>
{/if}
