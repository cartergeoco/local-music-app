<script lang="ts">
  import type { Progress } from '../api';

  interface Props {
    query: string;
    focused: number;
    total: number;
    progress: Progress | null;
    onImport: () => void;
    input?: HTMLInputElement;
  }

  let { query = $bindable(), focused, total, progress, onImport, input = $bindable() }: Props = $props();

  const PHASES: Record<Progress['phase'], string> = {
    copying: 'Importing',
    scanning: 'Scanning',
    artwork: 'Artwork',
  };
</script>

<header class="topbar">
  <div class="wordmark">Local Music</div>
  <input
    bind:this={input}
    bind:value={query}
    class="search"
    type="search"
    placeholder="Type to filter artists"
    spellcheck="false"
    autocomplete="off"
  />
  <div class="right">
    {#if progress}
      <div class="progress" aria-live="polite">
        {PHASES[progress.phase]}
        {#if progress.total > 0}{Math.min(progress.done + 1, progress.total)} / {progress.total}{/if}
      </div>
    {/if}
    <button class="import" onclick={onImport}>Import</button>
    <div class="counter">
      {#if total > 0}
        {String(focused + 1).padStart(String(total).length, '0')} / {total}
      {:else}
        0 / 0
      {/if}
    </div>
  </div>
</header>

<style>
  .topbar {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: 1fr minmax(240px, 420px) 1fr;
    align-items: center;
    gap: 16px;
    padding: 0 24px;
    background: rgb(10 10 12 / 0.55);
    border-bottom: 1px solid rgb(255 255 255 / 0.06);
  }
  .wordmark {
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .search {
    height: 34px;
    padding: 0 14px;
    border-radius: 10px;
    border: 1px solid rgb(255 255 255 / 0.1);
    background: rgb(255 255 255 / 0.06);
    color: inherit;
    font: inherit;
    outline: none;
  }
  .search:focus {
    border-color: rgb(255 255 255 / 0.35);
  }
  .right {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .progress,
  .counter {
    font-variant-numeric: tabular-nums;
    color: rgb(255 255 255 / 0.6);
  }
  .import {
    height: 32px;
    padding: 0 14px;
    border-radius: 8px;
    border: 1px solid rgb(255 255 255 / 0.12);
    background: rgb(255 255 255 / 0.06);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .import:hover {
    background: rgb(255 255 255 / 0.1);
  }
</style>
