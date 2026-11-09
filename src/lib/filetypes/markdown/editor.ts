import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  MatchDecorator,
  ViewPlugin,
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
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
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
    "&": { color: "#e4e4e7", backgroundColor: "transparent", fontSize: "var(--editor-font-size, 15px)" },
    ".cm-content": { maxWidth: "var(--editor-max-width, 48rem)", margin: "0 auto", padding: "2rem 1.5rem", lineHeight: "1.7", caretColor: "#a78bfa" },
    ".cm-scroller": { fontFamily: "var(--font-sans)" },
    ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.03)" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "rgba(167,139,250,0.3) !important" },
    ".cm-cursor": { borderLeftColor: "#a78bfa" },
    ".cm-gutters": { backgroundColor: "transparent", border: "none", color: "#52525b" },
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
  /** Extra CodeMirror extensions, e.g. from plugins (task checkboxes) or settings (line numbers). */
  extensions?: Extension[];
}

/** Holds the extensions from plugins and settings, so they can be swapped without a new editor. */
const extra = new Compartment();

/** Replaces the plugin/settings extensions of a running editor. */
export function setExtensions(view: EditorView, extensions: Extension[]): void {
  view.dispatch({ effects: extra.reconfigure(extensions) });
}

export function createMarkdownEditor({ parent, doc, onchange, onlink, extensions = [] }: EditorOptions): EditorView {
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
        extra.of(extensions),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) onchange(u.state.doc.toString());
        }),
        EditorView.domEventHandlers({
          mousedown(event, view) {
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
