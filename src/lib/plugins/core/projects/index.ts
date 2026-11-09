import { FolderKanban } from "@lucide/svelte";
import type { Plugin } from "../../types";
import ProjectsPanel from "./ProjectsPanel.svelte";

export const projectsPlugin: Plugin = {
  manifest: {
    id: "projects",
    name: "Projekte",
    description: "Notizen mit „type: projekt“ im Frontmatter als Projektliste mit Status, Fälligkeit und Fortschritt.",
    version: "1.0.0",
  },
  icon: FolderKanban,
  onload(ctx) {
    ctx.addPanel({ id: "projects", label: "Projekte", icon: FolderKanban, component: ProjectsPanel, order: 50 });
  },
};
