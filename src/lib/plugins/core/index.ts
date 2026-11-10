// The core plugins: optional functions that ship with the app and can be turned
// off in the settings. Basic functions that are always there live in
// `src/lib/features/` instead. Order = load order (also the order in the
// settings list). See `../types.ts` for what a plugin can register.
import type { Plugin } from "../types";
import { hiddenFilesPlugin } from "./hidden-files";
import { tasksPlugin } from "./tasks";
import { projectsPlugin } from "./projects";
import { canvasPlugin } from "./canvas";
import { graphPlugin } from "./graph";
import { gitPlugin } from "./git";
import { rightSidebarPlugin } from "./right-sidebar";

export const corePlugins: Plugin[] = [
  hiddenFilesPlugin,
  tasksPlugin,
  projectsPlugin,
  canvasPlugin,
  graphPlugin,
  gitPlugin,
  rightSidebarPlugin,
];
