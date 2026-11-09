// Live preview for the markdown editor: the syntax of a line (`#`, `**`, `[[`,
// ...) is only visible while the cursor is on that line, the way Obsidian shows
// it. The text itself is never changed, only what is displayed.
import { syntaxTree } from "@codemirror/language";
import { RangeSetBuilder, StateField, type EditorState, type Extension } from "@codemirror/state";
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

/** A part of the document that is on screen. */
interface Range {
  from: number;
  to: number;
}

/**
 * The visible ranges, widened to whole lines. Only these get decorations:
 * the document outside is not drawn, and rebuilding the whole note on every
 * key press and scroll step made typing and scrolling in long notes stutter.
 */
function visibleLines(view: EditorView): Range[] {
  const { doc } = view.state;
  return view.visibleRanges.map(({ from, to }) => ({ from: doc.lineAt(from).from, to: doc.lineAt(to).to }));
}

/** Hides the brackets of a wikilink and shows its name (or its alias). */
function collectWikilinks(state: EditorState, range: Range, hides: Hide[]): void {
  const tree = syntaxTree(state);
  // A wikilink never spans lines, so searching whole lines finds every one on screen.
  const text = state.doc.sliceString(range.from, range.to);
  for (const match of text.matchAll(WIKILINK)) {
    const start = range.from + match.index!;
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
  range: Range,
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
    from: range.from,
    to: range.to,
    enter(node) {
      const { from, to, name } = node;
      const parent = node.node.parent?.name;
      // A table off the cursor is drawn by `tables` below. With the cursor in
      // it, its source stays as it is, so the pipes line up while editing.
      if (name === "Table") {
        addLineClasses(state, lineClasses, Math.max(from, range.from), Math.min(to, range.to), "cm-lp-table-source");
        return false;
      }
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
          addLineClasses(state, lineClasses, Math.max(from, range.from), Math.min(to, range.to), "cm-lp-quote");
          break;
        case "FencedCode":
        case "CodeBlock":
          addLineClasses(state, lineClasses, Math.max(from, range.from), Math.min(to, range.to), "cm-lp-code");
          break;
        case "Frontmatter":
          addLineClasses(state, lineClasses, Math.max(from, range.from), Math.min(to, range.to), "cm-lp-frontmatter");
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

  for (const range of visibleLines(view)) {
    collectWikilinks(state, range, hides);
    collectSyntax(state, range, hides, bullets, rules, lineClasses);
  }

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

/** How a column is aligned, from the `:---:` row of a table. */
type Align = "left" | "center" | "right" | null;

/** Splits a table row at its pipes. `\|` and pipes inside `code` are text. */
function splitRow(line: string): string[] {
  let text = line.trim();
  if (text.startsWith("|")) text = text.slice(1);
  if (text.endsWith("|") && !text.endsWith("\\|")) text = text.slice(0, -1);
  const cells: string[] = [];
  let cell = "";
  let inCode = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === "\\" && text[i + 1] === "|") {
      cell += "|";
      i++;
    } else if (ch === "`") {
      inCode = !inCode;
      cell += ch;
    } else if (ch === "|" && !inCode) {
      cells.push(cell.trim());
      cell = "";
    } else {
      cell += ch;
    }
  }
  cells.push(cell.trim());
  return cells;
}

function alignOf(cell: string): Align {
  const left = cell.startsWith(":");
  const right = cell.endsWith(":");
  if (left && right) return "center";
  if (right) return "right";
  if (left) return "left";
  return null;
}

/** Inline markdown inside a cell: code, bold, italic, strikethrough, links. `_` only counts outside of words, so `snake_case` stays text. */
const INLINE = /`([^`]+)`|\*\*(.+?)\*\*|(?<!\w)__(.+?)__(?!\w)|~~(.+?)~~|\*(.+?)\*|(?<!\w)_(.+?)_(?!\w)|\[\[([^\]]+)\]\]|\[([^\]]*)\]\([^)]*\)/g;

/**
 * Fills a cell with its inline markdown. Built from DOM nodes instead of
 * HTML, so text from the note can never become markup.
 */
function renderInline(text: string, parent: HTMLElement): void {
  let last = 0;
  for (const m of text.matchAll(INLINE)) {
    if (m.index! > last) parent.append(text.slice(last, m.index));
    last = m.index! + m[0].length;
    const [, code, bold, bold2, strike, em, em2, wikilink, link] = m;
    let el: HTMLElement;
    if (code !== undefined) {
      el = document.createElement("code");
      el.textContent = code;
    } else if (bold !== undefined || bold2 !== undefined) {
      el = document.createElement("strong");
      renderInline(bold ?? bold2, el);
    } else if (strike !== undefined) {
      el = document.createElement("s");
      renderInline(strike, el);
    } else if (em !== undefined || em2 !== undefined) {
      el = document.createElement("em");
      renderInline(em ?? em2, el);
    } else if (wikilink !== undefined) {
      el = document.createElement("span");
      el.className = "cm-wikilink";
      const pipe = wikilink.indexOf("|");
      el.textContent = pipe >= 0 ? wikilink.slice(pipe + 1) : wikilink.split("#")[0];
    } else {
      el = document.createElement("span");
      el.className = "cm-lp-link";
      renderInline(link, el);
    }
    parent.append(el);
  }
  if (last < text.length) parent.append(text.slice(last));
}

/** A markdown table, drawn as a real table while the cursor is outside of it. */
class TableWidget extends WidgetType {
  constructor(readonly source: string) {
    super();
  }
  eq(other: TableWidget) {
    return other.source === this.source;
  }
  toDOM() {
    const [header = "", delimiter = "", ...rows] = this.source.split("\n");
    const aligns = splitRow(delimiter).map(alignOf);
    const head = splitRow(header);
    const columns = head.length;

    const wrap = document.createElement("div");
    wrap.className = "cm-lp-table-wrap";
    const table = document.createElement("table");
    table.className = "cm-lp-table";
    const row = (cells: string[], tag: "th" | "td") => {
      const tr = document.createElement("tr");
      // Like GitHub: every row has as many cells as the header.
      for (let i = 0; i < columns; i++) {
        const cell = document.createElement(tag);
        const align = aligns[i];
        if (align) cell.style.textAlign = align;
        renderInline(cells[i] ?? "", cell);
        tr.append(cell);
      }
      return tr;
    };
    const thead = document.createElement("thead");
    thead.append(row(head, "th"));
    const tbody = document.createElement("tbody");
    for (const r of rows) tbody.append(row(splitRow(r), "td"));
    table.append(thead, tbody);
    wrap.append(table);
    return wrap;
  }
  /** A click goes to the editor: the cursor lands in the table and shows its source. */
  ignoreEvent() {
    return false;
  }
}

/**
 * Replaces every table the cursor is not in with a `TableWidget`. A
 * replacement over several lines has to come from a state field, CodeMirror
 * does not take it from a view plugin. Only top-level tables are drawn: a
 * table inside a list or quote keeps its source.
 */
function buildTables(state: EditorState): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const sel = state.selection.main;
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Document") return true;
      if (node.name === "Table") {
        const from = state.doc.lineAt(node.from).from;
        // `to - 1`: should the node end after its line break, the next line is not part of it.
        const to = state.doc.lineAt(Math.max(node.from, node.to - 1)).to;
        // The cursor touching the table, even at its edge, shows the source.
        if (sel.to < from || sel.from > to) {
          const widget = new TableWidget(state.doc.sliceString(from, to));
          builder.add(from, to, Decoration.replace({ widget, block: true }));
        }
      }
      return false;
    },
  });
  return builder.finish();
}

const tables = StateField.define<DecorationSet>({
  create: buildTables,
  update(decorations, tr) {
    if (tr.docChanged || tr.selection || syntaxTree(tr.startState) !== syntaxTree(tr.state)) {
      return buildTables(tr.state);
    }
    return decorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/** Hides the markdown syntax of every line the cursor is not on and draws tables. */
export function livePreview(): Extension {
  return [livePreviewPlugin, tables];
}
