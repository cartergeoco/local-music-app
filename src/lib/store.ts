import { assetUrl, type ArtistSummary } from './api';

/** Immutable artist data prepared once for fast filtering and rendering. */
export class ArtistStore {
  readonly artists: ArtistSummary[];
  /** 15 floats (5 RGB colors, 0..1) per artist, indexed by artist position. */
  readonly palettes: Float32Array;
  /** Loadable image URLs, indexed by artist position. */
  readonly coverUrls: (string | null)[];
  readonly headshotUrls: (string | null)[];
  private readonly searchNames: string[];
  private readonly all: Uint32Array;
  private readonly positions = new Map<number, number>();

  constructor(artists: ArtistSummary[], url: (path: string | null) => string | null = assetUrl) {
    this.artists = artists;
    this.palettes = new Float32Array(artists.length * 15);
    artists.forEach((a, i) => {
      for (let c = 0; c < 15; c++) this.palettes[i * 15 + c] = (a.palette[c] ?? 0) / 255;
      this.positions.set(a.id, i);
    });
    this.coverUrls = artists.map((a) => url(a.coverPath));
    this.headshotUrls = artists.map((a) => url(a.headshotPath));
    this.searchNames = artists.map((a) => a.name.toLowerCase());
    this.all = Uint32Array.from(artists, (_, i) => i);
  }

  get size(): number {
    return this.artists.length;
  }

  /** Position of the artist with this id, or -1. */
  positionOf(id: number): number {
    return this.positions.get(id) ?? -1;
  }

  /** Artist positions whose name contains `query` (case-insensitive). */
  filter(query: string): Uint32Array {
    const q = query.trim().toLowerCase();
    if (!q) return this.all;
    const out: number[] = [];
    for (let i = 0; i < this.searchNames.length; i++) {
      if (this.searchNames[i].includes(q)) out.push(i);
    }
    return Uint32Array.from(out);
  }

  /** CSS color for one palette stop. */
  color(artist: number, stop: number): string {
    const p = this.artists[artist].palette;
    return `rgb(${p[stop * 3]} ${p[stop * 3 + 1]} ${p[stop * 3 + 2]})`;
  }
}
