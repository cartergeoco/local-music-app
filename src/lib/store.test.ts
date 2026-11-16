import { describe, expect, it } from 'vitest';
import { ArtistStore } from './store';

const palette = Array.from({ length: 15 }, (_, i) => i * 17);
const store = new ArtistStore([
  { id: 0, name: 'Neon Society', monogram: 'NS', palette },
  { id: 1, name: 'The Echos', monogram: 'EC', palette },
  { id: 2, name: 'Velvet Union', monogram: 'VU', palette },
]);

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
});
