// Markdown editor extensions of the tasks plugin: `[ ]` / `[x]` as real,
// clickable checkboxes, finished tasks dimmed, due dates and priorities highlighted.
import { RangeSetBuilder, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  MatchDecorator,
  ViewPlugin,
  WidgetType,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";

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

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }
  eq(other: CheckboxWidget) {
    return other.checked === this.checked;
  }
  toDOM() {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-task-checkbox";
    box.checked = this.checked;
    box.setAttribute("aria-label", this.checked ? "Mark as open" : "Mark as done");
    return box;
  }
  ignoreEvent() {
    return false;
  }
}

/** Shows `[ ]` / `[x]` of task list items as real checkboxes and dims finished tasks. */
function buildTaskDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter(node) {
        if (node.name !== "TaskMarker") return;
        const checked = view.state.doc.sliceString(node.from + 1, node.to - 1).trim() !== "";
        builder.add(node.from, node.to, Decoration.replace({ widget: new CheckboxWidget(checked) }));
        const line = view.state.doc.lineAt(node.from);
        if (checked && node.to < line.to) builder.add(node.to, line.to, Decoration.mark({ class: "cm-task-done" }));
      },
    });
  }
  return builder.finish();
}

const taskCheckboxes = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildTaskDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || syntaxTree(update.startState) !== syntaxTree(update.state)) {
        this.decorations = buildTaskDecorations(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

/** Flips the `[ ]`/`[x]` marker whose checkbox widget was clicked. */
function toggleCheckbox(view: EditorView, box: HTMLElement): boolean {
  const pos = view.posAtDOM(box);
  const marker = view.state.doc.sliceString(pos, pos + 3);
  if (!/^\[[ xX]\]$/.test(marker)) return false;
  view.dispatch({ changes: { from: pos + 1, to: pos + 2, insert: marker[1] === " " ? "x" : " " } });
  return true;
}

const taskTheme = EditorView.theme({
    ".cm-task-checkbox": {
      width: "0.95em",
      height: "0.95em",
      margin: "0 0.15em",
      verticalAlign: "-0.1em",
      accentColor: "#8b5cf6",
      cursor: "pointer",
    },
    ".cm-task-done": { color: "#71717a", textDecoration: "line-through" },
    ".cm-task-meta": { color: "#a1a1aa", fontSize: "0.9em", backgroundColor: "rgba(255,255,255,0.05)", borderRadius: "4px" },
});

const checkboxClicks = EditorView.domEventHandlers({
  mousedown(event, view) {
    const el = event.target as HTMLElement;
    if (!el.classList.contains("cm-task-checkbox")) return false;
    event.preventDefault();
    return toggleCheckbox(view, el);
  },
});

export interface TaskEditorOptions {
  /** Show `[ ]` as clickable checkboxes. */
  checkboxes: boolean;
  /** Highlight due dates and priorities. */
  highlightMeta: boolean;
}

export function taskEditorExtensions(opts: TaskEditorOptions): Extension[] {
  const out: Extension[] = [taskTheme];
  if (opts.checkboxes) out.push(taskCheckboxes, checkboxClicks);
  if (opts.highlightMeta) out.push(taskMeta);
  return out;
}
