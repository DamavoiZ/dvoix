// The app draws its own title bar (the native one is turned off in the Tauri
// config on Windows and Linux), so dragging, double-click to maximize and the
// window buttons are wired up here. Components only use `dragRegion` and
// `appWindow`, never the Tauri window API directly.
import { getCurrentWindow } from "@tauri-apps/api/window";


/** Clicks on these start no drag, so buttons and tabs inside a title bar keep working. */
const INTERACTIVE = "button, a, input, textarea, select, [role='tab'], [draggable='true'], [contenteditable]";

class AppWindow {
  /** True when the window has no native frame and the app must show its own buttons. */
  custom = $state(false);
  maximized = $state(false);

  async init() {
    try {
      this.custom = !(await getCurrentWindow().isDecorated());
      this.maximized = await getCurrentWindow().isMaximized();
      await getCurrentWindow().onResized(async () => {
        this.maximized = await getCurrentWindow().isMaximized();
      });
    } catch {
      // Outside Tauri (e.g. plain `vite` in a browser) there is no window to control.
    }
  }

  minimize = () => void getCurrentWindow().minimize();
  toggleMaximize = () => void getCurrentWindow().toggleMaximize();
  close = () => void getCurrentWindow().close();
}

export const appWindow = new AppWindow();

/**
 * Svelte action: empty space inside `node` moves the window, double click
 * maximizes. Unlike `data-tauri-drag-region` this also covers child elements
 * that are not interactive (labels, spacers), so plugins rendered inside a
 * title bar area don't need to know about it.
 */
export function dragRegion(node: HTMLElement) {
  // The drag starts on the first move, not on mousedown: on Linux (GTK) a
  // window drag grabs the pointer, so the second click of a double click
  // would never reach the page and maximizing would not work.
  let pending = false;
  const move = (e: MouseEvent) => {
    if (!pending) return;
    pending = false;
    if (e.buttons === 1) void getCurrentWindow().startDragging();
  };
  const up = () => (pending = false);
  const down = (e: MouseEvent) => {
    if (e.button !== 0 || !appWindow.custom) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest(INTERACTIVE)) return;
    e.preventDefault();
    if (e.detail === 2) {
      pending = false;
      appWindow.toggleMaximize();
    } else pending = true;
  };
  node.addEventListener("mousedown", down);
  node.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
  return {
    destroy: () => {
      node.removeEventListener("mousedown", down);
      node.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    },
  };
}
