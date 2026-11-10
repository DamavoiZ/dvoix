// The core features: basic note and editor functions that are always there and
// cannot be turned off (file explorer, search, tags, backlinks, tabs, ...).
// They hook into the app through the same `PluginContext` as plugins, so the
// app frame stays free of special cases, but the manager loads them for every
// vault and the settings show no on/off switch for them. Optional functions
// stay plugins under `../plugins/core/`. Order = load order.
import type { Plugin } from "../plugins/types";
import { fileExplorerFeature } from "./file-explorer";
import { searchFeature } from "./search";
import { tagsFeature } from "./tags";
import { backlinksFeature } from "./backlinks";
import { linkSuggestFeature } from "./link-suggest";
import { quickSwitcherFeature } from "./quick-switcher";
import { tabsFeature } from "./tabs";
import { editorModesFeature } from "./editor-modes";

export const coreFeatures: Plugin[] = [
  fileExplorerFeature,
  searchFeature,
  tagsFeature,
  backlinksFeature,
  linkSuggestFeature,
  quickSwitcherFeature,
  tabsFeature,
  editorModesFeature,
];
