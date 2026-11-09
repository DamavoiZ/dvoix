import { EditorState, RangeSetBuilder } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  MatchDecorator,
  ViewPlugin,
  WidgetType,
  drawSelection,
  highlightActiveLine,
  keymap,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import type { MarkdownConfig } from "@lezer/markdown";
import { languages } from "@codemirror/language-data";
import { HighlightStyle, syntaxHighlighting, syntaxTree } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

const WIKILINK = /\[\[([^\]\n]+)\]\]/g;

/**
 * Treats a `---` block at the very start of the note as YAML frontmatter, so
 * it is shown as metadata instead of being parsed as a heading.
 */
const frontmatter: MarkdownConfig = {
  defineNodes: [{ name: "Frontmatter", block: true, style: t.meta }],
  parseBlock: [
    {
      name: "Frontmatter",
      before: "HorizontalRule",
      parse(cx, line) {
        if (cx.lineStart !== 0 || line.text.trimEnd() !== "---") return false;
        const from = cx.lineStart;
        while (cx.nextLine()) {
          if (line.text.trimEnd() === "---") {
            cx.nextLine();
            break;
          }
        }
        cx.addElement(cx.elt("Frontmatter", from, cx.prevLineEnd()));
        return true;
      },
    },
  ],
};

/** Highlights [[wikilinks]] so they look clickable. */
const wikilinks = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    private matcher = new MatchDecorator({
      regexp: WIKILINK,
      decoration: () => Decoration.mark({ class: "cm-wikilink" }),
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

/** Due dates and priorities inside tasks (`📅 2026-12-01`, `due:2026-12-01`, `!hoch`, `⏫`). */
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
    box.setAttribute("aria-label", this.checked ? "Als offen markieren" : "Als erledigt markieren");
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

/** Moves the cursor to a 0-based line and scrolls it into the middle of the editor. */
export function revealLine(view: EditorView, line: number): void {
  const n = Math.min(Math.max(line + 1, 1), view.state.doc.lines);
  const pos = view.state.doc.line(n).from;
  view.dispatch({ selection: { anchor: pos }, effects: EditorView.scrollIntoView(pos, { y: "center" }) });
  view.focus();
}

/** Returns the wikilink target at a document position, if any. */
function wikilinkAt(state: EditorState, pos: number): string | undefined {
  const line = state.doc.lineAt(pos);
  for (const m of line.text.matchAll(WIKILINK)) {
    const from = line.from + m.index!;
    if (pos >= from && pos <= from + m[0].length) {
      return m[1].split(/[|#]/)[0].trim();
    }
  }
  return undefined;
}

const highlight = HighlightStyle.define([
  { tag: t.heading1, fontSize: "1.6em", fontWeight: "700" },
  { tag: t.heading2, fontSize: "1.35em", fontWeight: "700" },
  { tag: t.heading3, fontSize: "1.15em", fontWeight: "600" },
  { tag: [t.heading4, t.heading5, t.heading6], fontWeight: "600" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strikethrough, textDecoration: "line-through" },
  { tag: [t.link, t.url], color: "#a78bfa" },
  { tag: t.monospace, fontFamily: "var(--font-mono)", color: "#fbbf24" },
  { tag: t.quote, color: "#a1a1aa", fontStyle: "italic" },
  { tag: [t.processingInstruction, t.meta, t.contentSeparator], color: "#71717a" },
  { tag: t.keyword, color: "#f472b6" },
  { tag: t.string, color: "#86efac" },
  { tag: t.comment, color: "#71717a" },
]);

const theme = EditorView.theme(
  {
    "&": { color: "#e4e4e7", backgroundColor: "transparent", fontSize: "15px" },
    ".cm-content": { maxWidth: "48rem", margin: "0 auto", padding: "2rem 1.5rem", lineHeight: "1.7", caretColor: "#a78bfa" },
    ".cm-scroller": { fontFamily: "var(--font-sans)" },
    ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.03)" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "rgba(167,139,250,0.3) !important" },
    ".cm-cursor": { borderLeftColor: "#a78bfa" },
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
    ".cm-wikilink": { color: "#a78bfa", textDecoration: "underline", textDecorationColor: "rgba(167,139,250,0.4)", cursor: "pointer" },
  },
  { dark: true },
);

export interface EditorOptions {
  parent: HTMLElement;
  doc: string;
  onchange: (doc: string) => void;
  /** Called on Ctrl/Cmd+click on a [[wikilink]] with its target. */
  onlink: (target: string) => void;
}

export function createMarkdownEditor({ parent, doc, onchange, onlink }: EditorOptions): EditorView {
  return new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        history(),
        drawSelection(),
        highlightActiveLine(),
        EditorView.lineWrapping,
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        markdown({ base: markdownLanguage, codeLanguages: languages, extensions: [frontmatter] }),
        syntaxHighlighting(highlight),
        theme,
        wikilinks,
        taskCheckboxes,
        taskMeta,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) onchange(u.state.doc.toString());
        }),
        EditorView.domEventHandlers({
          mousedown(event, view) {
            const el = event.target as HTMLElement;
            if (el.classList.contains("cm-task-checkbox")) {
              event.preventDefault();
              return toggleCheckbox(view, el);
            }
            if (!(event.ctrlKey || event.metaKey)) return false;
            const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
            const target = pos == null ? undefined : wikilinkAt(view.state, pos);
            if (!target) return false;
            event.preventDefault();
            onlink(target);
            return true;
          },
        }),
      ],
    }),
  });
}

/** Replaces the document (e.g. after the file changed on disk) without losing the editor. */
export function setDoc(view: EditorView, doc: string): void {
  if (view.state.doc.toString() === doc) return;
  const head = Math.min(view.state.selection.main.head, doc.length);
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: doc }, selection: { anchor: head } });
}
