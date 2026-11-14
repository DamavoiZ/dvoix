import type { FileType } from "../types";
import { FileText } from "@lucide/svelte";
import MarkdownHeaderActions from "./MarkdownHeaderActions.svelte";
import MarkdownView from "./MarkdownView.svelte";

export const markdownType: FileType = {
  id: "markdown",
  label: "Note",
  extensions: ["md", "markdown"],
  icon: FileText,
  view: MarkdownView,
  headerActions: MarkdownHeaderActions,
  template: (name) => `# ${name}\n\n`,
};
