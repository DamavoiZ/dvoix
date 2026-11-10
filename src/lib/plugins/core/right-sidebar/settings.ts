// The state of the right sidebar: which panels are in it and how wide it is.
// Stored in `.dvoix/plugins/right-sidebar/data.json`, so it belongs to the
// vault and survives a restart.
import { pluginSettings } from "../../pluginSettings.svelte";

/** A panel in the right sidebar: a panel of a plugin or a docked note. */
export interface DockedItem {
  /** "panel" for a panel of a plugin, "note" for a note docked in the sidebar. */
  kind: "panel" | "note";
  /** The id of the panel or the path of the note. */
  id: string;
}

/** The id of a docked item, unique in the sidebar. */
export function itemKey(item: DockedItem): string {
  return `${item.kind}:${item.id}`;
}

export interface RightSidebarSettings {
  /** Whether the sidebar was set up already. The first time it is not. */
  configured: boolean;
  /** The panels in the column, from top to bottom. */
  items: DockedItem[];
  /** Whether the sidebar is hidden. The button in the title row shows it again. */
  collapsed: boolean;
  /** The width of the column in pixels. */
  width: number;
}

export const WIDTH_MIN = 200;
export const WIDTH_MAX = 520;
export const WIDTH_DEFAULT = 280;

/** The id of the panel with the outline of the open note. */
export const OUTLINE_PANEL = "outline";

export const rightSidebarSettings = pluginSettings<RightSidebarSettings>({
  configured: false,
  items: [],
  collapsed: false,
  width: WIDTH_DEFAULT,
});

/** Keeps the width inside the range the column can be dragged to. */
export function clampWidth(px: number): number {
  return Math.round(Math.min(WIDTH_MAX, Math.max(WIDTH_MIN, px)));
}

/** Hides the column of the sidebar, or shows it again. */
export function toggleColumn() {
  const value = rightSidebarSettings.value;
  value.collapsed = !value.collapsed;
  rightSidebarSettings.save();
}
