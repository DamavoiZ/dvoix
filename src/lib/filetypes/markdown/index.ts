import type { FileType } from "../types";
import { FileText } from "@lucide/svelte";
import MarkdownView from "./MarkdownView.svelte";

export const markdownType: FileType = {
  id: "markdown",
  label: "Notiz",
  extensions: ["md", "markdown"],
  icon: FileText,
  view: MarkdownView,
  template: (name) => `# ${name}\n\n`,
};
