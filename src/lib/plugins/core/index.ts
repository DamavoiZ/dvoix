// The core plugins, in load order (also the order in the settings list).
// A new feature = a new folder here with an `index.ts` exporting a `Plugin`,
// added to this list. See `../types.ts` for what a plugin can register.
import type { Plugin } from "../types";
import { fileExplorerPlugin } from "./file-explorer";
import { hiddenFilesPlugin } from "./hidden-files";
import { searchPlugin } from "./search";
import { tagsPlugin } from "./tags";
import { backlinksPlugin } from "./backlinks";
import { linkSuggestPlugin } from "./link-suggest";
import { tasksPlugin } from "./tasks";
import { projectsPlugin } from "./projects";
import { canvasPlugin } from "./canvas";
import { graphPlugin } from "./graph";
import { gitPlugin } from "./git";
import { quickSwitcherPlugin } from "./quick-switcher";
import { tabsPlugin } from "./tabs";
import { editorModesPlugin } from "./editor-modes";

export const corePlugins: Plugin[] = [
  fileExplorerPlugin,
  hiddenFilesPlugin,
  searchPlugin,
  tagsPlugin,
  backlinksPlugin,
  linkSuggestPlugin,
  tasksPlugin,
  projectsPlugin,
  canvasPlugin,
  graphPlugin,
  gitPlugin,
  quickSwitcherPlugin,
  tabsPlugin,
  editorModesPlugin,
];
