<script lang="ts">
  import { onMount } from 'svelte';
  import type { ArtistStore } from '../store';
  import { RailController } from './controller';
  import { computeLayout } from './layout';

  interface Props {
    store: ArtistStore;
    view: Uint32Array;
    onFocus: (index: number) => void;
    /** The focused card was clicked. */
    onActivate: (index: number) => void;
  }

  let { store, view, onFocus, onActivate }: Props = $props();
  let el: HTMLDivElement;
  let controller = $state.raw<RailController | null>(null);

  export function stepBy(cards: number): void {
    controller?.stepBy(cards);
  }

  export function goTo(index: number): void {
    controller?.goTo(index);
  }

  export function count(): number {
    return controller?.count ?? 0;
  }

  onMount(() => {
    const c = new RailController(
      el,
      store,
      (i) => onFocus(i),
      (i) => onActivate(i),
    );
    const observer = new ResizeObserver(() => {
      c.setLayout(computeLayout(el.getBoundingClientRect(), window.innerHeight));
    });
    observer.observe(el);
    controller = c;
    return () => {
      observer.disconnect();
      c.destroy();
      controller = null;
    };
  });

  $effect(() => {
    controller?.setData(store, view);
  });
</script>

<div bind:this={el} class="rail" role="listbox" aria-label="Artists" tabindex="-1"></div>

<style>
  .rail {
    position: relative;
    overflow: hidden;
    touch-action: none;
    cursor: grab;
    outline: none;
  }
  .rail:active {
    cursor: grabbing;
  }
</style>
