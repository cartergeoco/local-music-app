<script lang="ts">
  interface Props {
    query: string;
    focused: number;
    total: number;
    input?: HTMLInputElement;
  }

  let { query = $bindable(), focused, total, input = $bindable() }: Props = $props();
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
  <div class="counter">
    {#if total > 0}
      {String(focused + 1).padStart(String(total).length, '0')} / {total}
    {:else}
      0 / 0
    {/if}
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
  .counter {
    justify-self: end;
    font-variant-numeric: tabular-nums;
    color: rgb(255 255 255 / 0.6);
  }
</style>
