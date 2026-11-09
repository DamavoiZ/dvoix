import type { FileType } from "../types";
import MarkdownView from "./MarkdownView.svelte";

export const markdownType: FileType = {
  id: "markdown",
  label: "Notiz",
  extensions: ["md", "markdown"],
  icon: "¶",
  view: MarkdownView,
  template: (name) => `# ${name}\n\n`,
};
