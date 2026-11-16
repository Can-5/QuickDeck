<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { cycleTheme } from '$lib/theme';
  import { groupResults, type HistoryEntry, type RankableItem } from '$lib/rank';

  interface AppEntry { id: string; name: string; comment: string; keywords: string[]; icon: string; kind: string; source: string }
  interface FileEntry { id: string; name: string; path: string; kind: string }
  interface Website { id: string; name: string; url: string; kind: string }
  interface Command { id: string; name: string; cmd: string; kind: string }
  interface Snapshot { favorites: string[]; websites: Website[]; commands: Command[]; history: HistoryEntry[] }
  type RowItem = RankableItem & { kind: string; sub?: string };
  interface Row { item: RowItem; index: number }

  let query = $state('');
  let apps = $state<AppEntry[]>([]);
  let files = $state<FileEntry[]>([]);
  let filesFor = $state('');
  let filesLoading = $state(false);
  let snap = $state<Snapshot>({ favorites: [], websites: [], commands: [], history: [] });
  let activeIndex = $state(0);
  let toast = $state('');
  let modal = $state<'website' | 'command' | null>(null);
  let modalName = $state('');
  let modalValue = $state('');
  let modalError = $state('');
  let modalSaving = $state(false);
  let inputEl = $state<HTMLInputElement>();
  let modalNameEl = $state<HTMLInputElement>();
  let resultsEl = $state<HTMLDivElement>();
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const cmdMode = $derived(query.trim().startsWith('>'));
  const effectiveQ = $derived(cmdMode ? query.trim().slice(1).trim() : query.trim());
  const favorites = $derived(new Set(snap.favorites));

  const view = $derived.by(() => {
    const now = Math.floor(Date.now() / 1000);
    const q = effectiveQ;
    const appItems: RowItem[] = apps.map((a) => ({ id: a.id, name: a.name, keywords: [...a.keywords, a.comment], kind: 'app', sub: a.comment }));
    const fileItems: RowItem[] = (filesFor === q ? files : []).map((f) => ({ id: f.id, name: f.name, keywords: [f.path], kind: 'file', sub: f.path }));
    const webItems: RowItem[] = snap.websites.map((w) => ({ id: w.id, name: w.name, keywords: [w.url], kind: 'website', sub: w.url }));
    const cmdItems: RowItem[] = snap.commands.map((c) => ({ id: c.id, name: c.name, keywords: [c.cmd], kind: 'command', sub: c.cmd }));

    let sections: { title: string; items: RowItem[] }[] = [];
    if (!q && !cmdMode) {
      const resolvable = [...appItems, ...webItems, ...cmdItems].filter((i) => favorites.has(i.id));
      const recents: RowItem[] = snap.history.slice(0, 6).map((h) => ({ id: h.id, name: h.name, keywords: [], kind: h.kind, sub: kindLabel(h.kind) }));
      if (resolvable.length) sections.push({ title: 'Favorites', items: resolvable });
      if (recents.length) sections.push({ title: 'Recent', items: recents });
    } else if (cmdMode) {
      const g = groupResults([{ title: 'Commands', candidates: cmdItems }], q, snap.history, favorites, now);
      sections = g.sections;
    } else {
      const g = groupResults(
        [
          { title: 'Apps', candidates: appItems },
          { title: 'Files', candidates: fileItems },
          { title: 'Websites', candidates: webItems },
          { title: 'Commands', candidates: cmdItems },
        ],
        q,
        snap.history,
        favorites,
        now
      );
      sections = g.sections;
    }
    let idx = 0;
    const flat: RowItem[] = [];
    const secs = sections.map((s) => ({
      title: s.title,
      items: s.items.map((item): Row => {
        const index = idx++;
        flat.push(item);
        return { item, index };
      }),
    }));
    return { sections: secs, flat, count: idx };
  });

  $effect(() => {
    const q = effectiveQ;
    if (q.length < 2 || cmdMode) {
      files = [];
      filesFor = '';
      filesLoading = false;
      return;
    }
    filesLoading = true;
    const t = setTimeout(async () => {
      try {
        const res = await invoke<FileEntry[]>('search_files_cmd', { query: q, limit: 8 });
        files = res;
        filesFor = q;
      } catch {
        files = [];
        filesFor = q;
      }
      filesLoading = false;
    }, 220);
    return () => clearTimeout(t);
  });

  $effect(() => {
    if (activeIndex >= view.count) activeIndex = 0;
  });

  const kindLabel = (kind: string) =>
    ({ app: 'app', file: 'file', web: 'website', website: 'website', cmd: 'command', command: 'command', win: 'app', mac: 'app' } as Record<string, string>)[kind] ?? kind;

  const GLYPH: Record<string, string> = {
    app: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7.5" height="7.5" rx="1.5"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="1.5"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="1.5"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.5"/></svg>',
    file: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/></svg>',
    website: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18"/></svg>',
    web: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18"/></svg>',
    command: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 17 6-6-6-6M12 19h8"/></svg>',
    cmd: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 17 6-6-6-6M12 19h8"/></svg>',
    win: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7.5" height="7.5" rx="1.5"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="1.5"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="1.5"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.5"/></svg>',
    mac: '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7.5" height="7.5" rx="1.5"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="1.5"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="1.5"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.5"/></svg>',
  };

  async function refreshSnapshot() {
    try { snap = await invoke<Snapshot>('snapshot'); } catch { /* first run, store may be empty */ }
  }

  function showToast(msg: string) {
    toast = msg;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ''), 3000);
  }

  const kindOf = (id: string) => id.split(':')[0] ?? 'app';

  async function launch(item: RowItem) {
    try {
      await invoke('launch_item', { id: item.id, name: item.name });
    } catch (e) {
      showToast(`Couldn't open: ${e}`);
      return;
    }
    try {
      await invoke('record_launch', { id: item.id, name: item.name, kind: kindOf(item.id) });
    } catch { /* history is best-effort */ }
    query = '';
    files = [];
    filesFor = '';
    activeIndex = 0;
    refreshSnapshot();
    await invoke('hide_window');
  }

  async function toggleFav(item: RowItem) {
    try { await invoke('toggle_favorite', { id: item.id }); } catch (e) { showToast(String(e)); }
    refreshSnapshot();
  }

  async function removeItem(item: RowItem) {
    try {
      if (item.id.startsWith('web:')) await invoke('remove_website', { id: item.id });
      else if (item.id.startsWith('cmd:')) await invoke('remove_command', { id: item.id });
    } catch (e) { showToast(String(e)); }
    refreshSnapshot();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (view.count === 0) return;
      const dir = e.key === 'ArrowDown' ? 1 : -1;
      activeIndex = (activeIndex + dir + view.count) % view.count;
      scrollActiveIntoView();
    } else if (e.key === 'Enter') {
      const item = view.flat[activeIndex];
      if (!item) return;
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) toggleFav(item);
      else launch(item);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      if (modal) closeModal();
      else invoke('hide_window');
    }
  }

  function scrollActiveIntoView() {
    requestAnimationFrame(() => {
      resultsEl?.querySelector('.row.active')?.scrollIntoView({ block: 'nearest' });
    });
  }

  async function openModal(kind: 'website' | 'command') {
    modal = kind;
    modalName = '';
    modalValue = '';
    modalError = '';
    await tick();
    modalNameEl?.focus();
  }
  function closeModal() { modal = null; modalSaving = false; inputEl?.focus(); }

  async function submitModal() {
    if (!modal || modalSaving) return;
    modalSaving = true;
    modalError = '';
    try {
      if (modal === 'website') await invoke('add_website', { name: modalName.trim(), url: modalValue.trim() });
      else await invoke('add_command', { name: modalName.trim(), cmd: modalValue.trim() });
      closeModal();
      refreshSnapshot();
    } catch (e) {
      modalError = String(e);
      modalSaving = false;
    }
  }

  function modalKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') { e.preventDefault(); submitModal(); }
    else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closeModal(); }
  }

  onMount(() => {
    (async () => {
      try { apps = await invoke<AppEntry[]>('discover_apps_cmd'); } catch { /* shown as 0 apps */ }
      refreshSnapshot();
    })();
    inputEl?.focus();
    const onFocus = () => {
      query = '';
      activeIndex = 0;
      files = [];
      filesFor = '';
      refreshSnapshot();
      inputEl?.focus();
    };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  });
