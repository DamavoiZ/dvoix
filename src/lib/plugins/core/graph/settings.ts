// Settings of the graph view, in `.dvoix/plugins/graph/data.json`.
import { pluginSettings } from "../../pluginSettings.svelte";

/** Tags as own nodes, or as direct lines between notes that share a tag. */
export type TagMode = "nodes" | "shared";
export type ColorBy = "none" | "folder" | "tag";

export const GRAPH_DEFAULTS = {
  /** Lines for [[wikilinks]] and markdown links. */
  links: true,
  /** Folders as nodes: every note hangs on its folder, every folder on its parent. */
  folders: false,
  /** Connect notes through their #tags. */
  tags: false,
  tagMode: "nodes" as TagMode,
  /** Space-separated terms that must match path, title or #tag; `-term` hides matches. */
  filter: "",
  /** Notes without any line. */
  orphans: true,
  /** Linked files that are not notes (images, PDFs, canvases ...). */
  attachments: false,
  /** Links to notes that don't exist yet. */
  unresolved: false,
  colorBy: "none" as ColorBy,
  /** Bigger nodes for notes with more lines. */
  sizeByLinks: true,
  nodeSize: 1,
  linkWidth: 1,
  arrows: false,
  /** Zoom level from which titles are shown (always shown for the hovered node). */
  labelZoom: 1.5,
  /** Forces of the layout. */
  repel: 60,
  linkDistance: 40,
  linkStrength: 0.6,
  center: 0.08,
};

export type GraphSettings = typeof GRAPH_DEFAULTS;

export const graphSettings = pluginSettings({ ...GRAPH_DEFAULTS });
