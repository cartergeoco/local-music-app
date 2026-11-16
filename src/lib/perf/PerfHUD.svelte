<script lang="ts">
  import { onMount } from 'svelte';
  import { getPerfStats, type PerfStats } from '../api';
  import { ticker } from '../ticker';

  const UI_INTERVAL_MS = 500;
  const SYSTEM_INTERVAL_MS = 2000;

  let { soak }: { soak: boolean } = $props();

  let fps = $state(0);
  let avgMs = $state(0);
  let worstMs = $state(0);
  let dropped = $state(0);
  let heapMb = $state<number | null>(null);
  let domNodes = $state(0);
  let loopRunning = $state(false);
  let system = $state.raw<PerfStats | null>(null);

  interface ChromeMemory {
    memory?: { usedJSHeapSize: number };
  }

  onMount(() => {
    let frames = 0;
    let timed = 0;
    let sum = 0;
    let worst = 0;
    let droppedTotal = 0;
    // The shortest frame seen approximates the display's refresh interval.
    let vsyncMs = Infinity;
    let windowStart = performance.now();

    const unobserve = ticker.observe((ms, _now, continuous) => {
      frames++;
      // Frame pacing only means something when frames were requested back to back.
      if (!continuous) return;
      timed++;
      sum += ms;
      if (ms > worst) worst = ms;
      if (ms < vsyncMs && ms > 4) vsyncMs = ms;
      if (ms > vsyncMs * 1.5) droppedTotal++;
    });

    const ui = setInterval(() => {
      const now = performance.now();
      fps = (frames * 1000) / (now - windowStart);
      avgMs = timed ? sum / timed : 0;
      worstMs = worst;
      dropped = droppedTotal;
      frames = timed = sum = worst = 0;
      windowStart = now;
      const mem = (performance as ChromeMemory).memory;
      heapMb = mem ? mem.usedJSHeapSize / 1048576 : null;
      domNodes = document.getElementsByTagName('*').length;
      loopRunning = ticker.running;
    }, UI_INTERVAL_MS);

    let inFlight = false;
    const poll = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        system = await getPerfStats();
      } catch (err) {
        console.error(err);
      } finally {
        inFlight = false;
      }
    };
    poll();
    const sys = setInterval(poll, SYSTEM_INTERVAL_MS);

    return () => {
      unobserve();
      clearInterval(ui);
      clearInterval(sys);
    };
  });

  const f0 = (n: number) => n.toFixed(0);
  const f1 = (n: number) => n.toFixed(1);
</script>

<aside class="hud" aria-label="Performance">
  <div class="row"><span>loop</span><b>{loopRunning ? 'running' : 'idle'}{soak ? ' · soak' : ''}</b></div>
  <div class="row"><span>fps</span><b>{loopRunning ? f0(fps) : '-'}</b></div>
  <div class="row"><span>frame avg / worst</span><b>{f1(avgMs)} / {f1(worstMs)} ms</b></div>
  <div class="row"><span>dropped frames</span><b>{dropped}</b></div>
  <div class="row"><span>js heap</span><b>{heapMb === null ? 'n/a' : `${f1(heapMb)} MB`}</b></div>
  <div class="row"><span>dom nodes</span><b>{domNodes}</b></div>
  {#if system}
    <div class="sep"></div>
    <div class="row"><span>private, host</span><b>{f1(system.hostMb)} MB</b></div>
    <div class="row"><span>private, webview</span><b>{f1(system.webviewMb)} MB</b></div>
    <div class="row"><span>private, total</span><b>{f1(system.hostMb + system.webviewMb)} MB</b></div>
    <div class="row"><span>working set (sum)</span><b>{f1(system.workingSetMb)} MB</b></div>
    <div class="row"><span>cpu</span><b>{f1(system.cpuPercent)}% · {system.processCount} procs</b></div>
  {/if}
  <div class="hint">F3 hide{import.meta.env.DEV ? ' · F4 soak test' : ''}</div>
</aside>

<style>
  .hud {
    position: fixed;
    top: 68px;
    right: 12px;
    z-index: 10;
    width: 260px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgb(0 0 0 / 0.72);
    border: 1px solid rgb(255 255 255 / 0.08);
    font: 11px/1.6 ui-monospace, 'Cascadia Mono', Consolas, monospace;
    color: rgb(255 255 255 / 0.85);
    pointer-events: none;
    contain: content;
  }
  .row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .row span {
    color: rgb(255 255 255 / 0.5);
  }
  .row b {
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .sep {
    height: 1px;
    margin: 6px 0;
    background: rgb(255 255 255 / 0.1);
  }
  .hint {
    margin-top: 6px;
    color: rgb(255 255 255 / 0.35);
  }
</style>
