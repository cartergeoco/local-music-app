import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Ticker } from './ticker';

let frames: FrameRequestCallback[] = [];
let clock = 0;

function runFrame(advanceMs = 16) {
  clock += advanceMs;
  const pending = frames;
  frames = [];
  for (const cb of pending) cb(clock);
}

beforeEach(() => {
  frames = [];
  clock = 1000;
  vi.useFakeTimers();
  vi.stubGlobal('window', globalThis);
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => frames.push(cb));
  vi.stubGlobal('cancelAnimationFrame', () => {
    frames = [];
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('Ticker', () => {
  it('runs entries in order and stops when none need more frames', () => {
    const t = new Ticker();
    const calls: string[] = [];
    let remaining = 2;
    t.add(() => (calls.push('b'), false), 10);
    t.add(() => (calls.push('a'), --remaining > 0), 0);
    t.wake();
    runFrame();
    runFrame();
    expect(calls).toEqual(['a', 'b', 'a', 'b']);
    expect(t.running).toBe(false);
    expect(frames).toHaveLength(0);
  });

  it('waits on a timer when an entry asks for a delayed frame', () => {
    const t = new Ticker();
    const fn = vi.fn(() => 30);
    t.add(fn, 0);
    t.wake();
    runFrame();
    expect(frames).toHaveLength(0);
    expect(t.running).toBe(true);
    vi.advanceTimersByTime(29);
    expect(frames).toHaveLength(0);
    vi.advanceTimersByTime(1);
    expect(frames).toHaveLength(1);
  });

  it('wake cuts a pending delay short', () => {
    const t = new Ticker();
    t.add(() => 1000, 0);
    t.wake();
    runFrame();
    t.wake();
    expect(frames).toHaveLength(1);
    vi.advanceTimersByTime(1000);
    expect(frames).toHaveLength(1);
  });

  it('reports pacing only for back-to-back frames', () => {
    const t = new Ticker();
    const seen: boolean[] = [];
    let n = 0;
    t.add(() => (++n < 3 ? true : 50), 0);
    t.observe((_ms, _now, continuous) => seen.push(continuous));
    t.wake();
    runFrame();
    runFrame();
    runFrame();
    vi.advanceTimersByTime(50);
    runFrame();
    expect(seen).toEqual([true, true, false]);
  });

  it('stops entirely when the last entry is removed', () => {
    const t = new Ticker();
    const handle = t.add(() => 100, 0);
    t.wake();
    runFrame();
    handle.remove();
    expect(t.running).toBe(false);
  });
});
