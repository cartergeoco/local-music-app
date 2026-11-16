import type { RailLayout } from './layout';

/**
 * Per-frame rail state shared with the background renderer. Deliberately a
 * plain mutable object, not reactive state, so reading it costs nothing.
 */
export const railState = {
  /** Current position in card units, may sit slightly outside the list while rubber-banding. */
  pos: 0,
  /** Whether the rail is mid-motion this frame. */
  moving: false,
  layout: null as RailLayout | null,
  /** Visible list: view index to artist position in the store. */
  view: new Uint32Array(0) as Uint32Array,
  /** Bumped whenever anything above changes, so idle consumers can skip work. */
  version: 0,
};
