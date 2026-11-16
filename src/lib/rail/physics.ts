/** Rail motion math. Positions are in card units (0 = first card). */

export interface Spring {
  x: number;
  v: number;
}

/** Angular frequency used when settling onto a card. */
export const OMEGA_SNAP = 15;
/** Angular frequency used while following continuous input (trackpad). */
export const OMEGA_FOLLOW = 38;
/** Seconds of velocity carried into the landing point after a drag release. */
export const MOMENTUM = 0.16;
export const MAX_VELOCITY = 40;

/**
 * Advances a critically damped spring toward `target` using the closed-form
 * solution, so motion is identical at any frame rate.
 */
export function stepSpring(s: Spring, target: number, omega: number, dt: number): void {
  const delta = s.x - target;
  const decay = Math.exp(-omega * dt);
  const k = (s.v + omega * delta) * dt;
  s.x = target + (delta + k) * decay;
  s.v = (s.v - omega * k) * decay;
}

export function isSettled(s: Spring, target: number): boolean {
  return Math.abs(s.x - target) < 1e-4 && Math.abs(s.v) < 1e-3;
}

export function clamp(value: number, min: number, max: number): number {
  return value < min ? min : value > max ? max : value;
}

/** Lets a value travel past [min, max] with increasing resistance. */
export function rubberBand(value: number, min: number, max: number, k = 0.35): number {
  if (value < min) return min - (min - value) * k;
  if (value > max) return max + (value - max) * k;
  return value;
}

/** Card a drag release should land on, given position and velocity (cards/s). */
export function projectRelease(x: number, v: number, count: number): number {
  if (count <= 0) return 0;
  return Math.round(clamp(x + clamp(v, -MAX_VELOCITY, MAX_VELOCITY) * MOMENTUM, 0, count - 1));
}

/**
 * Snaps a free-scroll position to a card, favoring the direction of travel:
 * once the user has moved `threshold` of a card past one, it commits to the next.
 */
export function snapBiased(x: number, direction: number, count: number, threshold = 0.2): number {
  if (count <= 0) return 0;
  const c = clamp(x, 0, count - 1);
  const base = Math.floor(c);
  const f = c - base;
  let snapped: number;
  if (direction > 0) snapped = f > threshold ? base + 1 : base;
  else if (direction < 0) snapped = f < 1 - threshold ? base : base + 1;
  else snapped = Math.round(c);
  return clamp(snapped, 0, count - 1);
}

export type WheelKind = 'notch' | 'smooth';

/**
 * Notched mouse wheels report large whole-line steps on one axis; precision
 * touchpads report small, often fractional deltas, frequently on both axes.
 */
export function classifyWheel(deltaX: number, deltaY: number, deltaMode: number): WheelKind {
  if (deltaMode !== 0) return 'notch';
  if (deltaX === 0 && Math.abs(deltaY) >= 50 && Number.isInteger(deltaY)) return 'notch';
  return 'smooth';
}

const LINE_PX = 40;
const PAGE_PX = 800;

/** Dominant-axis wheel delta in CSS pixels. */
export function wheelDeltaPx(deltaX: number, deltaY: number, deltaMode: number): number {
  const d = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
  return deltaMode === 1 ? d * LINE_PX : deltaMode === 2 ? d * PAGE_PX : d;
}
