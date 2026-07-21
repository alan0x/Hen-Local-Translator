<script lang="ts">
  import { onMount } from 'svelte';
  import {
    listAppleVoices,
    previewAppleVoice,
    stopSpokenVoicePreview,
    type AppleSystemVoice
  } from './lib/api';

  type Filter = 'all' | 'zh' | 'en' | 'ja' | 'fr' | 'other' | 'favorites';
  const FAVORITES_KEY = 'hen-local-apple-voice-favorites-v1';
  const filters: Array<{ id: Filter; label: string }> = [
    { id: 'all', label: '全部' },
    { id: 'zh', label: '中文' },
    { id: 'en', label: '英文' },
    { id: 'ja', label: '日语' },
    { id: 'fr', label: '法语' },
    { id: 'other', label: '其他' },
    { id: 'favorites', label: '已收藏' }
  ];

  let voices: AppleSystemVoice[] = [];
  let filter: Filter = 'all';
  let query = '';
  let favorites = new Set<string>();
  let playing = '';
  let loading = true;
  let error = '';
  let copied = false;

  const keyFor = (voice: AppleSystemVoice) => `${voice.name}\u0000${voice.locale}`;
  const languageOf = (locale: string) => locale.split('_')[0].toLowerCase();
  const isPrimary = (locale: string) => ['zh', 'en', 'ja', 'fr'].includes(languageOf(locale));
  const isSiriLiveSpeechVoice = (voice: AppleSystemVoice) => voice.locale === 'en_US' && /^Voice [1-5]$/.test(voice.name);

  function voiceSort(left: AppleSystemVoice, right: AppleSystemVoice): number {
    const leftSiri = isSiriLiveSpeechVoice(left);
    const rightSiri = isSiriLiveSpeechVoice(right);
    if (leftSiri !== rightSiri) return leftSiri ? -1 : 1;
    return left.locale.localeCompare(right.locale) || left.name.localeCompare(right.name, undefined, { numeric: true });
  }

  $: visibleVoices = voices.filter((voice) => {
    const key = keyFor(voice);
    const language = languageOf(voice.locale);
    const matchesFilter = filter === 'all'
      || (filter === 'favorites' && favorites.has(key))
      || (filter === 'other' && !isPrimary(voice.locale))
      || filter === language;
    const needle = query.trim().toLowerCase();
    return matchesFilter && (!needle || `${voice.name} ${voice.locale} ${voice.sample}`.toLowerCase().includes(needle));
  });
  $: favoriteVoices = voices.filter((voice) => favorites.has(keyFor(voice)));

  function persistFavorites(): void {
    localStorage.setItem(FAVORITES_KEY, JSON.stringify([...favorites]));
    favorites = new Set(favorites);
  }

  function toggleFavorite(voice: AppleSystemVoice): void {
    const key = keyFor(voice);
    if (favorites.has(key)) favorites.delete(key);
    else favorites.add(key);
    persistFavorites();
  }

  async function togglePreview(voice: AppleSystemVoice): Promise<void> {
    error = '';
    const key = keyFor(voice);
    try {
      if (playing === key) {
        await stopSpokenVoicePreview();
        playing = '';
        return;
      }
      await stopSpokenVoicePreview();
      await previewAppleVoice(voice.name, voice.locale);
      playing = key;
      window.setTimeout(() => {
        if (playing === key) playing = '';
      }, 12_000);
    } catch (cause) {
      playing = '';
      error = String(cause);
    }
  }

  function shortlistText(): string {
    const chinese = favoriteVoices.filter((voice) => languageOf(voice.locale) === 'zh');
    const english = favoriteVoices.filter((voice) => languageOf(voice.locale) === 'en');
    const other = favoriteVoices.filter((voice) => !['zh', 'en'].includes(languageOf(voice.locale)));
    const format = (items: AppleSystemVoice[]) => items.length
      ? items.map((voice) => `- ${voice.name} (${voice.locale})`).join('\n')
      : '- 暂无';
    return `Hen Local Apple 音色候选\n\n中文：\n${format(chinese)}\n\n英文：\n${format(english)}\n\n其他：\n${format(other)}`;
  }

  async function copyShortlist(): Promise<void> {
    try {
      await navigator.clipboard.writeText(shortlistText());
      copied = true;
      window.setTimeout(() => copied = false, 1800);
    } catch {
      error = '无法复制，请直接截图收藏列表。';
    }
  }

  onMount(() => {
    try {
      favorites = new Set(JSON.parse(localStorage.getItem(FAVORITES_KEY) ?? '[]'));
    } catch {
      favorites = new Set();
    }
    void listAppleVoices()
      .then((items) => { voices = [...items].sort(voiceSort); })
      .catch((cause) => { error = String(cause); })
      .finally(() => { loading = false; });
    return () => { void stopSpokenVoicePreview(); };
  });
</script>

<svelte:head><title>Apple Voice Lab · Hen Local</title></svelte:head>

<main>
  <header>
    <div>
      <p class="eyebrow">HEN LOCAL · INTERNAL TOOL</p>
      <h1>Apple 音色实验室</h1>
      <p class="intro">这里列出这台 Mac 当前开放给应用的全部系统音色。试听后点星标收藏，分别选出适合中文和英文长时间播报的声音。</p>
    </div>
    <div class="count"><strong>{voices.length}</strong><span>个可用音色</span></div>
  </header>

  <section class="instructions">
    <b>建议听这三点</b>
    <span>自然度：不像机器人</span><span>清晰度：长句仍容易听懂</span><span>耐听度：连续听十分钟不刺耳</span>
  </section>

  <nav>
    {#each filters as item}
      <button class:active={filter === item.id} on:click={() => filter = item.id}>{item.label}{item.id === 'favorites' ? ` ${favorites.size}` : ''}</button>
    {/each}
    <input bind:value={query} placeholder="搜索名称或语言代码" aria-label="搜索音色" />
  </nav>

  {#if favoriteVoices.length > 0}
    <section class="shortlist">
      <div><strong>已收藏 {favoriteVoices.length} 个</strong><span>选完后复制清单发给我，我会把正式软件只保留你选中的声音。</span></div>
      <button on:click={copyShortlist}>{copied ? '已复制' : '复制候选清单'}</button>
    </section>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}
  {#if loading}
    <p class="empty">正在读取这台 Mac 的系统音色…</p>
  {:else if visibleVoices.length === 0}
    <p class="empty">没有符合当前筛选条件的音色。</p>
  {:else}
    <section class="voice-grid">
      {#each visibleVoices as voice}
        <article class:favorite={favorites.has(keyFor(voice))}>
          <button class="star" aria-label="收藏音色" title="收藏" on:click={() => toggleFavorite(voice)}>{favorites.has(keyFor(voice)) ? '★' : '☆'}</button>
          <div class="voice-info">
            <h2>{voice.name}</h2>
            <span>{voice.locale}</span>
            {#if isSiriLiveSpeechVoice(voice)}<b class="siri-badge">SIRI · LIVE SPEECH</b>{/if}
          </div>
          <p>{voice.sample || 'Apple system voice'}</p>
          <button class="play" class:playing={playing === keyFor(voice)} on:click={() => togglePreview(voice)}>
            {playing === keyFor(voice) ? '■ 停止' : '▶ 试听'}
          </button>
        </article>
      {/each}
    </section>
  {/if}
</main>
