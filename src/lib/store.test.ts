import { describe, expect, it } from 'vitest';
import type { ArtistSummary } from './api';
import { ArtistStore } from './store';

const palette = Array.from({ length: 15 }, (_, i) => i * 17);
const artist = (id: number, name: string, coverPath: string | null = null): ArtistSummary => ({
  id,
  name,
  monogram: name.slice(0, 2).toUpperCase(),
  palette,
  coverPath,
  headshotPath: null,
  releaseCount: 1,
  trackCount: 1,
  mbid: null,
});
const store = new ArtistStore(
  [artist(10, 'Neon Society', 'C:\\covers\\10.jpg'), artist(4, 'The Echos'), artist(7, 'Velvet Union')],
  (p) => (p ? `asset://${p}` : null),
);

describe('ArtistStore', () => {
  it('returns everyone for an empty query', () => {
    expect([...store.filter('  ')]).toEqual([0, 1, 2]);
  });

  it('matches case-insensitively anywhere in the name', () => {
    expect([...store.filter('ECH')]).toEqual([1]);
    expect([...store.filter('e')]).toEqual([0, 1, 2]);
    expect([...store.filter('zzz')]).toEqual([]);
  });

  it('normalizes palettes to 0..1 floats', () => {
    expect(store.palettes.length).toBe(45);
    expect(store.palettes[15 + 14]).toBeCloseTo(238 / 255);
  });

  it('formats palette stops as CSS colors', () => {
    expect(store.color(0, 1)).toBe('rgb(51 68 85)');
  });

  it('finds positions by artist id', () => {
    expect(store.positionOf(7)).toBe(2);
    expect(store.positionOf(99)).toBe(-1);
  });

  it('resolves image URLs once', () => {
    expect(store.coverUrls).toEqual(['asset://C:\\covers\\10.jpg', null, null]);
  });
});
