import { describe, expect, it } from 'vitest';
import {
  OMEGA_SNAP,
  classifyWheel,
  isSettled,
  projectRelease,
  rubberBand,
  snapBiased,
  stepSpring,
  wheelDeltaPx,
  type Spring,
} from './physics';

function simulate(s: Spring, target: number, dt: number, seconds: number): Spring {
  const steps = Math.round(seconds / dt);
  for (let i = 0; i < steps; i++) stepSpring(s, target, OMEGA_SNAP, dt);
  return s;
}

describe('stepSpring', () => {
  it('converges on the target without overshoot', () => {
    const s: Spring = { x: 0, v: 0 };
    let max = 0;
    for (let i = 0; i < 120; i++) {
      stepSpring(s, 3, OMEGA_SNAP, 1 / 60);
      max = Math.max(max, s.x);
    }
    expect(max).toBeLessThanOrEqual(3 + 1e-9);
    expect(isSettled(s, 3)).toBe(true);
  });

  it('is frame-rate independent', () => {
    const a = simulate({ x: 0, v: 5 }, 4, 1 / 60, 0.25);
    const b = simulate({ x: 0, v: 5 }, 4, 1 / 144, 0.25);
    expect(a.x).toBeCloseTo(b.x, 6);
    expect(a.v).toBeCloseTo(b.v, 6);
  });

  it('settles within half a second for a one-card move', () => {
    const s = simulate({ x: 0, v: 0 }, 1, 1 / 60, 0.5);
    expect(Math.abs(s.x - 1)).toBeLessThan(0.01);
  });
});

describe('rubberBand', () => {
  it('passes values inside the range through', () => {
    expect(rubberBand(2, 0, 4)).toBe(2);
  });
  it('resists travel past either end', () => {
    expect(rubberBand(-1, 0, 4)).toBeCloseTo(-0.35);
    expect(rubberBand(6, 0, 4)).toBeCloseTo(4.7);
  });
});

describe('projectRelease', () => {
  it('rounds to the nearest card with no velocity', () => {
    expect(projectRelease(2.4, 0, 10)).toBe(2);
  });
  it('carries momentum forward', () => {
    expect(projectRelease(2.4, 10, 10)).toBe(4);
  });
  it('clamps to the ends', () => {
    expect(projectRelease(8, 1000, 10)).toBe(9);
    expect(projectRelease(1, -1000, 10)).toBe(0);
  });
  it('handles an empty rail', () => {
    expect(projectRelease(3, 5, 0)).toBe(0);
  });
});

describe('snapBiased', () => {
  it('commits forward past the threshold', () => {
    expect(snapBiased(2.25, 1, 10)).toBe(3);
    expect(snapBiased(2.15, 1, 10)).toBe(2);
  });
  it('commits backward past the threshold', () => {
    expect(snapBiased(2.75, -1, 10)).toBe(2);
    expect(snapBiased(2.85, -1, 10)).toBe(3);
  });
  it('rounds with no direction', () => {
    expect(snapBiased(2.6, 0, 10)).toBe(3);
  });
  it('stays in range', () => {
    expect(snapBiased(-2, -1, 10)).toBe(0);
    expect(snapBiased(12, 1, 10)).toBe(9);
  });
});

describe('wheel input', () => {
  it('treats line-mode and large integer deltas as mouse notches', () => {
    expect(classifyWheel(0, 3, 1)).toBe('notch');
    expect(classifyWheel(0, 100, 0)).toBe('notch');
    expect(classifyWheel(0, -125, 0)).toBe('notch');
  });
  it('treats small or two-axis deltas as smooth', () => {
    expect(classifyWheel(0, 4.5, 0)).toBe('smooth');
    expect(classifyWheel(12, 60, 0)).toBe('smooth');
    expect(classifyWheel(0, 20, 0)).toBe('smooth');
  });
  it('uses the dominant axis and normalizes units', () => {
    expect(wheelDeltaPx(-30, 10, 0)).toBe(-30);
    expect(wheelDeltaPx(0, 2, 1)).toBe(80);
  });
});
