import { describe, expect, it } from 'vitest';
import {
  cardOpacity,
  cardScale,
  computeLayout,
  focusAmount,
  poolSize,
  visibleRange,
} from './layout';

const rail1080 = { left: 0, top: 56, width: 1920, height: 1080 - 56 - 72 };

describe('computeLayout', () => {
  it('matches the spec proportions at 1080p', () => {
    const l = computeLayout(rail1080, 1080);
    expect(l.cardH).toBe(670);
    expect(l.cardW).toBe(502);
    expect(l.pitch).toBe(526);
    expect(l.centerX).toBe(960);
    expect(l.centerY).toBe(56 + 476);
  });

  it('shrinks cards to fit short rails', () => {
    const l = computeLayout({ left: 0, top: 56, width: 1280, height: 300 }, 720);
    expect(l.cardH).toBe(300 - 32);
  });
});

describe('focus curve', () => {
  it('peaks at the center and fades over one card', () => {
    expect(focusAmount(3, 3)).toBe(1);
    expect(focusAmount(3, 3.5)).toBe(0.5);
    expect(focusAmount(3, 4.2)).toBe(0);
  });

  it('maps focus to scale and opacity', () => {
    expect(cardScale(1)).toBe(1);
    expect(cardScale(0)).toBeCloseTo(0.88);
    expect(cardOpacity(0)).toBeCloseTo(0.6);
  });
});

describe('virtualization', () => {
  const sizes = [
    { w: 1280, h: 720 },
    { w: 1920, h: 1080 },
    { w: 3840, h: 1080 },
    { w: 2560, h: 1440 },
  ];

  it('never needs more nodes than the pool provides', () => {
    for (const { w, h } of sizes) {
      const layout = computeLayout({ left: 0, top: 56, width: w, height: h - 128 }, h);
      const pool = poolSize(layout);
      for (let pos = -0.6; pos <= 50.6; pos += 0.037) {
        const [first, last] = visibleRange(pos, 1000, layout);
        expect(last - first + 1).toBeLessThanOrEqual(pool);
      }
    }
  });

  it('keeps the pool small', () => {
    const layout = computeLayout(rail1080, 1080);
    expect(poolSize(layout)).toBeLessThanOrEqual(14);
  });

  it('clamps to the list bounds', () => {
    const layout = computeLayout(rail1080, 1080);
    expect(visibleRange(0, 3, layout)).toEqual([0, 2]);
    expect(visibleRange(0, 0, layout)).toEqual([0, -1]);
  });

  it('covers every card that overlaps the viewport', () => {
    const layout = computeLayout(rail1080, 1080);
    const pos = 20.3;
    const [first, last] = visibleRange(pos, 1000, layout);
    for (let i = 0; i < 1000; i++) {
      const center = layout.centerX + (i - pos) * layout.pitch;
      const onScreen = center + layout.cardW / 2 > 0 && center - layout.cardW / 2 < layout.width;
      if (onScreen) {
        expect(i).toBeGreaterThanOrEqual(first);
        expect(i).toBeLessThanOrEqual(last);
      }
    }
  });
});
