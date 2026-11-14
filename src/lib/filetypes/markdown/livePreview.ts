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
import { embedImage, embeddedImageHeight, isImage, parseEmbed } from "../../attachments";
import { SCROLL_SETTLE_MS } from "../../editorScroll";
import { createCalloutIcon, resolveCallout, type CalloutKind } from "./callouts";
import { parseFrontmatter } from "./frontmatter";
import { PropertiesWidget } from "./propertiesWidget.svelte";
import { TableWidget, focusTable } from "./tableWidget";

/** `[[wikilinks]]` are not markdown, so they are found with a regex. */
const WIKILINK = /\[\[[^\]\n]+\]\]/g;
const CALLOUT_HEADER = /^(\s*>\s*\[!([a-zA-Z-]+)\])(?:[ \t]+(.*))?$/;

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

/** Icon (and fallback title) replacing `> [!TYPE]` on a callout's first line. */
class CalloutBadgeWidget extends WidgetType {
  constructor(
    readonly kind: CalloutKind,
    readonly label?: string,
  ) {
    super();
  }
  eq(other: CalloutBadgeWidget) {
    return other.kind === this.kind && other.label === this.label;
  }
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-lp-callout-badge";
    el.appendChild(createCalloutIcon(this.kind));
    if (this.label) {
      const span = document.createElement("span");
      span.textContent = this.label;
      el.appendChild(span);
    }
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
    readonly width: number | undefined,
    /** The editor the image is drawn in; asked for the line width when the height is estimated. */
    readonly view: EditorView,
  ) {
    super();
  }
  eq(other: ImageWidget) {
    // Only what the image shows decides whether its DOM can stay.
    return other.target === this.target && other.width === this.width;
  }
  /**
   * The height the image will take, from the size of the file remembered
   * when it was first loaded. CodeMirror uses it for the lines it has not
   * drawn yet, so the scrollbar and the text below do not jump when the
   * image comes into view. Unknown size: -1, CodeMirror measures the DOM.
   */
  get estimatedHeight() {
    // The text inside `.cm-line` is inset by 6px left + 2px right (`editor.ts` / CodeMirror base theme).
    const lineWidth = Math.max(1, this.view.contentDOM.clientWidth - 8);
    return embeddedImageHeight(this.target, this.width ?? lineWidth) ?? -1;
  }
  toDOM(view: EditorView) {
    // Once the image has its size the editor measures again: until then the
    // line was only as tall as its text.
    return embedImage(this.target, this.width, () => view.requestMeasure());
  }
}

/** An embed that is drawn as an image. */
interface Embed extends Hide {
  target: string;
  width?: number;
}

