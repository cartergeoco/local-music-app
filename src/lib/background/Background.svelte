<script lang="ts">
  import { onMount } from 'svelte';
  import type { ArtistStore } from '../store';
  import { ticker, TICK_ORDER } from '../ticker';
  import { railState } from '../rail/state';
  import { BackgroundRenderer } from './renderer';

  /** While the rail is still, the slow flow looks the same at 30 fps. */
  const IDLE_FRAME_MS = 1000 / 30;
  const VSYNC_SLACK_MS = 4;

  let { store }: { store: ArtistStore } = $props();
  let canvas: HTMLCanvasElement;
  let redraw: (() => void) | null = null;
  let current: BackgroundRenderer | null = null;

  $effect(() => {
    const s = store;
    if (!current) return;
    current.setStore(s);
    redraw?.();
  });

  onMount(() => {
    let renderer: BackgroundRenderer;
    try {
      renderer = new BackgroundRenderer(canvas, store);
    } catch (err) {
      console.error(err);
      return;
    }
    current = renderer;

    const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
    const shouldAnimate = () =>
      document.visibilityState === 'visible' && document.hasFocus() && !reducedMotion.matches;

    let animate = shouldAnimate();
    let time = 0;
    let lastRender = -Infinity;
    let lastVersion = -1;

    const handle = ticker.add((dt, now) => {
      if (!renderer.ready) return false;
      const changed = railState.version !== lastVersion;
      if (animate) {
        time += dt;
        // rAF timestamps mark the frame start, slightly before the timer fired.
        const due = IDLE_FRAME_MS - VSYNC_SLACK_MS;
        if (railState.moving || changed || now - lastRender >= due) {
          renderer.render(time);
          lastRender = now;
          lastVersion = railState.version;
        }
        return railState.moving || due - (now - lastRender);
      }
      // Paused: draw only when the rail moves so the focus mask stays aligned.
      if (changed) {
        renderer.render(time);
        lastVersion = railState.version;
      }
      return false;
    }, TICK_ORDER.background);

    const onStateChange = () => {
      animate = shouldAnimate();
      if (animate) handle.wake();
    };
    const onResize = () => {
      renderer.resize(window.innerWidth, window.innerHeight, window.devicePixelRatio);
      lastVersion = -1;
      handle.wake();
    };
    const onContextLost = (e: Event) => {
      e.preventDefault();
      renderer.lose();
    };
    const onContextRestored = () => {
      renderer.init();
      lastVersion = -1;
      handle.wake();
    };

    redraw = () => {
      lastVersion = -1;
      handle.wake();
    };
    onResize();
    window.addEventListener('resize', onResize);
    window.addEventListener('focus', onStateChange);
    window.addEventListener('blur', onStateChange);
    document.addEventListener('visibilitychange', onStateChange);
    reducedMotion.addEventListener('change', onStateChange);
    canvas.addEventListener('webglcontextlost', onContextLost);
    canvas.addEventListener('webglcontextrestored', onContextRestored);

    return () => {
      window.removeEventListener('resize', onResize);
      window.removeEventListener('focus', onStateChange);
      window.removeEventListener('blur', onStateChange);
      document.removeEventListener('visibilitychange', onStateChange);
      reducedMotion.removeEventListener('change', onStateChange);
      canvas.removeEventListener('webglcontextlost', onContextLost);
      canvas.removeEventListener('webglcontextrestored', onContextRestored);
      redraw = null;
      current = null;
      handle.remove();
      renderer.destroy();
    };
  });
</script>

<canvas bind:this={canvas} class="background" aria-hidden="true"></canvas>

<style>
  .background {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    z-index: 0;
  }
</style>
