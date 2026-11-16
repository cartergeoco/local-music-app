/** Rail geometry. All values are CSS pixels in window coordinates. */

export const PROPORTIONS = {
  cardHeightOfWindow: 0.62,
  aspect: 3 / 4,
  gap: 24,
  radius: 20,
  minVerticalMargin: 16,
  neighborScale: 0.88,
  neighborOpacity: 0.6,
  bufferCards: 2,
} as const;

export interface RailLayout {
  /** Rail element origin in the window; card DOM is positioned relative to it. */
  left: number;
  top: number;
  width: number;
  centerX: number;
  centerY: number;
  cardW: number;
  cardH: number;
  pitch: number;
  radius: number;
}

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function computeLayout(rail: Rect, windowHeight: number): RailLayout {
  const maxH = rail.height - PROPORTIONS.minVerticalMargin * 2;
  // Even sizes keep the centered card on whole pixels so its text stays crisp.
  const even = (v: number) => Math.max(2, Math.round(v / 2) * 2);
  const cardH = even(Math.min(windowHeight * PROPORTIONS.cardHeightOfWindow, maxH));
  const cardW = even(cardH * PROPORTIONS.aspect);
  return {
    left: rail.left,
    top: rail.top,
    width: rail.width,
    centerX: Math.round(rail.left + rail.width / 2),
    centerY: Math.round(rail.top + rail.height / 2),
    cardW,
    cardH,
    pitch: cardW + PROPORTIONS.gap,
    radius: PROPORTIONS.radius,
  };
}

/** 1 when the card is centered, falling to 0 one card away. */
export function focusAmount(index: number, pos: number): number {
  const d = Math.abs(index - pos);
  return d >= 1 ? 0 : 1 - d;
}

export function cardScale(focus: number): number {
  return PROPORTIONS.neighborScale + (1 - PROPORTIONS.neighborScale) * focus;
}

export function cardOpacity(focus: number): number {
  return PROPORTIONS.neighborOpacity + (1 - PROPORTIONS.neighborOpacity) * focus;
}

/** Cards from the center to the far edge of the viewport, inclusive of partial cards. */
function halfSpan(layout: RailLayout): number {
  return (layout.width / 2 + layout.cardW / 2) / layout.pitch;
}

/** Inclusive range of card indices to render, including off-screen buffer cards. */
export function visibleRange(pos: number, count: number, layout: RailLayout): [number, number] {
  if (count <= 0) return [0, -1];
  const h = halfSpan(layout);
  const first = Math.max(0, Math.floor(pos - h) - PROPORTIONS.bufferCards);
  const last = Math.min(count - 1, Math.ceil(pos + h) + PROPORTIONS.bufferCards);
  return [first, last];
}

/** DOM nodes needed so `visibleRange` never exceeds the pool at any position. */
export function poolSize(layout: RailLayout): number {
  return 2 * Math.ceil(halfSpan(layout)) + 2 * PROPORTIONS.bufferCards + 2;
}