/** A callout header badge decoration range. */
interface CalloutBadge extends Hide {
  kind: CalloutKind;
  label?: string;
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
  badges: CalloutBadge[],
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
        case "Blockquote": {
          const firstLine = state.doc.lineAt(from);
          const match = firstLine.text.match(CALLOUT_HEADER);
          if (match) {
            const info = resolveCallout(match[2]);
            addCalloutLines(state, lineClasses, from, to, range, info.kind);
            if (firstLine.from >= range.from && firstLine.from <= range.to) {
              const customTitle = match[3]?.trim();
              if (customTitle) {
                const markerEnd = firstLine.from + match[1].length;
                badges.push({ from: firstLine.from, to: markerEnd, priority: 3, kind: info.kind });
                hides.push({ from: firstLine.from, to: markerEnd, priority: 3 });
                if (markerEnd < firstLine.to) {
                  marks.push({ from: markerEnd, to: firstLine.to, class: "cm-lp-callout-title" });
                }
              } else {
                badges.push({ from: firstLine.from, to: firstLine.to, priority: 3, kind: info.kind, label: info.defaultTitle });
                hides.push({ from: firstLine.from, to: firstLine.to, priority: 3 });
              }
            }
          } else {
            addLineClasses(state, lineClasses, Math.max(from, range.from), Math.min(to, range.to), "cm-lp-quote");
          }
          break;
        }
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
 * Marks the lines of a callout block, first and last extra, so the callout
 * renders as one rounded tinted container.
 */
function addCalloutLines(
  state: EditorState,
  lineClasses: { from: number; class: string }[],
  from: number,
  to: number,
  range: Range,
  kind: CalloutKind,
): void {
  const first = state.doc.lineAt(from).number;
  const last = state.doc.lineAt(Math.max(from, to - 1)).number;
  const start = Math.max(first, state.doc.lineAt(range.from).number);
  const end = Math.min(last, state.doc.lineAt(range.to).number);
  for (let n = start; n <= end; n++) {
    let className = `cm-lp-callout cm-lp-callout-${kind}`;
    if (n === first) className += " cm-lp-callout-first";
    if (n === last) className += " cm-lp-callout-last";
    lineClasses.push({ from: state.doc.line(n).from, class: className });
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

/** Builds the decorations of the live preview for the given ranges. */
function buildDecorations(view: EditorView, ranges: Range[] = visibleLines(view)): DecorationSet {
  const { state } = view;
  const active = activeLines(state);
  const hides: Hide[] = [];
  const bullets: Hide[] = [];
  const rules: Hide[] = [];
  const lineClasses: { from: number; class: string }[] = [];
  const marks: { from: number; to: number; class: string }[] = [];
  const images: Embed[] = [];
  const badges: CalloutBadge[] = [];

  for (const range of ranges) {
    collectWikilinks(state, range, hides, images);
    collectSyntax(state, range, hides, bullets, rules, lineClasses, marks, badges);
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
    const badge = badges.find((b) => b.from === h.from && b.to === h.to);
    const widget = image
      ? new ImageWidget(image.target, image.width, view)
      : badge
        ? new CalloutBadgeWidget(badge.kind, badge.label)
        : undefined;
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

/** Sorts ranges and joins the ones that touch or overlap. */
function mergeRanges(ranges: Range[]): Range[] {
  const sorted = [...ranges].sort((a, b) => a.from - b.from);
  const out: Range[] = [];
  for (const r of sorted) {
    const last = out[out.length - 1];
    if (last && r.from <= last.to + 1) last.to = Math.max(last.to, r.to);
    else out.push({ ...r });
  }
  return out;
}

const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    private view: EditorView | undefined;
    /** The positions the decorations cover; while the scroll settles they lag behind. */
    private covered: Range[] = [];
    private timer: ReturnType<typeof setTimeout> | undefined;
    private onScroll = () => {
      // Every scroll step restarts the wait; the full rebuild happens once
      // the scroll has been quiet for a moment.
      clearTimeout(this.timer);
      this.timer = setTimeout(() => {
        this.timer = undefined;
        // A new decoration set only reaches the view through a transaction;
        // the empty one asks for the redraw without changing anything else.
        this.view?.dispatch({});
      }, SCROLL_SETTLE_MS);
    };

    constructor(view: EditorView) {
      this.view = view;
      this.decorations = buildDecorations(view);
      this.rememberCovered(view);
      view.scrollDOM.addEventListener("scroll", this.onScroll, { passive: true });
    }

    update(update: ViewUpdate) {
      this.view = update.view;
      if (
        update.docChanged ||
        update.selectionSet ||
        syntaxTree(update.startState) !== syntaxTree(update.state) ||
        update.transactions.length > 0
      ) {
        // A real change (typing, cursor, an empty dispatch after the scroll
        // settled) decorates right away, also while a wait is running.
        this.buildNow();
        return;
      }
      if (update.viewportChanged) {
        // The rendered range moved. The lines that came into view are
        // decorated right away, so no raw syntax shows while scrolling; the
        // full rebuild (which also drops what scrolled out and re-measures
        // the heights) waits for the scroll to settle.
        this.coverNewLines();
      }
    }

    /** Adds the decorations of the lines the viewport gained, without touching the rest. */
    private coverNewLines() {
      const view = this.view;
      if (!view) return;
      const { doc } = view.state;
      // The part of the rendered ranges the decorations do not cover yet,
      // widened to whole lines: a marker is hidden per line, never half.
      const fresh: Range[] = [];
      for (const range of visibleLines(view)) {
        let rest: Range[] = [range];
        for (const old of this.covered) {
          const next: Range[] = [];
          for (const r of rest) {
            if (old.to < r.from || old.from > r.to) next.push(r);
            else {
              if (r.from < old.from) next.push({ from: r.from, to: old.from - 1 });
              if (r.to > old.to) next.push({ from: old.to + 1, to: r.to });
            }
          }
          rest = next;
        }
        for (const r of rest) fresh.push({ from: doc.lineAt(r.from).from, to: doc.lineAt(r.to).to });
      }
      if (!fresh.length) return;
      // The new ranges lie outside the covered ones, so the sets can be
      // joined without a conflict; the cursor line keeps its syntax because
      // the new build skips it just like the old one did.
      const added: { from: number; to: number; value: Decoration }[] = [];
      buildDecorations(view, fresh).between(0, view.state.doc.length, (from, to, value) => {
        added.push({ from, to, value });
      });
      this.decorations = this.decorations.update({ add: added });
      // Union, not reset: the old decorations stay while the scroll settles,
      // so what they cover must not be forgotten even though it has left the
      // viewport. Merged, so the list stays as short as the covered stretch.
      this.covered = mergeRanges([...this.covered, ...fresh]);
    }

    /** Rebuilds the decorations for the current state and ends a running wait. */
    private buildNow() {
      clearTimeout(this.timer);
      this.timer = undefined;
      if (!this.view) return;
      this.decorations = buildDecorations(this.view);
      this.rememberCovered(this.view);
    }

    /** Notes which ranges the decorations cover now. */
    private rememberCovered(view: EditorView) {
      this.covered = visibleLines(view);
    }

    destroy() {
      clearTimeout(this.timer);
      this.view?.scrollDOM.removeEventListener("scroll", this.onScroll);
      this.view = undefined;
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
 *
 * The blocks are kept, so a change of the cursor does not have to walk the
 * whole document again, and a block whose text did not change keeps its
 * widget (and with it the state of the editing).
 */
function collectBlocks(state: EditorState, previous: Block[]): Block[] {
  const found: FoundBlock[] = [];
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Document") return true;
      if (node.name === "Frontmatter") {
        // `parseFrontmatter` only takes what it can edit line by line; anything else stays YAML.
        const source = state.doc.sliceString(0, node.to);
        const fm = parseFrontmatter(source);
        if (fm) {
          const text = source.slice(0, fm.to);
          found.push({ from: 0, to: fm.to, kind: "frontmatter", source: text });
        }
      }
      if (node.name === "Table") {
        const from = state.doc.lineAt(node.from).from;
        // `to - 1`: should the node end after its line break, the next line is not part of it.
        const to = state.doc.lineAt(Math.max(node.from, node.to - 1)).to;
        const text = state.doc.sliceString(from, to);
        found.push({ from, to, kind: "table", source: text });
      }
      return false;
    },
  });
  // A block whose text did not change keeps its widget, and with it the
  // state of the editing.
  return found.map((block, i) => {
    const before = previous[i];
    const widget =
      before && before.kind === block.kind && before.source === block.source
        ? before.widget
        : block.kind === "frontmatter"
          ? new PropertiesWidget(block.source)
          : new TableWidget(block.source);
    return { ...block, widget };
  });
}

/**
 * The decorations for the blocks. A block is not drawn while the editor's
 * cursor is touching it, even at its edge: then its source shows.
 */
function drawBlocks(all: Block[], state: EditorState): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const sel = state.selection.main;
  for (const block of all) {
    if (sel.to < block.from || sel.from > block.to) {
      builder.add(block.from, block.to, Decoration.replace({ widget: block.widget, block: true }));
    }
  }
  return builder.finish();
}

/** A block of the document that is drawn as a widget: the properties or a table. */
interface Block {
  from: number;
  to: number;
  /** What kind of block it is: the properties or a table. */
  kind: "frontmatter" | "table";
  /** The text the block shows. */
  source: string;
  /** The widget that draws the block. */
  widget: WidgetType;
}

/** A block that was found in the document; its widget comes later. */
type FoundBlock = Omit<Block, "widget">;

/** The blocks of the document and the decorations that draw them. */
interface Blocks {
  /** Every block of the document, in the order they appear. */
  all: Block[];
  /** The blocks that are drawn right now. */
  decorations: DecorationSet;
}

const blocks = StateField.define<Blocks>({
  create: (state) => {
    const all = collectBlocks(state, []);
    return { all, decorations: drawBlocks(all, state) };
  },
  update(value, tr) {
    if (tr.docChanged || syntaxTree(tr.startState) !== syntaxTree(tr.state)) {
      const all = collectBlocks(tr.state, value.all);
      return { all, decorations: drawBlocks(all, tr.state) };
    }
    if (tr.selection) {
      // The blocks themselves did not change; only which ones are drawn.
      return { all: value.all, decorations: drawBlocks(value.all, tr.state) };
    }
    return value;
  },
  provide: (field) => EditorView.decorations.compute([field], (state) => state.field(field).decorations),
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
    state.field(blocks).decorations.between(Math.min(range.head, target.head), Math.max(range.head, target.head), (from, to, deco) => {
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
