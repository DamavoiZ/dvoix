// State of the right-click menu, shared by every component that opens one.
// Not named contextMenu.svelte.ts: on case-insensitive file systems (Windows,
// macOS) the import "./contextMenu.svelte" would resolve to ContextMenu.svelte.
import type { Component } from "svelte";

export interface MenuItem {
  label: string;
  icon?: Component;
  danger?: boolean;
  action: () => void;
}

export type MenuEntry = MenuItem | "separator";

class ContextMenuState {
  open = $state<{ x: number; y: number; items: MenuEntry[] } | null>(null);

  show(event: MouseEvent, items: MenuEntry[]) {
    event.preventDefault();
    event.stopPropagation();
    this.open = { x: event.clientX, y: event.clientY, items };
  }

  close() {
    this.open = null;
  }
}

export const contextMenu = new ContextMenuState();
