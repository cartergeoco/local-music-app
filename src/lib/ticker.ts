/**
 * Single requestAnimationFrame loop shared by everything that animates.
 *
 * Every registered callback runs on every frame that happens, in `order`, and
 * returns what it needs next: `true` for the very next frame, a number of
 * milliseconds to wait before its next frame, or `false` for nothing. When
 * nobody needs anything the loop stops completely, so an idle app costs nothing.
 */

export type TickResult = boolean | number;
export type TickFn = (dt: number, now: number) => TickResult;
/** `continuous` is false when the frame followed an idle stop or a deliberate delay. */
export type FrameObserver = (frameMs: number, now: number, continuous: boolean) => void;

export interface TickHandle {
  wake(): void;
  remove(): void;
}

interface Entry {
  fn: TickFn;
  order: number;
}

const MAX_DT = 0.1;

export class Ticker {
  private entries: Entry[] = [];
  private observers = new Set<FrameObserver>();
  private raf = 0;
  private timer = 0;
  private last = 0;
  private continuous = false;

  get running(): boolean {
    return this.raf !== 0 || this.timer !== 0;
  }

  add(fn: TickFn, order: number): TickHandle {
    const entry: Entry = { fn, order };
    this.entries.push(entry);
    this.entries.sort((a, b) => a.order - b.order);
    return {
      wake: () => this.wake(),
      remove: () => {
        const i = this.entries.indexOf(entry);
        if (i >= 0) this.entries.splice(i, 1);
        if (this.entries.length === 0) this.stop();
      },
    };
  }

  /** Observers see frame timings but never keep the loop alive. */
  observe(fn: FrameObserver): () => void {
    this.observers.add(fn);
    return () => this.observers.delete(fn);
  }

  wake(): void {
    if (this.raf !== 0) return;
    if (this.timer !== 0) {
      clearTimeout(this.timer);
      this.timer = 0;
    }
    this.raf = requestAnimationFrame(this.frame);
  }

  private stop(): void {
    if (this.raf !== 0) cancelAnimationFrame(this.raf);
    if (this.timer !== 0) clearTimeout(this.timer);
    this.raf = this.timer = 0;
    this.last = 0;
    this.continuous = false;
  }

  private frame = (now: number): void => {
    this.raf = 0;
    const frameMs = this.last === 0 ? 0 : now - this.last;
    const dt = this.last === 0 ? 1 / 60 : Math.min(frameMs / 1000, MAX_DT);
    this.last = now;

    let soonest = Infinity;
    for (let i = 0; i < this.entries.length; i++) {
      const r = this.entries[i].fn(dt, now);
      if (r === true) soonest = 0;
      else if (typeof r === 'number' && r < soonest) soonest = Math.max(0, r);
    }
    if (frameMs > 0 && this.observers.size > 0) {
      for (const o of this.observers) o(frameMs, now, this.continuous);
    }

    if (soonest === 0) {
      this.continuous = true;
      this.raf = requestAnimationFrame(this.frame);
    } else if (soonest !== Infinity) {
      this.continuous = false;
      this.timer = window.setTimeout(this.onTimer, soonest);
    } else {
      this.last = 0;
      this.continuous = false;
    }
  };

  private onTimer = (): void => {
    this.timer = 0;
    this.raf = requestAnimationFrame(this.frame);
  };
}

export const ticker = new Ticker();

export const TICK_ORDER = { rail: 0, background: 10 } as const;
