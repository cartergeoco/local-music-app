import { invoke } from '@tauri-apps/api/core';

export interface ArtistCard {
  id: number;
  name: string;
  monogram: string;
  /** Five RGB colors, flattened, ordered dark to light. */
  palette: number[];
}

export interface PerfStats {
  hostMb: number;
  webviewMb: number;
  workingSetMb: number;
  cpuPercent: number;
  processCount: number;
}

export interface LaunchOptions {
  soakSeconds: number;
}

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function getArtists(): Promise<ArtistCard[]> {
  return inTauri ? invoke<ArtistCard[]>('get_artists') : Promise.resolve(browserFallback(500));
}

export function getLaunchOptions(): Promise<LaunchOptions> {
  return inTauri ? invoke<LaunchOptions>('launch_options') : Promise.resolve({ soakSeconds: 0 });
}

export function getPerfStats(): Promise<PerfStats | null> {
  return inTauri ? invoke<PerfStats>('perf_stats') : Promise.resolve(null);
}

export async function showWindow(): Promise<void> {
  if (!inTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().show();
}

/** Lets the UI run in a plain browser during development. */
function browserFallback(count: number): ArtistCard[] {
  return Array.from({ length: count }, (_, id) => {
    const hue = (id * 47) % 360;
    const palette: number[] = [];
    for (let i = 0; i < 5; i++) {
      const l = 0.1 + i * 0.17;
      const [r, g, b] = hslToRgb((hue + i * 18) % 360, 0.6, l);
      palette.push(r, g, b);
    }
    return { id, name: `Artist ${id + 1}`, monogram: `A${(id + 1) % 10}`, palette };
  });
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    return Math.round((l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))) * 255);
  };
  return [f(0), f(8), f(4)];
}
