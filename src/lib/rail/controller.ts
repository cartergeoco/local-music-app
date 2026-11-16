import { ticker, TICK_ORDER, type TickHandle } from '../ticker';
import type { ArtistStore } from '../store';
import { poolSize, type RailLayout } from './layout';
import {
  OMEGA_FOLLOW,
  OMEGA_SNAP,
  MAX_VELOCITY,
  classifyWheel,
  clamp,
  isSettled,
  projectRelease,
  rubberBand,
  snapBiased,
  stepSpring,
  wheelDeltaPx,
  type Spring,
} from './physics';
import { CardPool } from './pool';
import { railState } from './state';

const SNAP_IDLE_MS = 120;
const CLICK_SLOP_PX = 6;
/** A drag that pauses this long before release lands without momentum. */
const RELEASE_STALE_MS = 80;

interface Drag {
  pointerId: number;
  /** Card under the pointer at press time; pointer capture retargets later events. */
  pressedIndex: number;
  startX: number;
  startPos: number;
  lastX: number;
  lastT: number;
  velocity: number;
  moved: boolean;
}

/**
 * Owns rail motion: input handling, spring physics, and driving the card pool.
 * The DOM is only touched from the shared ticker, at most once per frame.
 */
export class RailController {
  private readonly pool: CardPool;
  private readonly tick: TickHandle;
  private readonly spring: Spring = { x: 0, v: 0 };
  private target = 0;
  private omega = OMEGA_SNAP;
  private view: Uint32Array = new Uint32Array(0);
  private layout: RailLayout | null = null;
  private freeRaw: number | null = null;
  private freeDir = 0;
  private snapTimer = 0;
  private drag: Drag | null = null;
  private focused = -1;

  constructor(
    private readonly el: HTMLElement,
    store: ArtistStore,
    private readonly onFocus: (index: number) => void,
  ) {
    this.pool = new CardPool(el, store);
    this.tick = ticker.add(this.step, TICK_ORDER.rail);
    el.addEventListener('wheel', this.onWheel, { passive: true });
    el.addEventListener('pointerdown', this.onPointerDown);
    el.addEventListener('pointermove', this.onPointerMove);
    el.addEventListener('pointerup', this.onPointerUp);
    el.addEventListener('pointercancel', this.onPointerCancel);
  }

  get count(): number {
    return this.view.length;
  }

  setLayout(layout: RailLayout): void {
    this.layout = layout;
    railState.layout = layout;
    this.el.style.setProperty('--card-w', `${layout.cardW}px`);
    this.el.style.setProperty('--card-h', `${layout.cardH}px`);
    this.el.style.setProperty('--card-radius', `${layout.radius}px`);
    this.pool.resize(poolSize(layout));
    this.renderNow();
  }

  /** Replaces the visible list, keeping the focused artist in place when it survives. */
  setView(view: Uint32Array): void {
    const prevArtist = this.focused >= 0 ? this.view[this.focused] : -1;
    this.view = view;
    railState.view = view;
    let index = prevArtist >= 0 ? view.indexOf(prevArtist) : -1;
    if (index < 0) index = 0;
    this.cancelGestures();
    this.spring.x = this.target = view.length ? index : 0;
    this.spring.v = 0;
    this.pool.invalidate();
    this.focused = -1;
    this.renderNow();
  }

  /** Moves focus by whole cards from wherever the rail is heading. */
  stepBy(cards: number): void {
    this.goTo(Math.round(this.target) + cards);
  }

  goTo(index: number): void {
    if (!this.view.length) return;
    this.cancelGestures();
    this.target = clamp(Math.round(index), 0, this.view.length - 1);
    this.omega = OMEGA_SNAP;
    this.tick.wake();
  }

  destroy(): void {
    this.cancelGestures();
    this.tick.remove();
    this.el.removeEventListener('wheel', this.onWheel);
    this.el.removeEventListener('pointerdown', this.onPointerDown);
    this.el.removeEventListener('pointermove', this.onPointerMove);
    this.el.removeEventListener('pointerup', this.onPointerUp);
    this.el.removeEventListener('pointercancel', this.onPointerCancel);
    this.pool.destroy();
    railState.moving = false;
    railState.layout = null;
    railState.view = new Uint32Array(0);
    railState.version++;
  }

  private step = (dt: number): boolean => {
    if (!this.layout) return false;
    const settled = isSettled(this.spring, this.target);
    if (settled && !railState.moving) return false;
    if (settled) {
      this.spring.x = this.target;
      this.spring.v = 0;
    } else {
      stepSpring(this.spring, this.target, this.omega, dt);
    }
    railState.moving = !settled;
    this.publish();
    return !settled;
  };

