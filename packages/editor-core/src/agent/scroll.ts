/**
 * Streaming must not make the panel jump (DESIGN.md "Agent panel rules"): the panel follows new content only while
 * the reader is at the bottom. Scrolled up, the reading position stays and a "Latest" pill offers the way down.
 * Pure: the view reports sizes and applies the `scrollTo` it is given.
 */
export interface ScrollMetrics {
  height: number;
  client: number;
}

/** How close to the bottom still counts as at the bottom, in pixels. */
const NEAR_BOTTOM = 24;

export function createScrollFollow() {
  const state = { following: true, latest: false };
  // Where the view was last told to go. Its scroll event can arrive after more content has landed, and must not
  // read as the reader leaving the bottom.
  let commanded: number | null = null;
  const bottom = (m: ScrollMetrics) => Math.max(0, m.height - m.client);
  const go = (to: number) => {
    commanded = to;
    return to;
  };
  return {
    state,
    /** The reader scrolled (or the view moved to where it was told). */
    scrolled(m: ScrollMetrics & { top: number }) {
      if (commanded !== null && Math.abs(m.top - commanded) <= 1) {
        commanded = null;
        return;
      }
      commanded = null;
      state.following = bottom(m) - m.top <= NEAR_BOTTOM;
      if (state.following) state.latest = false;
    },
    /** The content changed size. */
    content(m: ScrollMetrics): { scrollTo: number | null } {
      if (state.following) return { scrollTo: go(bottom(m)) };
      state.latest = true;
      return { scrollTo: null };
    },
    /** The Latest pill. */
    jump(m: ScrollMetrics): { scrollTo: number } {
      state.following = true;
      state.latest = false;
      return { scrollTo: go(bottom(m)) };
    },
  };
}
export type ScrollFollow = ReturnType<typeof createScrollFollow>;
