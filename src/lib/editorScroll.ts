// How long the editor waits after the last scroll step before the views that
// follow the viewport catch up, in milliseconds. The live preview decorates
// the lines a scroll step brings into view right away, but the full rebuild
// of its decorations (see `filetypes/markdown/livePreview.ts`) waits for this
// pause: rebuilding the whole rendered window on every frame of a wheel
// scroll made scrolling stutter. Typing, clicking and every other update
// still decorate right away.
export const SCROLL_SETTLE_MS = 120;
