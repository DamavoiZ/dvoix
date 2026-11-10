// Live preview for the markdown editor: the syntax of a line (`#`, `**`, `[[`,
// ...) is only visible while the cursor is on that line, the way Obsidian shows
// it. The text itself is never changed, only what is displayed.
import { syntaxTree } from "@codemirror/language";
import { Prec, RangeSetBuilder, StateField, type EditorState, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  keymap,
  WidgetType,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { embedImage, isImage, parseEmbed } from "../../attachments";
import { parseFrontmatter } from "./frontmatter";
import { PropertiesWidget } from "./propertiesWidget.svelte";
import { TableWidget, focusTable } from "./tableWidget";

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

/** An embedded image (`![[photo.png]]`), shown in place of its source. */
class ImageWidget extends WidgetType {
  constructor(
    readonly target: string,
    readonly width?: number,
  ) {
    super();
  }
  eq(other: ImageWidget) {
    return other.target === this.target && other.width === this.width;
  }
  toDOM() {
    return embedImage(this.target, this.width);
  }
}

/** An embed that is drawn as an image. */
interface Embed extends Hide {
  target: string;
  width?: number;
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
function collectWikilinks(state: EditorState, range: Range, hides: Hide[], images: Embed[]): void {
  const tree = syntaxTree(state);
  // A wikilink never spans lines, so searching whole lines finds every one on screen.
  const text = state.doc.sliceString(range.from, range.to);
  for (const match of text.matchAll(WIKILINK)) {
    const start = range.from + match.index!;
    const end = start + match[0].length;
    // A wikilink inside code is code, not a link.
    if (tree.resolveInner(start).name === "InlineCode") continue;
    const inner = match[0].slice(2, -2);
    // `![[photo.png]]` shows the image; other embeds stay links.
    if (start > range.from && state.doc.sliceString(start - 1, start) === "!") {
      const embed = parseEmbed(inner);
      if (isImage(embed.target)) {
        images.push({ from: start - 1, to: end, priority: 3, ...embed });
        hides.push({ from: start - 1, to: end, priority: 3 });
        continue;
      }
    }
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
  marks: { from: number; to: number; class: string }[],
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
      // A table is drawn and edited by `blocks` below. Its source only shows
      // when the editor's cursor gets into it (e.g. by search), unchanged.
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
          // The language stays as a small label on the first line of the block.
          marks.push({ from, to, class: "cm-lp-code-info" });
          break;
        case "QuoteMark":
          hide(from, to, true);
          break;
        case "ListMark": {
          const item = node.node.parent;
          if (item?.parent?.name === "OrderedList") {
            // A number stays a number, only its color changes.
            marks.push({ from, to, class: "cm-lp-number" });
          } else if (item?.getChild("Task")) {
            // A task shows its checkbox instead of a bullet.
            hide(from, to, true);
          } else {
            bullets.push({ from, to, priority: 1 });
          }
          break;
        }
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
          addCodeLines(state, lineClasses, from, to, range);
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

/**
 * Marks the lines of a code block, the first and last one extra, so the
 * block is drawn as one box with rounded corners.
 */
function addCodeLines(
  state: EditorState,
  lineClasses: { from: number; class: string }[],
  from: number,
  to: number,
  range: Range,
): void {
  const first = state.doc.lineAt(from).number;
  // `to - 1`: a block can end after its line break.
  const last = state.doc.lineAt(Math.max(from, to - 1)).number;
  const start = Math.max(first, state.doc.lineAt(range.from).number);
  const end = Math.min(last, state.doc.lineAt(range.to).number);
  for (let n = start; n <= end; n++) {
    let className = "cm-lp-code";
    if (n === first) className += " cm-lp-code-first";
    if (n === last) className += " cm-lp-code-last";
    lineClasses.push({ from: state.doc.line(n).from, class: className });
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
  const marks: { from: number; to: number; class: string }[] = [];
  const images: Embed[] = [];

  for (const range of visibleLines(view)) {
    collectWikilinks(state, range, hides, images);
    collectSyntax(state, range, hides, bullets, rules, lineClasses, marks);
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
    const image = images.find((i) => i.from === h.from && i.to === h.to);
    const widget = image ? new ImageWidget(image.target, image.width) : undefined;
    replaces.push({ from: h.from, to: h.to, deco: Decoration.replace(widget ? { widget } : {}) });
  }
  for (const b of bullets) {
    if (active.has(state.doc.lineAt(b.from).number)) continue;
    replaces.push({ from: b.from, to: b.to, deco: Decoration.replace({ widget: new BulletWidget() }) });
  }
  for (const r of rules) {
    if (active.has(state.doc.lineAt(r.from).number)) continue;
    replaces.push({ from: r.from, to: r.to, deco: Decoration.replace({ widget: new RuleWidget() }) });
  }

  for (const m of marks) {
    if (active.has(state.doc.lineAt(m.from).number)) continue;
    replaces.push({ from: m.from, to: m.to, deco: Decoration.mark({ class: m.class }) });
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

/**
 * Replaces the frontmatter by the properties and every table by a
 * `TableWidget`; both are edited inside the widget. Should the editor's own
 * cursor get inside (search, selecting across), the source shows instead,
 * so typing never lands in a hidden range. A
 * replacement over several lines has to come from a state field, CodeMirror
 * does not take it from a view plugin. Only top-level tables are drawn: a
 * table inside a list or quote keeps its source.
 */
function buildBlocks(state: EditorState): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const sel = state.selection.main;
  const outside = (from: number, to: number) => sel.to < from || sel.from > to;
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Document") return true;
      if (node.name === "Frontmatter") {
        // `parseFrontmatter` only takes what it can edit line by line; anything else stays YAML.
        const source = state.doc.sliceString(0, node.to);
        const fm = parseFrontmatter(source);
        if (fm && outside(0, fm.to)) {
          builder.add(0, fm.to, Decoration.replace({ widget: new PropertiesWidget(source.slice(0, fm.to)), block: true }));
        }
      }
      if (node.name === "Table") {
        const from = state.doc.lineAt(node.from).from;
        // `to - 1`: should the node end after its line break, the next line is not part of it.
        const to = state.doc.lineAt(Math.max(node.from, node.to - 1)).to;
        // The editor's cursor touching the table, even at its edge, shows the source.
        if (outside(from, to)) {
          const widget = new TableWidget(state.doc.sliceString(from, to));
          builder.add(from, to, Decoration.replace({ widget, block: true }));
        }
      }
      return false;
    },
  });
  return builder.finish();
}

const blocks = StateField.define<DecorationSet>({
  create: buildBlocks,
  update(decorations, tr) {
    if (tr.docChanged || tr.selection || syntaxTree(tr.startState) !== syntaxTree(tr.state)) {
      return buildBlocks(tr.state);
    }
    return decorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/**
 * Arrow up/down into a drawn table or the properties. CodeMirror moves over a
 * block widget in one step, so the cursor jumped from the line above a table
 * to the line below it. A table gets the focus in its first (down) or last
 * (up) row; for the properties the cursor stops on the YAML, which shows it.
 */
function enterBlock(forward: boolean) {
  return (view: EditorView): boolean => {
    const { state } = view;
    const range = state.selection.main;
    if (!range.empty || state.selection.ranges.length > 1) return false;
    const target = view.moveVertically(range, forward);
    let block: { from: number; to: number; table: boolean } | null = null;
    state.field(blocks).between(Math.min(range.head, target.head), Math.max(range.head, target.head), (from, to, deco) => {
      if (forward ? from <= range.head : to >= range.head) return;
      // Down takes the first block on the way, up the last one.
      block = { from, to, table: deco.spec.widget instanceof TableWidget };
      if (forward) return false;
    });
    if (!block) return false;
    const { from, to, table } = block;
    // A table is edited in its cells: the focus goes into its first or last row.
    if (table && focusTable(view, from, !forward)) return true;
    const row = state.doc.lineAt(forward ? from : to);
    const column = range.head - state.doc.lineAt(range.head).from;
    view.dispatch({
      selection: { anchor: row.from + Math.min(column, row.length) },
      scrollIntoView: true,
      userEvent: "select",
    });
    return true;
  };
}

const blockKeys = Prec.high(
  keymap.of([
    { key: "ArrowDown", run: enterBlock(true) },
    { key: "ArrowUp", run: enterBlock(false) },
  ]),
);

/** Hides the markdown syntax of every line the cursor is not on, draws tables and the properties. */
export function livePreview(): Extension {
  return [livePreviewPlugin, blocks, blockKeys];
}
