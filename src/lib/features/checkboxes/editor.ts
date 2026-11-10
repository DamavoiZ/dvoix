// Markdown editor extensions of the checkboxes feature: `[ ]` / `[x]` of list
// items as real, clickable checkboxes, finished items dimmed. Clicking changes
// only the one character inside the brackets.
import { RangeSetBuilder, type Extension } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";

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

/** Shows `[ ]` / `[x]` of task list items as real checkboxes and dims finished ones. */
function buildDecorations(view: EditorView): DecorationSet {
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

const checkboxes = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || syntaxTree(update.startState) !== syntaxTree(update.state)) {
        this.decorations = buildDecorations(update.view);
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

const checkboxClicks = EditorView.domEventHandlers({
  mousedown(event, view) {
    const el = event.target as HTMLElement;
    if (!el.classList.contains("cm-task-checkbox")) return false;
    event.preventDefault();
    return toggleCheckbox(view, el);
  },
});

const theme = EditorView.theme({
  ".cm-task-checkbox": {
    width: "0.95em",
    height: "0.95em",
    margin: "0 0.15em",
    verticalAlign: "-0.1em",
    accentColor: "#8b5cf6",
    cursor: "pointer",
  },
  ".cm-task-done": { color: "#71717a", textDecoration: "line-through" },
});

export function checkboxExtensions(): Extension[] {
  return [theme, checkboxes, checkboxClicks];
}
