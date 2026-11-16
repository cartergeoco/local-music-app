<script lang="ts">
  import { onMount } from 'svelte';
  import { getArtists, getLaunchOptions, showWindow } from './lib/api';
  import { ArtistStore } from './lib/store';
  import Background from './lib/background/Background.svelte';
  import Rail from './lib/rail/Rail.svelte';
  import TopBar from './lib/chrome/TopBar.svelte';
  import PlayerBar from './lib/chrome/PlayerBar.svelte';
  import PerfHUD from './lib/perf/PerfHUD.svelte';

  const SOAK_INTERVAL_MS = 300;

  let store = $state.raw<ArtistStore | null>(null);
  let loadError = $state<string | null>(null);
  let query = $state('');
  let focused = $state(-1);
  let hud = $state(import.meta.env.DEV);
  let soak = $state(false);
  let rail = $state<ReturnType<typeof Rail> | null>(null);
  let input = $state<HTMLInputElement | undefined>();

  const view = $derived(store ? store.filter(query) : new Uint32Array(0));

  onMount(() => {
    let alive = true;
    Promise.all([getArtists(), getLaunchOptions()])
      .then(([artists, options]) => {
        if (!alive) return;
        store = new ArtistStore(artists);
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
  <TopBar bind:query bind:input {focused} total={view.length} />
  {#if store}
    <Rail bind:this={rail} {store} {view} onFocus={(i) => (focused = i)} />
  {:else}
    <div class="status">{loadError ?? ''}</div>
  {/if}
  <PlayerBar />
</main>

{#if store && view.length === 0}
  <div class="empty">No artists match “{query}”</div>
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
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
    z-index: 2;
    color: rgb(255 255 255 / 0.6);
    font-size: 18px;
  }
</style>