</script>

<div class="launcher">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="searchRow" data-tauri-drag-region>
    <svg class="searchGlyph" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      bind:this={inputEl}
      bind:value={query}
      onkeydown={onKeydown}
      class="searchInput"
      placeholder="Search apps, files, websites — or > for commands…"
      autofocus
      spellcheck="false"
      autocomplete="off"
      aria-label="Search"
    />
    {#if filesLoading}<div class="spinner" aria-label="Searching files"></div>{/if}
  </div>

  <div class="results" bind:this={resultsEl} role="listbox" aria-label="Results">
    {#each view.sections as section (section.title)}
      <div class="sectionTitle">{section.title}</div>
      {#each section.items as row (row.item.id + ':' + row.index)}
        {@const item = row.item}
        <button
          class="row"
          class:active={row.index === activeIndex}
          role="option"
          aria-selected={row.index === activeIndex}
          onmouseenter={() => (activeIndex = row.index)}
          onclick={() => launch(item)}
        >
          <span class="tile kind-{kindOf(item.id)}">{@html GLYPH[kindOf(item.id)] ?? GLYPH.app}</span>
          <span class="meta">
            <span class="name">{item.name}</span>
            {#if item.sub}<span class="sub">{item.sub}</span>{/if}
          </span>
          {#if item.id.startsWith('web:') || item.id.startsWith('cmd:')}
            <span
              class="removeBtn"
              role="button"
              tabindex="-1"
              title="Remove"
              aria-label="Remove {item.name}"
              onclick={(e) => { e.stopPropagation(); removeItem(item); }}
              onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); removeItem(item); } }}
            >×</span>
          {/if}
          <span
            class="favBtn"
            class:on={favorites.has(item.id)}
            role="button"
            tabindex="-1"
            title="{favorites.has(item.id) ? 'Unfavorite' : 'Favorite'}"
            aria-label="Toggle favorite"
            onclick={(e) => { e.stopPropagation(); toggleFav(item); }}
            onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); toggleFav(item); } }}
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill={favorites.has(item.id) ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 2.6 14.9 8.7l6.7.8-5 4.7 1.3 6.6L12 18.4 6.1 20.8l1.3-6.6-5-4.7 6.7-.8z"/></svg>
          </span>
        </button>
      {/each}
    {/each}
    {#if view.count === 0 && effectiveQ}
      <div class="empty">
        <p>No results for “{effectiveQ}”</p>
        {#if cmdMode}<p class="dim">Add a shortcut for “{effectiveQ}” with <strong>+ Cmd</strong> below.</p>{/if}
      </div>
    {:else if view.count === 0}
      <div class="empty">
        <p class="dim">Type to search apps, files and websites.</p>
        <p class="dim small">Use <span class="kbd">&gt;</span> to run your own commands, e.g. <span class="kbd">&gt; git pull</span></p>
      </div>
    {/if}
  </div>

  <footer class="statusbar">
    <span class="hints">
      <span class="kbd">↑↓</span> navigate
      <span class="kbd">↵</span> open
      <span class="kbd">Ctrl↵</span> favorite
      <span class="kbd">Esc</span> close
    </span>
    <span class="spacer"></span>
    <span class="appsCount dim">{apps.length} apps</span>
    <button class="btn btn-ghost btn-sm" onclick={() => openModal('website')}>+ Web</button>
    <button class="btn btn-ghost btn-sm" onclick={() => openModal('command')}>+ Cmd</button>
    <button class="icon-btn" onclick={cycleTheme} title="Cycle theme" aria-label="Cycle theme">
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4.2"/><path d="M12 2v2.4M12 19.6V22M2 12h2.4M19.6 12H22M4.2 4.2l1.7 1.7M18.1 18.1l1.7 1.7M4.2 19.8l1.7-1.7M18.1 5.9l1.7-1.7"/></svg>
    </button>
  </footer>

  {#if modal}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="overlay" role="presentation" onclick={closeModal}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="dialog card" role="dialog" aria-modal="true" aria-label={modal === 'website' ? 'Add website' : 'Add command'} onclick={(e) => e.stopPropagation()} onkeydown={modalKeydown} tabindex="-1">
        <h2>{modal === 'website' ? 'Add website shortcut' : 'Add command shortcut'}</h2>
        <label>
          <span>{modal === 'website' ? 'Name' : 'Alias'}</span>
          <input class="input" bind:this={modalNameEl} bind:value={modalName} placeholder={modal === 'website' ? 'GitHub' : 'update'} spellcheck="false" />
        </label>
        <label>
          <span>{modal === 'website' ? 'URL (https://…)' : 'Shell command'}</span>
          <input class="input mono" bind:value={modalValue} placeholder={modal === 'website' ? 'https://github.com' : 'git pull --rebase'} spellcheck="false" />
        </label>
        {#if modalError}<p class="errorText">{modalError}</p>{/if}
        <div class="dialogActions">
          <button class="btn btn-ghost" onclick={closeModal}>Cancel</button>
          <button class="btn" onclick={submitModal} disabled={modalSaving}>{modalSaving ? 'Saving…' : 'Save'}</button>
        </div>
      </div>
    </div>
  {/if}

  {#if toast}<div class="toast" role="status">{toast}</div>{/if}
</div>

<style>
  .launcher {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    overflow: hidden;
    user-select: none;
  }
  .searchRow {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.55rem 0.8rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elevated);
  }
  .searchGlyph { color: var(--fg-dim); flex-shrink: 0; }
  .searchInput {
    flex: 1;
    font-size: 1.06rem;
    font-family: inherit;
    color: var(--fg);
    background: transparent;
    border: none;
    outline: none;
    padding: 0.3rem 0;
    user-select: text;
  }
  .results {
    flex: 1;
    overflow-y: auto;
    padding: 0.35rem 0.5rem;
  }
  .sectionTitle {
    font-size: 0.7rem;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-dim);
    padding: 0.5rem 0.5rem 0.2rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.38rem 0.5rem;
    cursor: pointer;
    color: var(--fg);
  }
  .row.active { background: var(--accent-soft); outline: 1px solid color-mix(in srgb, var(--accent) 35%, transparent); }
  .tile {
    flex-shrink: 0;
    width: 34px;
    height: 34px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-inset);
    color: var(--fg-dim);
  }
  .row.active .tile { background: var(--bg-elevated); color: var(--accent); }
  .meta { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .name { font-weight: 550; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 0.75rem; color: var(--fg-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .favBtn, .removeBtn {
    flex-shrink: 0;
    background: transparent;
    border: none;
    color: var(--fg-dim);
    border-radius: 6px;
    padding: 0.3rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .favBtn:hover, .removeBtn:hover { background: var(--bg-hover); color: var(--fg); }
  .favBtn.on { color: var(--accent); }
  .removeBtn { font-size: 1.05rem; line-height: 1; }
  /* Show fav/remove on hover of row, always show when favorited */
  .row .favBtn, .row .removeBtn { visibility: hidden; }
  .row:hover .favBtn, .row:hover .removeBtn, .row.active .favBtn, .row.active .removeBtn, .row .favBtn.on { visibility: visible; }
  .empty { text-align: center; padding: 2.2rem 1rem; color: var(--fg-dim); }
  .empty p { margin: 0.25rem 0; }
  .empty .small { font-size: 0.8rem; }
  .statusbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.7rem;
    border-top: 1px solid var(--border);
    background: var(--bg-elevated);
    font-size: 0.78rem;
  }
  .hints { display: flex; align-items: center; gap: 0.4rem; color: var(--fg-dim); white-space: nowrap; }
  .spacer { flex: 1; }
  .appsCount { font-size: 0.78rem; }
  .overlay {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog {
    width: min(420px, calc(100vw - 2rem));
    padding: 1rem 1.1rem 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }
  .dialog h2 { font-size: 0.95rem; margin: 0; }
  .dialog label { display: flex; flex-direction: column; gap: 0.3rem; font-size: 0.78rem; color: var(--fg-dim); }
  .dialogActions { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 0.2rem; }
  .errorText { color: var(--danger); font-size: 0.8rem; margin: 0; }
</style>
