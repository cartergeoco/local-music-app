import { convertFileSrc, invoke } from '@tauri-apps/api/core';

export interface ArtistSummary {
  id: number;
  name: string;
  monogram: string;
  /** Five RGB colors, flattened, ordered dark to light. */
  palette: number[];
  coverPath: string | null;
  headshotPath: string | null;
  releaseCount: number;
  trackCount: number;
  mbid: string | null;
}

export interface Library {
  root: string;
  /** An import or scan is running. */
  busy: boolean;
  artists: ArtistSummary[];
}

export interface TrackInfo {
  id: number;
  title: string;
  artist: string;
  trackNo: number | null;
  discNo: number | null;
  durationMs: number;
  path: string;
}

export interface ReleaseInfo {
  id: number;
  title: string;
  year: number | null;
  mbid: string | null;
  mbType: string | null;
  mbDate: string | null;
  coverThumbPath: string | null;
  tracks: TrackInfo[];
}

export interface ArtistDetail {
  id: number;
  name: string;
  mbid: string | null;
  mbStatus: 'pending' | 'matched' | 'unmatched' | 'error';
  mbVerified: boolean;
  disambiguation: string | null;
  headshotPath: string | null;
  releases: ReleaseInfo[];
}

export interface Progress {
  phase: 'copying' | 'scanning' | 'artwork';
  done: number;
  total: number;
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

type Unlisten = () => void;

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function getLibrary(): Promise<Library> {
  return inTauri
    ? invoke<Library>('get_library')
    : Promise.resolve({ root: '', busy: false, artists: browserFallback(500) });
}

export function getArtist(id: number): Promise<ArtistDetail | null> {
  return inTauri ? invoke<ArtistDetail | null>('get_artist', { id }) : Promise.resolve(null);
}

export function importPaths(paths: string[]): Promise<void> {
  return inTauri ? invoke('import_paths', { paths }) : Promise.resolve();
}

/** URL the webview can load for a file in the app's image cache. */
export function assetUrl(path: string | null): string | null {
  return path && inTauri ? convertFileSrc(path) : null;
}

export async function pickImportFolders(): Promise<string[]> {
  if (!inTauri) return [];
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ directory: true, multiple: true, title: 'Import music folders' });
  return picked ?? [];
}

async function listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
  if (!inTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  return listen<T>(event, (e) => cb(e.payload));
}

export const onLibraryChanged = (cb: () => void) => listen<null>('library-changed', cb);
export const onProgress = (cb: (p: Progress) => void) => listen<Progress>('library-progress', cb);

/** Files or folders dropped onto the window; `hovering` tracks drag-over state. */
export async function onFileDrop(cb: (paths: string[]) => void, hovering: (over: boolean) => void): Promise<Unlisten> {
  if (!inTauri) return () => {};
  const { getCurrentWebview } = await import('@tauri-apps/api/webview');
  return getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    if (p.type === 'enter' || p.type === 'over') hovering(true);
    else if (p.type === 'leave') hovering(false);
    else if (p.type === 'drop') {
      hovering(false);
      if (p.paths.length) cb(p.paths);
    }
  });
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
function browserFallback(count: number): ArtistSummary[] {
  return Array.from({ length: count }, (_, id) => {
    const hue = (id * 47) % 360;
    const palette: number[] = [];
    for (let i = 0; i < 5; i++) {
      const l = 0.1 + i * 0.17;
      const [r, g, b] = hslToRgb((hue + i * 18) % 360, 0.6, l);
      palette.push(r, g, b);
    }
    return {
      id,
      name: `Artist ${id + 1}`,
      monogram: `A${(id + 1) % 10}`,
      palette,
      coverPath: null,
      headshotPath: null,
      releaseCount: 0,
      trackCount: 0,
      mbid: null,
    };
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
