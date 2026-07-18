<script lang="ts">
  import { onMount } from 'svelte';
  import logoUrl from '../../icons/icon.png';
  import {
    getSettings,
    listenRuntime,
    openTranscriptHistory,
    previewSpokenVoice,
    startTranslation,
    stopSpokenVoicePreview,
    stopTranslation,
    toggleSubtitlePreview,
    updateSettings,
    type RuntimeState,
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

  const isEnglish = () => settings?.appLanguage === 'en';
  const tr = (zh: string, en: string) => (isEnglish() ? en : zh);

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

  async function setVoiceReturn(enabled: boolean): Promise<void> {
    if (!settings) return;
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

  onMount(() => {
    let unlisten: () => void = () => undefined;
    void getSettings().then((data) => {
      payload = data;
      settings = { ...data.settings };
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
    return () => {
      clearPreviewTimer();
      void stopSpokenVoicePreview();
      unlisten();
    };
  });
</script>

<svelte:head>
  <title>Hen Local Translator</title>
</svelte:head>

{#if settings && payload}
  <main class="app-shell">
    <header class="topbar">
      <div class="brand">
        <img class="brand-logo" src={logoUrl} alt="" />
        <div>
          <h1>{tr('很LOCAL / 实时翻译', 'HEN LOCAL / LIVE TRANSLATION')}</h1>
          <p>{tr('离线 · 私密 · 本地处理', 'OFFLINE · PRIVATE · ON-DEVICE')}</p>
        </div>
      </div>
      <div class="header-actions">
        <div class:active={runtimeStatus === 'listening'} class="status-line">
          <span></span>{tr(
            runtimeStatus === 'listening' ? '正在聆听' : runtimeStatus === 'warming' ? '正在准备' : '本地 AI 就绪',
            runtimeStatus === 'listening' ? 'LISTENING' : runtimeStatus === 'warming' ? 'WARMING UP' : 'LOCAL AI READY'
          )}
        </div>
        <button class="outline-button" on:click={() => openTranscriptHistory()}>{tr('转录记录', 'TRANSCRIPTS')}</button>
        <button class="outline-button square" aria-label={tr('设置', 'Settings')} on:click={() => appSettingsOpen = true}>设置</button>
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
            <span class="route-heading"><strong>{tr('原语言', 'SOURCE LANGUAGE')}</strong></span>
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
              <button class:active={settings.overlayFullscreen} on:click={async () => { settings!.overlayFullscreen = true; settings = { ...settings! }; await persist(); }}>{tr('全屏窗', 'LARGE')}</button>
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
          <strong>{tr('语音回传', 'VOICE RETURN')}</strong>
          <span>{tr('音色与设备设置', 'VOICE + DEVICE SETTINGS')}</span>
        </div>
        <div class="row-controls voice-controls">
          <div class="control-block voice-toggle-control">
            <span class="control-label">{tr('回传开关', 'VOICE OUTPUT')}</span>
            <div class="segmented two">
              <button class:active={!settings.spokenTranslationEnabled} on:click={() => setVoiceReturn(false)}>{tr('关', 'OFF')}</button>
              <button class:active={settings.spokenTranslationEnabled} on:click={() => setVoiceReturn(true)}>{tr('开', 'ON')}</button>
            </div>
          </div>

          <div class:is-disabled={!settings.spokenTranslationEnabled} class="control-block voice-picker-control">
            <span class="control-label">{tr('朗读音色', 'VOICE')} · {languageName(settings.targetLanguage)}</span>
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
        {#if errorMessage}<strong class="error">{errorMessage}</strong>{:else}<span>{runtimeMessage}</span>{/if}
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
        <p class="privacy-note">{tr('语音、字幕和偏好设置均保留在本机。', 'Audio, subtitles, and preferences remain on this device.')}</p>
      </dialog>
    </div>
  {/if}
{:else}
  <main class="loading-screen"><img class="brand-logo" src={logoUrl} alt="" /><p>LOADING LOCAL TRANSLATOR</p></main>
{/if}
