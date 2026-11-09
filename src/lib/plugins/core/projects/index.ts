import { FolderKanban } from "@lucide/svelte";
import type { Plugin } from "../../types";
import ProjectsPanel from "./ProjectsPanel.svelte";

export const projectsPlugin: Plugin = {
  manifest: {
    id: "projects",
    name: "Projects",
    description: "Notes with \"type: project\" in their frontmatter, listed as projects with status, due date and progress.",
    version: "1.0.0",
  },
  icon: FolderKanban,
  onload(ctx) {
    ctx.addPanel({ id: "projects", label: "Projects", icon: FolderKanban, component: ProjectsPanel, order: 50 });
  },
};
