<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getLaunchOptions,
    getLibrary,
    importPaths,
    onFileDrop,
    onLibraryChanged,
    onProgress,
    pickImportFolders,
    showWindow,
    type Progress,
  } from './lib/api';
  import { ArtistStore } from './lib/store';
  import Background from './lib/background/Background.svelte';
  import Rail from './lib/rail/Rail.svelte';
  import TopBar from './lib/chrome/TopBar.svelte';
  import PlayerBar from './lib/chrome/PlayerBar.svelte';
  import PerfHUD from './lib/perf/PerfHUD.svelte';
  import ArtistDetail from './lib/detail/ArtistDetail.svelte';

  const SOAK_INTERVAL_MS = 300;
  /** Enrichment can report several changes a second; coalesce them. */
  const REFRESH_DEBOUNCE_MS = 250;

  let store = $state.raw<ArtistStore | null>(null);
  let root = $state('');
  let busy = $state(false);
  let progress = $state<Progress | null>(null);
  let libraryVersion = $state(0);
  let loadError = $state<string | null>(null);
  let query = $state('');
  let focused = $state(-1);
  let detailFor = $state<number | null>(null);
  let dropping = $state(false);
  let hud = $state(import.meta.env.DEV);
  let soak = $state(false);
  let rail = $state<ReturnType<typeof Rail> | null>(null);
  let input = $state<HTMLInputElement | undefined>();

  const view = $derived(store ? store.filter(query) : new Uint32Array(0));
  const detailArtist = $derived(
    store && detailFor !== null ? (store.artists[store.positionOf(detailFor)] ?? null) : null,
  );

  let refreshSeq = 0;
  let refreshTimer = 0;
  let alive = true;

  async function refresh(): Promise<void> {
    const seq = ++refreshSeq;
    const lib = await getLibrary();
    if (!alive || seq !== refreshSeq) return;
    store = new ArtistStore(lib.artists);
    root = lib.root;
    busy = lib.busy;
    if (!busy) progress = null;
    libraryVersion++;
  }

  function scheduleRefresh(): void {
    if (refreshTimer) return;
    refreshTimer = window.setTimeout(() => {
      refreshTimer = 0;
      refresh().catch((e) => console.error(e));
    }, REFRESH_DEBOUNCE_MS);
  }

  async function importFolders(): Promise<void> {
    const paths = await pickImportFolders();
    if (paths.length) await importPaths(paths);
  }

  function openDetail(index: number): void {
    if (!store || index < 0 || index >= view.length) return;
    detailFor = store.artists[view[index]].id;
  }

  onMount(() => {
    const unlisteners = [
      onLibraryChanged(scheduleRefresh),
      onProgress((p) => {
        progress = p;
        busy = true;
      }),
      onFileDrop(
        (paths) => void importPaths(paths),
        (over) => (dropping = over),
      ),
    ];

    Promise.all([refresh(), getLaunchOptions()])
      .then(([, options]) => {
        if (!alive) return;
        if (options.soakSeconds > 0) {
          hud = soak = true;
          setTimeout(() => (soak = false), options.soakSeconds * 1000);
        }
      })
      .catch((err) => {
        loadError = String(err);
      })
      .finally(() => {
        // Show the window only after the first real frame to avoid a blank flash.
        requestAnimationFrame(() => requestAnimationFrame(() => void showWindow()));
      });

    return () => {
      alive = false;
      clearTimeout(refreshTimer);
      for (const u of unlisteners) void u.then((off) => off());
    };
  });

  $effect(() => {
    if (!soak) return;
    let n = 0;
    const timer = setInterval(() => {
      const r = rail;
      if (!r) return;
      n++;
      if (n % 12 === 0) query = query ? '' : 'aeiou'[n % 5];
      else r.goTo(Math.floor(Math.random() * r.count()));
    }, SOAK_INTERVAL_MS);
    return () => clearInterval(timer);
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'F3') {
      e.preventDefault();
      hud = !hud;
      return;
    }
    if (e.key === 'F4' && import.meta.env.DEV) {
      e.preventDefault();
      soak = !soak;
      return;
    }

    if (detailFor !== null) {
      if (e.key === 'Escape') {
        e.preventDefault();
        detailFor = null;
      }
      return;
    }

    if (e.target === input) {
      if (e.key === 'Escape') {
        query = '';
        input?.blur();
      } else if (e.key === 'Enter') {
        input?.blur();
      }
      return;
    }

    if (e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case 'ArrowRight':
        rail?.stepBy(1);
        break;
      case 'ArrowLeft':
        rail?.stepBy(-1);
        break;
      case 'PageDown':
        rail?.stepBy(5);
        break;
      case 'PageUp':
        rail?.stepBy(-5);
        break;
      case 'Home':
        rail?.goTo(0);
        break;
      case 'End':
        rail?.goTo(Number.MAX_SAFE_INTEGER);
        break;
      case 'Enter':
        openDetail(focused);
        break;
      case 'Escape':
        query = '';
        break;
      default:
        // Type-to-filter: focusing the field during keydown routes this keystroke into it.
        if (e.key.length === 1 || e.key === 'Backspace') {
          input?.focus();
          return;
        }
        return;
    }
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if store}
  <Background {store} />
{/if}

<main class="app">
  <TopBar bind:query bind:input {focused} total={view.length} {progress} onImport={importFolders} />
  {#if store}
    <Rail bind:this={rail} {store} {view} onFocus={(i) => (focused = i)} onActivate={openDetail} />
  {:else}
    <div class="status">{loadError ?? ''}</div>
  {/if}
  <PlayerBar />
</main>

{#if store && store.size === 0}
  <div class="empty">
    <h2>{busy ? 'Building your library…' : 'Your library is empty'}</h2>
    {#if !busy}
      <button class="primary" onclick={importFolders}>Import a folder</button>
      <p>or drop folders onto the window. Files are copied into<br />{root}</p>
    {/if}
  </div>
{:else if store && view.length === 0}
  <div class="no-match">No artists match “{query}”</div>
{/if}

{#if dropping}
  <div class="drop">Drop to copy into your library</div>
{/if}

{#if detailArtist}
  <ArtistDetail
    artistId={detailArtist.id}
    monogram={detailArtist.monogram}
    version={libraryVersion}
    onClose={() => (detailFor = null)}
  />
{/if}

{#if hud}
  <PerfHUD {soak} />
{/if}

<style>
  .app {
    position: relative;
    height: 100%;
    display: grid;
    grid-template-rows: var(--topbar-h) minmax(0, 1fr) var(--player-h);
  }
  .status {
    display: grid;
    place-items: center;
    color: rgb(255 255 255 / 0.6);
  }
  .empty {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
    color: rgb(255 255 255 / 0.6);
  }
  .empty h2 {
    margin: 0;
    font-size: 24px;
    font-weight: 650;
    color: #f2f2f4;
  }
  .empty p {
    margin: 0;
    line-height: 1.5;
  }
  .primary {
    height: 40px;
    padding: 0 20px;
    border-radius: 10px;
    border: none;
    background: #f2f2f4;
    color: #0a0a0c;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .no-match {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    z-index: 2;
    color: rgb(255 255 255 / 0.6);
    font-size: 18px;
  }
  .drop {
    position: fixed;
    inset: 16px;
    z-index: 20;
    display: grid;
    place-items: center;
    border: 2px dashed rgb(255 255 255 / 0.35);
    border-radius: 24px;
    background: rgb(10 10 12 / 0.7);
    font-size: 20px;
    font-weight: 600;
    pointer-events: none;
  }
</style>
