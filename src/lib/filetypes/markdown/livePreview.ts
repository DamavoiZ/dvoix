// Live preview for the markdown editor: the syntax of a line (`#`, `**`, `[[`,
// ...) is only visible while the cursor is on that line, the way Obsidian shows
// it. The text itself is never changed, only what is displayed.
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder, type EditorState, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";

/** `[[wikilinks]]` are not markdown, so they are found with a regex. */
const WIKILINK = /\[\[[^\]\n]+\]\]/g;

/** A range of markdown syntax that is hidden while the cursor is elsewhere. */
interface Hide {
  from: number;
  to: number;
  /** Wikilinks win: the markdown parser sees them as a plain link. */
  priority: number;
}

/** The bullet in front of a list item. */
class BulletWidget extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-lp-bullet";
    el.textContent = "•";
    return el;
  }
  ignoreEvent() {
    return true;
  }
}

/** A horizontal rule, drawn as a line instead of three dashes. */
class RuleWidget extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    const el = document.createElement("hr");
    el.className = "cm-lp-rule";
    return el;
  }
  ignoreEvent() {
    return true;
  }
}

/** The lines the cursor is on: their syntax stays visible. */
function activeLines(state: EditorState): Set<number> {
  const lines = new Set<number>();
  const range = state.selection.main;
  const first = state.doc.lineAt(range.from).number;
  const last = state.doc.lineAt(range.to).number;
  for (let n = first; n <= last; n++) lines.add(n);
  return lines;
}

/** Hides the brackets of a wikilink and shows its name (or its alias). */
function collectWikilinks(state: EditorState, hides: Hide[]): void {
  const tree = syntaxTree(state);
  // The whole document is searched: the syntax tree is parsed for it anyway,
  // and a wikilink outside the viewport still has to be found.
  const text = state.doc.sliceString(0, state.doc.length);
  for (const match of text.matchAll(WIKILINK)) {
    const start = match.index!;
    const end = start + match[0].length;
    // A wikilink inside code is code, not a link.
    if (tree.resolveInner(start).name === "InlineCode") continue;
    const inner = match[0].slice(2, -2);
    const pipe = inner.indexOf("|");
    const label = pipe >= 0 ? inner.slice(pipe + 1) : inner;
    const offset = pipe >= 0 ? pipe + 1 : 0;
    // `[[Note#Section]]` shows the note, not the section.
    const hash = label.indexOf("#");
    const shown = hash >= 0 ? label.slice(0, hash) : label;
    const visibleFrom = start + 2 + offset;
    const visibleTo = visibleFrom + shown.length;
    hides.push({ from: start, to: visibleFrom, priority: 2 });
    hides.push({ from: visibleTo, to: end, priority: 2 });
  }
}

/** Collects the syntax of the markdown parser: markers that get hidden. */
function collectSyntax(
  state: EditorState,
  hides: Hide[],
  bullets: Hide[],
  rules: Hide[],
  lineClasses: { from: number; class: string }[],
): void {
  /** Hides a marker and the space behind it. */
  const hide = (from: number, to: number, withSpace = false) => {
    const line = state.doc.lineAt(to);
    const end = withSpace ? Math.min(to + 1, line.to) : to;
    hides.push({ from, to: end, priority: 1 });
  };

  syntaxTree(state).iterate({
    enter(node) {
      const { from, to, name } = node;
      const parent = node.node.parent?.name;
      switch (name) {
        case "HeaderMark":
          hide(from, to, true);
          break;
        case "EmphasisMark":
        case "StrikethroughMark":
          hide(from, to);
          break;
        case "CodeMark":
          // The backticks of a fenced code block are its fence, the ones of
          // inline code surround the code. Both are hidden.
          hide(from, to);
          break;
        case "CodeInfo":
          hide(from, to);
          break;
        case "QuoteMark":
          hide(from, to, true);
          break;
        case "ListMark":
          bullets.push({ from, to, priority: 1 });
          break;
        case "LinkMark":
          hide(from, to);
          break;
        case "URL":
          // The target of a link is not part of its text.
          if (parent === "Link") hide(from, to);
          break;
        case "HorizontalRule":
          rules.push({ from, to, priority: 1 });
          break;
        case "Blockquote":
          addLineClasses(state, lineClasses, from, to, "cm-lp-quote");
          break;
        case "FencedCode":
        case "CodeBlock":
          addLineClasses(state, lineClasses, from, to, "cm-lp-code");
          break;
        case "Frontmatter":
          addLineClasses(state, lineClasses, from, to, "cm-lp-frontmatter");
          break;
      }
    },
  });
}

/** Marks every line of a range with a class. */
function addLineClasses(
  state: EditorState,
  lineClasses: { from: number; class: string }[],
  from: number,
  to: number,
  className: string,
): void {
  let pos = from;
  while (pos <= to) {
    const line = state.doc.lineAt(pos);
    lineClasses.push({ from: line.from, class: className });
    if (line.to >= to) break;
    pos = line.to + 1;
  }
}

/** Builds the decorations of the live preview for the current state. */
function buildDecorations(view: EditorView): DecorationSet {
  const { state } = view;
  const active = activeLines(state);
  const hides: Hide[] = [];
  const bullets: Hide[] = [];
  const rules: Hide[] = [];
  const lineClasses: { from: number; class: string }[] = [];

  collectWikilinks(state, hides);
  collectSyntax(state, hides, bullets, rules, lineClasses);

  // Ranges must not overlap. The first one wins, and a longer range comes
  // first when two start at the same place.
  const sorted = [...hides].sort((a, b) => a.from - b.from || b.to - a.to || b.priority - a.priority);
  const accepted: Hide[] = [];
  let end = -1;
  for (const h of sorted) {
    if (h.from < end) continue;
    accepted.push(h);
    end = h.to;
  }

  const replaces: { from: number; to: number; deco: Decoration }[] = [];
  for (const h of accepted) {
    // The line of the cursor keeps its syntax.
    if (active.has(state.doc.lineAt(h.from).number)) continue;
    replaces.push({ from: h.from, to: h.to, deco: Decoration.replace({}) });
  }
  for (const b of bullets) {
    if (active.has(state.doc.lineAt(b.from).number)) continue;
    replaces.push({ from: b.from, to: b.to, deco: Decoration.replace({ widget: new BulletWidget() }) });
  }
  for (const r of rules) {
    if (active.has(state.doc.lineAt(r.from).number)) continue;
    replaces.push({ from: r.from, to: r.to, deco: Decoration.replace({ widget: new RuleWidget() }) });
  }

  const lines: { from: number; to: number; deco: Decoration }[] = [];
  const sortedLines = [...lineClasses].sort((a, b) => a.from - b.from);
  let lastLine = -1;
  for (const l of sortedLines) {
    if (l.from === lastLine) continue;
    lastLine = l.from;
    lines.push({ from: l.from, to: l.from, deco: Decoration.line({ class: l.class }) });
  }

  // CodeMirror wants the ranges sorted by position, and a line decoration
  // comes before a replacement at the same position.
  const entries = [...lines, ...replaces].sort((a, b) => a.from - b.from || a.to - b.to);
  const builder = new RangeSetBuilder<Decoration>();
  for (const e of entries) builder.add(e.from, e.to, e.deco);
  return builder.finish();
}

const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(update: ViewUpdate) {
      if (
        update.docChanged ||
        update.selectionSet ||
        update.viewportChanged ||
        syntaxTree(update.startState) !== syntaxTree(update.state)
      ) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

/** Hides the markdown syntax of every line the cursor is not on. */
export function livePreview(): Extension {
  return livePreviewPlugin;
}
