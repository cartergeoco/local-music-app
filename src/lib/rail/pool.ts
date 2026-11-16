import type { ArtistStore } from '../store';
import { cardOpacity, cardScale, focusAmount, visibleRange, type RailLayout } from './layout';

/**
 * Fixed set of card DOM nodes recycled as the rail scrolls. Card `i` always
 * lives in slot `i % size`, so scrolling never creates, removes, or reorders
 * nodes; it only rewrites transforms, and content when a slot changes hands.
 */

interface Slot {
  el: HTMLDivElement;
  still: HTMLDivElement;
  badge: HTMLDivElement;
  name: HTMLDivElement;
  index: number;
  artist: number;
  visible: boolean;
  frame: number;
  x: number;
  scale: number;
  opacity: number;
  stillOpacity: number;
}

function div(className: string, parent: HTMLElement): HTMLDivElement {
  const el = document.createElement('div');
  el.className = className;
  parent.appendChild(el);
  return el;
}

export class CardPool {
  private slots: Slot[] = [];
  private frame = 0;

  constructor(
    private readonly container: HTMLElement,
    private readonly store: ArtistStore,
  ) {}

  get size(): number {
    return this.slots.length;
  }

  resize(size: number): void {
    while (this.slots.length > size) this.slots.pop()!.el.remove();
    while (this.slots.length < size) this.slots.push(this.createSlot());
    // Slot assignment and geometry depend on size, so every slot must be redone.
    for (const s of this.slots) {
      s.index = s.artist = -1;
      s.x = s.scale = NaN;
    }
  }

  /** Forces content refresh, e.g. after the visible list changes. */
  invalidate(): void {
    for (const s of this.slots) s.index = s.artist = -1;
  }

  render(pos: number, view: Uint32Array, layout: RailLayout): void {
    const size = this.slots.length;
    if (size === 0) return;
    const frame = ++this.frame;
    const [first, last] = visibleRange(pos, view.length, layout);
    const y = layout.centerY - layout.top - layout.cardH / 2;
    const originX = layout.centerX - layout.left - layout.cardW / 2;

    for (let i = first; i <= last; i++) {
      const slot = this.slots[i % size];
      slot.frame = frame;
      if (slot.index !== i) {
        slot.index = i;
        slot.el.dataset.index = String(i);
      }
      const artist = view[i];
      if (slot.artist !== artist) this.fill(slot, artist);

      const f = focusAmount(i, pos);
      const x = originX + (i - pos) * layout.pitch;
      const scale = cardScale(f);
      if (x !== slot.x || scale !== slot.scale) {
        slot.x = x;
        slot.scale = scale;
        slot.el.style.transform = `translate3d(${x}px,${y}px,0) scale(${scale})`;
      }
      const opacity = cardOpacity(f);
      if (opacity !== slot.opacity) {
        slot.opacity = opacity;
        slot.el.style.opacity = String(opacity);
      }
      const stillOpacity = 1 - f;
      if (stillOpacity !== slot.stillOpacity) {
        slot.stillOpacity = stillOpacity;
        slot.still.style.opacity = String(stillOpacity);
      }
      if (!slot.visible) {
        slot.visible = true;
        slot.el.style.visibility = 'visible';
      }
    }

    for (const slot of this.slots) {
      if (slot.frame !== frame && slot.visible) {
        slot.visible = false;
        slot.el.style.visibility = 'hidden';
      }
    }
  }

  destroy(): void {
    for (const s of this.slots) s.el.remove();
    this.slots = [];
  }

  private createSlot(): Slot {
    const el = div('card', this.container);
    el.style.visibility = 'hidden';
    const still = div('card-still', el);
    div('card-shade', el);
    const meta = div('card-meta', el);
    const badge = div('card-badge', meta);
    const name = div('card-name', meta);
    return {
      el,
      still,
      badge,
      name,
      index: -1,
      artist: -1,
      visible: false,
      frame: 0,
      x: NaN,
      scale: NaN,
      opacity: NaN,
      stillOpacity: NaN,
    };
  }

  private fill(slot: Slot, artist: number): void {
    const a = this.store.artists[artist];
    const s = this.store;
    slot.artist = artist;
    slot.name.textContent = a.name;
    slot.badge.textContent = a.monogram;
    slot.still.style.background = `linear-gradient(160deg, ${s.color(artist, 3)} 0%, ${s.color(artist, 1)} 55%, ${s.color(artist, 0)} 100%)`;
    slot.badge.style.background = `linear-gradient(135deg, ${s.color(artist, 4)}, ${s.color(artist, 2)})`;
  }
}