  private renderNow(): void {
    if (!this.layout) return;
    railState.moving = false;
    this.publish();
  }

  private publish(): void {
    const pos = this.spring.x;
    railState.pos = pos;
    railState.version++;
    this.pool.render(pos, this.view, this.layout!);
    const focused = this.view.length ? clamp(Math.round(pos), 0, this.view.length - 1) : -1;
    if (focused !== this.focused) {
      this.focused = focused;
      this.onFocus(focused);
    }
  }

  private cancelGestures(): void {
    if (this.snapTimer) clearTimeout(this.snapTimer);
    this.snapTimer = 0;
    this.freeRaw = null;
    if (this.drag && this.el.hasPointerCapture(this.drag.pointerId)) {
      this.el.releasePointerCapture(this.drag.pointerId);
    }
    this.drag = null;
  }

  private onWheel = (e: WheelEvent): void => {
    if (!this.layout || !this.view.length || this.drag) return;
    const last = this.view.length - 1;

    if (classifyWheel(e.deltaX, e.deltaY, e.deltaMode) === 'notch') {
      const dir = Math.sign(e.deltaY || e.deltaX);
      this.goTo(Math.round(this.target) + dir);
      return;
    }

    const delta = wheelDeltaPx(e.deltaX, e.deltaY, e.deltaMode);
    if (delta === 0) return;
    this.freeRaw = (this.freeRaw ?? this.target) + delta / this.layout.pitch;
    this.freeRaw = clamp(this.freeRaw, -1, last + 1);
    this.freeDir = Math.sign(delta);
    this.target = rubberBand(this.freeRaw, 0, last);
    this.omega = OMEGA_FOLLOW;
    this.tick.wake();

    if (this.snapTimer) clearTimeout(this.snapTimer);
    this.snapTimer = window.setTimeout(this.snapAfterWheel, SNAP_IDLE_MS);
  };

  private snapAfterWheel = (): void => {
    this.snapTimer = 0;
    if (this.freeRaw === null) return;
    this.target = snapBiased(this.freeRaw, this.freeDir, this.view.length);
    this.freeRaw = null;
    this.omega = OMEGA_SNAP;
    this.tick.wake();
  };

  private onPointerDown = (e: PointerEvent): void => {
    if (e.button !== 0 || !this.layout || !this.view.length) return;
    this.cancelGestures();
    const card = (e.target as Element | null)?.closest<HTMLElement>('.card');
    this.el.setPointerCapture(e.pointerId);
    this.drag = {
      pointerId: e.pointerId,
      pressedIndex: card?.dataset.index ? Number(card.dataset.index) : -1,
      startX: e.clientX,
      startPos: this.spring.x,
      lastX: e.clientX,
      lastT: e.timeStamp,
      velocity: 0,
      moved: false,
    };
  };

  private onPointerMove = (e: PointerEvent): void => {
    const d = this.drag;
    if (!d || e.pointerId !== d.pointerId || !this.layout) return;
    const pitch = this.layout.pitch;
    if (!d.moved && Math.abs(e.clientX - d.startX) > CLICK_SLOP_PX) d.moved = true;
    if (!d.moved) return;

    const dt = (e.timeStamp - d.lastT) / 1000;
    if (dt > 0) {
      const instant = -(e.clientX - d.lastX) / pitch / dt;
      d.velocity = clamp(d.velocity * 0.6 + instant * 0.4, -MAX_VELOCITY, MAX_VELOCITY);
    }
    d.lastX = e.clientX;
    d.lastT = e.timeStamp;

    const raw = d.startPos - (e.clientX - d.startX) / pitch;
    this.target = rubberBand(raw, 0, this.view.length - 1);
    this.spring.x = this.target;
    this.spring.v = 0;
    railState.moving = true;
    this.tick.wake();
  };

  private onPointerUp = (e: PointerEvent): void => {
    const d = this.drag;
    if (!d || e.pointerId !== d.pointerId) return;
    this.drag = null;
    if (this.el.hasPointerCapture(e.pointerId)) this.el.releasePointerCapture(e.pointerId);

    if (!d.moved) {
      if (d.pressedIndex >= 0) this.goTo(d.pressedIndex);
      return;
    }
    const velocity = e.timeStamp - d.lastT > RELEASE_STALE_MS ? 0 : d.velocity;
    this.target = projectRelease(this.spring.x, velocity, this.view.length);
    this.spring.v = velocity;
    this.omega = OMEGA_SNAP;
    this.tick.wake();
  };

  private onPointerCancel = (e: PointerEvent): void => {
    if (!this.drag || e.pointerId !== this.drag.pointerId) return;
    this.drag = null;
    this.goTo(this.spring.x);
  };
}
