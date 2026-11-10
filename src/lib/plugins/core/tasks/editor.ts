// Markdown editor extensions of the tasks plugin: due dates and priorities
// highlighted. The clickable checkboxes are the core feature `checkboxes`.
import type { Extension } from "@codemirror/state";
import { Decoration, EditorView, MatchDecorator, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";

/** Due dates and priorities inside tasks (`📅 2026-12-01`, `due:2026-12-01`, `!high`, `⏫`). */
const TASK_META = /(?:📅|🗓️?)\s?\d{4}-\d{2}-\d{2}|\bdue:\d{4}-\d{2}-\d{2}|(?<=\s)!(?:hoch|mittel|niedrig|high|medium|low)\b|[⏫🔼🔽🔺⏬]/gu;

const taskMeta = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    private matcher = new MatchDecorator({
      regexp: TASK_META,
      decoration: () => Decoration.mark({ class: "cm-task-meta" }),
    });
    constructor(view: EditorView) {
      this.decorations = this.matcher.createDeco(view);
    }
    update(update: ViewUpdate) {
      this.decorations = this.matcher.updateDeco(update, this.decorations);
    }
  },
  { decorations: (v) => v.decorations },
);

const taskTheme = EditorView.theme({
  ".cm-task-meta": { color: "#a1a1aa", fontSize: "0.9em", backgroundColor: "rgba(255,255,255,0.05)", borderRadius: "4px" },
});

/** Highlights due dates and priorities in the editor. */
export function taskMetaExtensions(): Extension[] {
  return [taskTheme, taskMeta];
}
