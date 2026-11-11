// A markdown table in the live preview that stays a table while it is
// edited, like in Obsidian: every cell can be typed in directly. A focused
// cell shows its markdown, the others show it rendered. Each key press
// changes only the text of that cell in the note (see `markdownTable.ts`),
// as a normal editor change, so undo and saving work as for typing.
// Several cells can be selected (drag, Shift + click, Shift + arrows, grips
// for whole rows and columns) to clear, copy, paste or format them, and
// rows, columns and blocks of cells can be moved by dragging.
import { redo, undo } from "@codemirror/commands";
import { EditorView, WidgetType } from "@codemirror/view";
import { contextMenu, type MenuEntry } from "../../components/contextMenuState.svelte";
import {
  alignOf,
  deleteColumn,
  escapeCell,
  fromTSV,
  insertColumn,
  insertRow,
  moveBlock,
  moveColumns,
  moveRows,
  parseRow,
  parseTable,
  setAlign,
  setCell,
  setCells,
  toggleMark,
  toTSV,
  type CellUpdate,
  type Change,
  type Rect,
  type Row,
  type Table,
} from "./markdownTable";
import { openLink } from "./propertiesWidget.svelte";

/** Inline markdown inside a cell: code, bold, italic, strikethrough, links. `_` only counts outside of words, so `snake_case` stays text. */
const INLINE =
  /`([^`]+)`|\*\*(.+?)\*\*|(?<!\w)__(.+?)__(?!\w)|~~(.+?)~~|\*(.+?)\*|(?<!\w)_(.+?)_(?!\w)|\[\[([^\]]+)\]\]|\[([^\]]*)\]\([^)]*\)|\\(\|)/g;

/**
 * Fills a cell with its inline markdown. Built from DOM nodes instead of
 * HTML, so text from the note can never become markup.
 */
function renderInline(text: string, parent: HTMLElement): void {
  let last = 0;
  for (const m of text.matchAll(INLINE)) {
    if (m.index! > last) parent.append(text.slice(last, m.index));
    last = m.index! + m[0].length;
    const [, code, bold, bold2, strike, em, em2, wikilink, link, pipe] = m;
    let el: HTMLElement | string;
    if (pipe !== undefined) {
      el = "|";
    } else if (code !== undefined) {
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
      const bar = wikilink.indexOf("|");
      el.dataset.target = (bar >= 0 ? wikilink.slice(0, bar) : wikilink).split("#")[0].trim();
      el.textContent = bar >= 0 ? wikilink.slice(bar + 1) : wikilink.split("#")[0];
    } else {
      el = document.createElement("span");
      el.className = "cm-lp-link";
      renderInline(link, el);
    }
    parent.append(el);
  }
  if (last < text.length) parent.append(text.slice(last));
}

/** Makes an element editable as plain text, where the webview supports it. */
function makeEditable(el: HTMLElement): void {
  try {
    el.contentEditable = "plaintext-only";
  } catch {
    el.contentEditable = "true";
  }
}

/** Where the caret is in a cell: at its start, its end, or both when it is empty. */
function caretAt(el: HTMLElement): { start: boolean; end: boolean } {
  const sel = el.ownerDocument.getSelection();
  if (!sel || !sel.isCollapsed || sel.rangeCount === 0) return { start: false, end: false };
  const range = sel.getRangeAt(0);
  const before = range.cloneRange();
  before.selectNodeContents(el);
  before.setEnd(range.startContainer, range.startOffset);
  const offset = before.toString().length;
  return { start: offset === 0, end: offset === (el.textContent ?? "").length };
}

function placeCaret(el: HTMLElement, atEnd: boolean): void {
  const sel = el.ownerDocument.getSelection();
  if (!sel) return;
  const range = el.ownerDocument.createRange();
  range.selectNodeContents(el);
  range.collapse(!atEnd);
  sel.removeAllRanges();
  sel.addRange(range);
}

const editors = new WeakMap<HTMLElement, TableEditor>();

/** A selection of several cells: where it was started and where it ends now. */
interface Selection {
  ar: number;
  ac: number;
  hr: number;
  hc: number;
}

/** What a mouse drag that has started is doing. */
type Drag =
  | { kind: "select"; r: number; c: number; x: number; y: number; active: boolean }
  | { kind: "cells"; rect: Rect; r: number; c: number; x: number; y: number; active: boolean; target: [number, number] | null }
  | { kind: "rows"; from: number; to: number; y: number; active: boolean; click: number; marked?: number }
  | { kind: "cols"; from: number; to: number; x: number; active: boolean; click: number; marked?: number };

/** How far the mouse has to move before a press becomes a drag. */
const DRAG_DISTANCE = 4;

const rectOf = (s: Selection): Rect => ({
  r0: Math.min(s.ar, s.hr),
  c0: Math.min(s.ac, s.hc),
  r1: Math.max(s.ar, s.hr),
  c1: Math.max(s.ac, s.hc),
});

const inRect = (rect: Rect, r: number, c: number) => r >= rect.r0 && r <= rect.r1 && c >= rect.c0 && c <= rect.c1;

/** Text of the cells for other programs: a plain HTML table, escaped. */
function toHTML(texts: string[][]): string {
  const esc = (t: string) => t.replace(/\\\|/g, "|").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return `<table>${texts.map((row) => `<tr>${row.map((t) => `<td>${esc(t)}</td>`).join("")}</tr>`).join("")}</table>`;
}

/** The common start and end of two texts, so a change replaces only what differs. */
function diff(a: string, b: string): Change | null {
  if (a === b) return null;
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  return { from: start, to: endA, insert: b.slice(start, endB) };
}

/** Applies changes (relative to one text, not overlapping) to that text. */
function applyChanges(text: string, changes: Change[]): string {
  for (const ch of [...changes].sort((a, b) => b.from - a.from)) text = text.slice(0, ch.from) + ch.insert + text.slice(ch.to);
  return text;
}

/**
 * The table for a new text, with the rows whose line did not change kept as
 * they are: growing a table by pasting or adding rows re-reads only the new
 * or changed lines, not the whole table for every step. A row's cell
 * positions are relative to its line, so an unchanged line keeps its cells
 * even when it moved; only `start` follows the new position.
 */
function reparse(table: Table, text: string): Table {
  const old = allRows(table);
  const rows: Row[] = [];
  let delimiter: Row | null = null;
  let start = 0;
  text.split("\n").forEach((line, i) => {
    const before = old[i];
    // A line that did not change keeps its row object (and with it its drawn
    // cells); only a moved line gets a new object with the new start.
    const row = before?.text === line ? (before.start === start ? before : { ...before, start }) : parseRow(line, start);
    if (i === 1) delimiter = row;
    else rows.push(row);
    start += line.length + 1;
  });
  delimiter ??= parseRow("", start);
  const aligns = delimiter.text === table.delimiter.text ? table.aligns : delimiter.cells.map((c) => alignOf(c.text));
  return { rows, delimiter, aligns, columns: rows[0]?.cells.length ?? 0 };
}

/** Header, delimiter and body lines of a table, in the order they stand in. */
function allRows(table: Table): Row[] {
  return [table.rows[0], table.delimiter, ...table.rows.slice(1)].filter((r): r is Row => !!r);
}

class TableEditor {
  readonly dom: HTMLDivElement;
  private table: Table;
  private cells: HTMLElement[][] = [];
  private rowGrips: HTMLElement[] = [];
  private colGrips: HTMLElement[] = [];
  /** Several cells selected; the keyboard then goes to `clip`. */
  private sel: Selection | null = null;
  /**
   * Holds the keyboard focus while several cells are selected. Copy, cut
   * and paste only reach elements that can be typed in, a selection of
   * cells is none, so this hidden field stands in for it.
   */
  private clip: HTMLTextAreaElement;
  private drag: Drag | null = null;

  constructor(
    private view: EditorView,
    private source: string,
  ) {
    this.table = parseTable(source);
    this.dom = document.createElement("div");
    this.dom.className = "cm-lp-table-wrap";
    this.clip = document.createElement("textarea");
    this.clip.className = "cm-lp-table-clip";
    this.clip.setAttribute("aria-label", "Selected cells");
    this.clip.tabIndex = -1;
    this.dom.addEventListener("focusin", (e) => this.onFocusIn(e));
    this.dom.addEventListener("focusout", (e) => this.onFocusOut(e));
    this.dom.addEventListener("input", (e) => this.onInput(e));
    this.dom.addEventListener("keydown", (e) => this.onKeyDown(e));
    this.dom.addEventListener("paste", (e) => this.onPaste(e));
    this.dom.addEventListener("copy", (e) => this.onCopy(e, false));
    this.dom.addEventListener("cut", (e) => this.onCopy(e, true));
    this.dom.addEventListener("mousedown", (e) => this.onMouseDown(e));
    this.dom.addEventListener("contextmenu", (e) => this.onContextMenu(e));
    this.build();
    editors.set(this.dom, this);
  }

  private raw(r: number, c: number): string {
    return this.table.rows[r]?.cells[c]?.text ?? "";
  }

  get lastRow(): number {
    return this.table.rows.length - 1;
  }

  /** Draws the whole table again, e.g. after a row or column was added. */
  private build(): void {
    const { table } = this;
    const el = document.createElement("table");
    el.className = "cm-lp-table";
    const thead = document.createElement("thead");
    const tbody = document.createElement("tbody");

    // A row of grips above the columns and one grip in front of every row:
    // a click selects the column or row, dragging moves it.
    const gripRow = document.createElement("tr");
    gripRow.className = "cm-lp-grips";
    gripRow.append(document.createElement("th"));
    this.colGrips = [];
    for (let c = 0; c < table.columns; c++) {
      const grip = document.createElement("th");
      grip.className = "cm-lp-col-grip";
      grip.dataset.col = String(c);
      grip.title = "Select column, drag to move";
      gripRow.append(grip);
      this.colGrips.push(grip);
    }
    thead.append(gripRow);

    this.rowGrips = [];
    this.cells = table.rows.map((_, r) => {
      const built = this.buildRow(r);
      (r === 0 ? thead : tbody).append(built.tr);
      this.rowGrips.push(built.grip);
      return built.cells;
    });
    el.append(thead, tbody);

    const addRow = document.createElement("button");
    addRow.className = "cm-lp-table-add cm-lp-table-add-row";
    addRow.title = "Add row";
    addRow.setAttribute("aria-label", "Add row");
    addRow.textContent = "+";
    addRow.addEventListener("click", () => this.addRow(this.table.rows.length, 0));
    const addCol = document.createElement("button");
    addCol.className = "cm-lp-table-add cm-lp-table-add-col";
    addCol.title = "Add column";
    addCol.setAttribute("aria-label", "Add column");
    addCol.textContent = "+";
    addCol.addEventListener("click", () => this.addColumn(this.table.columns, 0));

    const grid = document.createElement("div");
    grid.className = "cm-lp-table-grid";
    grid.append(el, addCol, addRow);
    this.dom.replaceChildren(grid, this.clip);
    this.paintSelection();
  }

  /**
   * A cell being typed in shows its markdown, the others show it rendered.
   * Which one that is, is kept on the cell itself: focus events do not come
   * while the window is in the background, so they cannot be relied on.
   */
  private showSource(el: HTMLElement): void {
    if (el.dataset.editing) return;
    for (const other of this.cells.flat()) if (other !== el && other.dataset.editing) this.showRendered(other);
    el.dataset.editing = "1";
    el.textContent = this.raw(Number(el.dataset.row), Number(el.dataset.col));
  }

  private showRendered(el: HTMLElement): void {
    delete el.dataset.editing;
    el.replaceChildren();
    renderInline(this.raw(Number(el.dataset.row), Number(el.dataset.col)), el);
  }

  private editingCell(): { r: number; c: number } | null {
    for (const el of this.cells.flat()) {
      if (el.dataset.editing) return { r: Number(el.dataset.row), c: Number(el.dataset.col) };
    }
    return null;
  }

  /** The note changed (typing here, undo, a change from outside): show the new text. */
  update(source: string): void {
    if (source === this.source) return;
    this.source = source;
    const old = this.table;
    this.table = reparse(old, source);
    const sameShape =
      old.rows.length === this.table.rows.length &&
      old.columns === this.table.columns &&
      old.aligns.join() === this.table.aligns.join();
    if (!sameShape) {
      const keep = this.editingCell();
      if (this.sel) {
        // The selection stays, cut to the new size.
        const clamp = (v: number, max: number) => Math.max(0, Math.min(v, max));
        const { ar, ac, hr, hc } = this.sel;
        const cols = this.table.columns - 1;
        this.sel = { ar: clamp(ar, this.lastRow), ac: clamp(ac, cols), hr: clamp(hr, this.lastRow), hc: clamp(hc, cols) };
      }
      // Rows whose line did not change keep their DOM (and the cell being
      // typed in): only new rows are built and old ones thrown away.
      this.syncRows(old);
      if (keep && !this.sel) this.focusCell(Math.min(keep.r, this.lastRow), Math.min(keep.c, this.table.columns - 1), true);
      if (this.sel) this.clip.focus({ preventScroll: true });
      return;
    }
    this.table.rows.forEach((_, r) => {
      for (let c = 0; c < this.table.columns; c++) {
        const cell = this.cells[r][c];
        const text = this.raw(r, c);
        if (cell.dataset.editing) {
          // Only differs after undo/redo or an outside change; typing keeps them equal.
          if (cell.textContent !== text) {
            cell.textContent = text;
            placeCaret(cell, true);
          }
        } else if (text !== old.rows[r]?.cells[c]?.text) {
          cell.replaceChildren();
          renderInline(text, cell);
        }
      }
    });
    if (this.sel) this.fillClip();
  }

  /**
   * Brings the drawn table in line with `this.table` after rows were added,
   * removed or moved: body rows whose cells did not change keep their DOM
   * (and the cell being typed in), the rest are built anew. Columns or
   * alignments that changed mean every row differs, then `build` draws all.
   * Only called from `update`, where the DOM still matches `old` exactly.
   */
  private syncRows(old: Table): void {
    const { table } = this;
    const tableEl = this.dom.querySelector("table");
    if (
      !tableEl ||
      old.columns !== table.columns ||
      old.aligns.join() !== table.aligns.join() ||
      old.rows[0]?.text !== table.rows[0]?.text
    ) {
      this.build();
      return;
    }
    const tbody = tableEl.tBodies[0];
    const sameRow = (r: number) =>
      r > 0 && r < old.rows.length && r < table.rows.length && old.rows[r].cells.length === table.rows[r].cells.length &&
      table.rows[r].cells.every((cell, c) => cell.text === old.rows[r].cells[c]?.text);
    // The old body rows that can be kept, by the row they were drawn for.
    const oldBody = new Map<number, { tr: HTMLTableRowElement; cells: HTMLElement[]; grip: HTMLElement }>();
    [...tbody.rows].forEach((tr, i) => {
      const r = i + 1;
      if (sameRow(r)) oldBody.set(r, { tr, cells: this.cells[r], grip: this.rowGrips[r] });
    });
    const cells: HTMLElement[][] = [this.cells[0]];
    const grips: HTMLElement[] = [this.rowGrips[0]];
    const bodyRows: HTMLTableRowElement[] = [];
    for (let r = 1; r < table.rows.length; r++) {
      const reuse = oldBody.get(r);
      const built = reuse ?? this.buildRow(r);
      built.cells.forEach((el, c) => {
        el.dataset.row = String(r);
        el.dataset.col = String(c);
      });
      built.grip.dataset.row = String(r);
      cells.push(built.cells);
      grips.push(built.grip);
      bodyRows.push(built.tr);
    }
    tbody.replaceChildren(...bodyRows);
    this.cells = cells;
    this.rowGrips = grips;
    this.paintSelection();
  }

  /** One drawn row: its grip and its cells, the way `build` makes them. */
  private buildRow(r: number): { tr: HTMLTableRowElement; cells: HTMLElement[]; grip: HTMLElement } {
    const { table } = this;
    const tr = document.createElement("tr");
    const grip = document.createElement("td");
    grip.className = "cm-lp-row-grip";
    grip.dataset.row = String(r);
    grip.title = r === 0 ? "Select header row" : "Select row, drag to move";
    tr.append(grip);
    const row: HTMLElement[] = [];
    // Like GitHub: every row has as many cells as the header.
    for (let c = 0; c < table.columns; c++) {
      const td = document.createElement(r === 0 ? "th" : "td");
      const align = table.aligns[c];
      if (align) td.style.textAlign = align;
      const cell = document.createElement("div");
      cell.className = "cm-lp-cell";
      cell.dataset.row = String(r);
      cell.dataset.col = String(c);
      makeEditable(cell);
      cell.spellcheck = false;
      renderInline(this.raw(r, c), cell);
      td.append(cell);
      tr.append(td);
      row.push(cell);
    }
    return { tr, cells: row, grip };
  }

  private cellOf(target: EventTarget | null): { el: HTMLElement; r: number; c: number } | null {
    const el = (target as HTMLElement | null)?.closest?.<HTMLElement>(".cm-lp-cell");
    if (!el || !this.dom.contains(el)) return null;
    return { el, r: Number(el.dataset.row), c: Number(el.dataset.col) };
  }

  /** The cell under the mouse, also when it is over the border or padding of a cell. */
  private cellAt(x: number, y: number): { r: number; c: number } | null {
    const hit = document.elementFromPoint(x, y) as HTMLElement | null;
    const td = hit?.closest?.("td, th");
    const el = td?.querySelector<HTMLElement>(".cm-lp-cell");
    if (!el || !this.dom.contains(el)) return null;
    return { r: Number(el.dataset.row), c: Number(el.dataset.col) };
  }

  // --- Selection of several cells ---------------------------------------

  private select(sel: Selection): void {
    const editing = this.cells.flat().find((el) => el.dataset.editing);
    if (editing) this.showRendered(editing);
    this.sel = sel;
    this.paintSelection();
    this.fillClip();
    this.clip.focus({ preventScroll: true });
    this.clip.select();
  }

  private clearSelection(): void {
    if (!this.sel) return;
    this.sel = null;
    this.paintSelection();
  }

  private paintSelection(): void {
    const rect = this.sel ? rectOf(this.sel) : null;
    this.cells.forEach((row, r) =>
      row.forEach((el, c) => el.parentElement?.classList.toggle("cm-lp-selected", !!rect && inRect(rect, r, c))),
    );
    const fullRows = rect && rect.c0 === 0 && rect.c1 === this.table.columns - 1;
    const fullCols = rect && rect.r0 === 0 && rect.r1 === this.lastRow;
    this.rowGrips.forEach((g, r) => g.classList.toggle("cm-lp-grip-on", !!fullRows && r >= rect.r0 && r <= rect.r1));
    this.colGrips.forEach((g, c) => g.classList.toggle("cm-lp-grip-on", !!fullCols && c >= rect.c0 && c <= rect.c1));
  }

  /** The selected cells as text in the hidden field, so even the system's own copy has them. */
  private fillClip(): void {
    if (!this.sel) return;
    this.clip.value = toTSV(this.selectedTexts());
    this.clip.select();
  }

  private selectedTexts(): string[][] {
    const rect = rectOf(this.sel!);
    const out: string[][] = [];
    for (let r = rect.r0; r <= rect.r1; r++) {
      const row: string[] = [];
      for (let c = rect.c0; c <= rect.c1; c++) row.push(this.raw(r, c));
      out.push(row);
    }
    return out;
  }

  private selectedUpdates(fn: (text: string, r: number, c: number) => string): CellUpdate[] {
    const rect = rectOf(this.sel!);
    const out: CellUpdate[] = [];
    for (let r = rect.r0; r <= rect.r1; r++) for (let c = rect.c0; c <= rect.c1; c++) out.push({ r, c, text: fn(this.raw(r, c), r, c) });
    return out;
  }

  private clearSelected(): void {
    this.apply(setCells(this.table, this.selectedUpdates(() => "")));
  }

  private formatSelected(mark: "**" | "*"): void {
    const updates = this.selectedUpdates((t) => t);
    const texts = toggleMark(
      updates.map((u) => u.text),
      mark,
    );
    this.apply(setCells(this.table, updates.map((u, i) => ({ ...u, text: texts[i] }))));
  }

  /** Keys while several cells are selected (the focus is in `clip` then). */
  private onSelectionKey(event: KeyboardEvent): void {
    const sel = this.sel!;
    const mod = event.ctrlKey || event.metaKey;
    const key = event.key;
    const done = () => {
      event.preventDefault();
      event.stopPropagation();
    };
    const moves: Record<string, [number, number]> = {
      ArrowUp: [-1, 0],
      ArrowDown: [1, 0],
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1],
    };
    if (key in moves) {
      done();
      const [dr, dc] = moves[key];
      const r = Math.max(0, Math.min(sel.hr + dr, this.lastRow));
      const c = Math.max(0, Math.min(sel.hc + dc, this.table.columns - 1));
      if (event.shiftKey) this.select({ ...sel, hr: r, hc: c });
      else {
        this.clearSelection();
        this.focusCell(r, c, true);
      }
      return;
    }
    if (mod && !event.altKey) {
      const k = key.toLowerCase();
      if (k === "z" || k === "y") {
        done();
        if (k === "y" || event.shiftKey) redo(this.view);
        else undo(this.view);
      } else if (k === "b") {
        done();
        this.formatSelected("**");
      } else if (k === "i") {
        done();
        this.formatSelected("*");
      } else if (k === "a") {
        done();
        this.select({ ar: 0, ac: 0, hr: this.lastRow, hc: this.table.columns - 1 });
      }
      // Ctrl+C, X, V: the browser fires copy, cut and paste on the hidden field.
      return;
    }
    switch (key) {
      case "Delete":
      case "Backspace":
        done();
        this.clearSelected();
        return;
      case "Escape":
      case "Enter":
        done();
        this.clearSelection();
        this.focusCell(sel.hr, sel.hc, true);
        return;
      case "Tab":
        done();
        this.clearSelection();
        this.focusCell(sel.hr, sel.hc, true);
        return;
    }
    // Typing does nothing here, it must not end up in the hidden field.
    if (key.length === 1) done();
  }

  private onCopy(event: ClipboardEvent, cut: boolean): void {
    if (!this.sel || event.target !== this.clip) return;
    event.preventDefault();
    const texts = this.selectedTexts();
    event.clipboardData?.setData("text/plain", toTSV(texts));
    event.clipboardData?.setData("text/html", toHTML(texts));
    if (cut) this.clearSelected();
  }

  /**
   * Pastes a block of cells (tab-separated, the way spreadsheets copy) from
   * the selection's or the edited cell's top left on. Rows and columns are
   * added when the block is bigger than the room left. A single value fills
   * every selected cell.
   */
  private pasteBlock(block: string[][], r0: number, c0: number, fill: Rect | null): void {
    let text = this.source;
    let table = this.table;
    if (fill && block.length === 1 && block[0].length === 1) {
      const updates: CellUpdate[] = [];
      for (let r = fill.r0; r <= fill.r1; r++) for (let c = fill.c0; c <= fill.c1; c++) updates.push({ r, c, text: block[0][0] });
      this.apply(setCells(table, updates));
      return;
    }
    const width = Math.max(...block.map((row) => row.length));
    while (table.columns < c0 + width) {
      const changes = insertColumn(table, table.columns);
      text = applyChanges(text, changes);
      table = reparse(table, text);
    }
    while (table.rows.length < r0 + block.length) {
      const changes = [insertRow(table, table.rows.length)];
      text = applyChanges(text, changes);
      table = reparse(table, text);
    }
    const updates = block.flatMap((row, dr) => row.map((t, dc) => ({ r: r0 + dr, c: c0 + dc, text: t })));
    text = applyChanges(text, setCells(table, updates));
    this.applyText(text);
    this.select({ ar: r0, ac: c0, hr: r0 + block.length - 1, hc: c0 + width - 1 });
  }

  // --- Events -------------------------------------------------------------

  private onFocusIn(event: FocusEvent): void {
    const cell = this.cellOf(event.target);
    if (cell) {
      this.clearSelection();
      this.showSource(cell.el);
    }
  }

  private onFocusOut(event: FocusEvent): void {
    const cell = this.cellOf(event.target);
    if (cell) this.showRendered(cell.el);
    // Clicked somewhere else: the selection goes, like a text selection would.
    if (event.target === this.clip && !this.dom.contains(event.relatedTarget as Node | null) && !this.drag) {
      this.clearSelection();
    }
  }

  private onInput(event: Event): void {
    if (event.target === this.clip) {
      this.fillClip();
      return;
    }
    const cell = this.cellOf(event.target);
    if (!cell) return;
    // Typed into means edited, even if no focus event said so.
    cell.el.dataset.editing = "1";
    // Escaped like a paste, in case a pipe got in some other way (e.g. by drag and drop).
    const text = escapeCell(cell.el.textContent ?? "").trim();
    if (text === this.raw(cell.r, cell.c)) return;
    this.apply(setCell(this.table, cell.r, cell.c, text));
  }

  private onPaste(event: ClipboardEvent): void {
    const text = event.clipboardData?.getData("text/plain") ?? "";
    if (event.target === this.clip && this.sel) {
      event.preventDefault();
      const rect = rectOf(this.sel);
      this.pasteBlock(fromTSV(text), rect.r0, rect.c0, rect);
      return;
    }
    const cell = this.cellOf(event.target);
    if (!cell) return;
    event.preventDefault();
    // Several cells (with tabs or lines) fill the table from here on, one value goes into this cell.
    if (/[\t\n]/.test(text.replace(/\r?\n$/, ""))) this.pasteBlock(fromTSV(text), cell.r, cell.c, null);
    else document.execCommand("insertText", false, escapeCell(text));
  }

  private onMouseDown(event: MouseEvent): void {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement;
    // Ctrl/Cmd + click on a [[link]] in a cell opens it, like in the text.
    const link = target.closest<HTMLElement>(".cm-wikilink");
    if (link?.dataset.target && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      this.view.state.facet(openLink)?.(link.dataset.target);
      return;
    }

    const rowGrip = target.closest<HTMLElement>(".cm-lp-row-grip");
    const colGrip = target.closest<HTMLElement>(".cm-lp-col-grip");
    if (rowGrip || colGrip) {
      event.preventDefault();
      const rect = this.sel ? rectOf(this.sel) : null;
      if (rowGrip) {
        const r = Number(rowGrip.dataset.row);
        const whole = rect && rect.c0 === 0 && rect.c1 === this.table.columns - 1 && r >= rect.r0 && r <= rect.r1;
        const [from, to] = whole ? [rect.r0, rect.r1] : [r, r];
        this.startDrag({ kind: "rows", from, to, y: event.clientY, active: false, click: r });
      } else {
        const c = Number(colGrip!.dataset.col);
        const whole = rect && rect.r0 === 0 && rect.r1 === this.lastRow && c >= rect.c0 && c <= rect.c1;
        const [from, to] = whole ? [rect.c0, rect.c1] : [c, c];
        this.startDrag({ kind: "cols", from, to, x: event.clientX, active: false, click: c });
      }
      return;
    }

    const hit = this.cellAt(event.clientX, event.clientY);
    if (!hit) return;
    if (event.shiftKey) {
      // Shift + click: from the cell being edited (or the selection's start) to here.
      event.preventDefault();
      const from = this.sel ? { r: this.sel.ar, c: this.sel.ac } : (this.editingCell() ?? this.cellOf(document.activeElement) ?? hit);
      this.select({ ar: from.r, ac: from.c, hr: hit.r, hc: hit.c });
      return;
    }
    const rect = this.sel ? rectOf(this.sel) : null;
    if (rect && inRect(rect, hit.r, hit.c) && (rect.r0 !== rect.r1 || rect.c0 !== rect.c1)) {
      // Pressing on the selection: dragging moves its contents, a click edits the cell.
      event.preventDefault();
      this.startDrag({ kind: "cells", rect, ...hit, x: event.clientX, y: event.clientY, active: false, target: null });
      return;
    }
    // A press in a cell: a click edits it, dragging over other cells selects them.
    this.startDrag({ kind: "select", ...hit, x: event.clientX, y: event.clientY, active: false });
  }

  private startDrag(drag: Drag): void {
    this.drag = drag;
    const move = (e: MouseEvent) => this.onDragMove(e);
    const up = (e: MouseEvent) => {
      document.removeEventListener("mousemove", move, true);
      document.removeEventListener("mouseup", up, true);
      this.onDragEnd(e);
    };
    document.addEventListener("mousemove", move, true);
    document.addEventListener("mouseup", up, true);
  }

  /**
   * The row a drop at `y` lands before (`rows.length`: past the last row).
   * The grips are measured as they stand now, not once at the drag's start:
   * the table can scroll during a drag, and cached positions would then name
   * a row the scrolled distance away from the mouse. The scan stops at the
   * first row below `y` and the drop marks are only shadows, so the layout
   * stays clean and this is cheap on every mouse move.
   */
  private rowBefore(y: number): number {
    for (let r = 1; r < this.table.rows.length; r++) {
      const box = this.rowGrips[r].getBoundingClientRect();
      if (y < box.top + box.height / 2) return r;
    }
    return this.table.rows.length;
  }

  /** The column a drop at `x` lands before (`columns`: past the last column), measured like `rowBefore`. */
  private colBefore(x: number): number {
    for (let c = 0; c < this.table.columns; c++) {
      const box = this.colGrips[c].getBoundingClientRect();
      if (x < box.left + box.width / 2) return c;
    }
    return this.table.columns;
  }

  private onDragMove(event: MouseEvent): void {
    const drag = this.drag;
    if (!drag) return;
    const far = (x: number, y: number) => Math.hypot(event.clientX - x, event.clientY - y) >= DRAG_DISTANCE;
    switch (drag.kind) {
      case "select": {
        const hit = this.cellAt(event.clientX, event.clientY);
        if (!drag.active && (!hit || (hit.r === drag.r && hit.c === drag.c))) return;
        drag.active = true;
        event.preventDefault();
        this.dom.classList.add("cm-lp-dragging");
        if (hit) this.select({ ar: drag.r, ac: drag.c, hr: hit.r, hc: hit.c });
        return;
      }
      case "cells": {
        if (!drag.active && !far(drag.x, drag.y)) return;
        drag.active = true;
        event.preventDefault();
        this.dom.classList.add("cm-lp-dragging");
        const hit = this.cellAt(event.clientX, event.clientY);
        if (!hit) return;
        // The cell under the mouse keeps its place within the moved block.
        const h = drag.rect.r1 - drag.rect.r0;
        const w = drag.rect.c1 - drag.rect.c0;
        const r = Math.max(0, Math.min(hit.r - (drag.r - drag.rect.r0), this.lastRow - h));
        const c = Math.max(0, Math.min(hit.c - (drag.c - drag.rect.c0), this.table.columns - 1 - w));
        // The drop place did not move: the marks stay, no style is touched.
        if (drag.target && drag.target[0] === r && drag.target[1] === c) return;
        drag.target = [r, c];
        this.markDrop(".cm-lp-drop-target", (rr, cc) => rr >= r && rr <= r + h && cc >= c && cc <= c + w);
        return;
      }
      case "rows": {
        if (!drag.active && Math.abs(event.clientY - drag.y) < DRAG_DISTANCE) return;
        if (drag.click === 0) return; // The header stays on top.
        drag.active = true;
        event.preventDefault();
        this.dom.classList.add("cm-lp-dragging");
        const before = this.rowBefore(event.clientY);
        // The drop place did not move: the marks stay, no style is touched.
        if (drag.marked === before) return;
        drag.marked = before;
        this.clearDropMarks();
        const ok = !(before >= drag.from && before <= drag.to + 1);
        if (ok) {
          if (before < this.table.rows.length) this.rowGrips[before].parentElement!.classList.add("cm-lp-drop-above");
          else this.rowGrips[this.lastRow].parentElement!.classList.add("cm-lp-drop-below");
        }
        return;
      }
      case "cols": {
        if (!drag.active && Math.abs(event.clientX - drag.x) < DRAG_DISTANCE) return;
        drag.active = true;
        event.preventDefault();
        this.dom.classList.add("cm-lp-dragging");
        const before = this.colBefore(event.clientX);
        // The drop place did not move: the marks stay, no style is touched.
        if (drag.marked === before) return;
        drag.marked = before;
        this.clearDropMarks();
        if (!(before >= drag.from && before <= drag.to + 1)) {
          const last = before === this.table.columns;
          const col = last ? this.table.columns - 1 : before;
          for (const row of this.cells) row[col]?.parentElement?.classList.add(last ? "cm-lp-drop-right" : "cm-lp-drop-left");
        }
        return;
      }
    }
  }

  private markDrop(className: string, test: (r: number, c: number) => boolean): void {
    this.clearDropMarks();
    this.cells.forEach((row, r) => row.forEach((el, c) => test(r, c) && el.parentElement?.classList.add(className.slice(1))));
  }

  private clearDropMarks(): void {
    for (const el of this.dom.querySelectorAll(".cm-lp-drop-target, .cm-lp-drop-above, .cm-lp-drop-below, .cm-lp-drop-left, .cm-lp-drop-right")) {
      el.classList.remove("cm-lp-drop-target", "cm-lp-drop-above", "cm-lp-drop-below", "cm-lp-drop-left", "cm-lp-drop-right");
    }
  }

  private onDragEnd(event: MouseEvent): void {
    const drag = this.drag;
    this.drag = null;
    this.dom.classList.remove("cm-lp-dragging");
    this.clearDropMarks();
    if (!drag) return;
    switch (drag.kind) {
      case "select":
        // A plain click: the cell got the focus by itself.
        return;
      case "cells": {
        if (!drag.active) {
          this.clearSelection();
          this.focusCell(drag.r, drag.c, true);
          return;
        }
        if (!drag.target) return;
        const [r, c] = drag.target;
        this.apply(setCells(this.table, moveBlock(this.table, drag.rect, r, c)));
        this.select({ ar: r, ac: c, hr: r + drag.rect.r1 - drag.rect.r0, hc: c + drag.rect.c1 - drag.rect.c0 });
        return;
      }
      case "rows": {
        if (!drag.active) {
          // A click on a grip selects the row; with Shift up to the rows selected before.
          const r = drag.click;
          const from = event.shiftKey && this.sel ? this.sel.ar : r;
          this.select({ ar: from, ac: 0, hr: r, hc: this.table.columns - 1 });
          return;
        }
        // Measured again where the mouse is released: the table may have
        // scrolled since the last move, and the drop follows the view, not
        // the place the mark was drawn at.
        const before = this.rowBefore(event.clientY);
        const change = moveRows(this.table, drag.from, drag.to, before);
        if (!change) return;
        const count = drag.to - drag.from;
        const start = before < drag.from ? before : before - count - 1;
        this.apply(change);
        this.select({ ar: start, ac: 0, hr: start + count, hc: this.table.columns - 1 });
        return;
      }
      case "cols": {
        if (!drag.active) {
          const c = drag.click;
          const from = event.shiftKey && this.sel ? this.sel.ac : c;
          this.select({ ar: 0, ac: from, hr: this.lastRow, hc: c });
          return;
        }
        // Measured again at the release point, like the rows above.
        const before = this.colBefore(event.clientX);
        const changes = moveColumns(this.table, drag.from, drag.to, before);
        if (!changes.length) return;
        const count = drag.to - drag.from;
        const start = before < drag.from ? before : before - count - 1;
        this.apply(changes);
        this.select({ ar: 0, ac: start, hr: this.lastRow, hc: start + count });
        return;
      }
    }
  }

  private onKeyDown(event: KeyboardEvent): void {
    if (event.target === this.clip) {
      if (this.sel) this.onSelectionKey(event);
      return;
    }
    const cell = this.cellOf(event.target);
    if (!cell) return;
    const { r, c, el } = cell;
    const mod = event.ctrlKey || event.metaKey;
    const key = event.key;
    const done = () => {
      event.preventDefault();
      event.stopPropagation();
    };

    // Before the modifier check: on Windows AltGr (for `|` on German keyboards) counts as Ctrl+Alt.
    if (key === "|") {
      done();
      // A typed pipe would split the cell; written escaped it stays text.
      document.execCommand("insertText", false, "\\|");
      return;
    }
    if (mod && !event.altKey) {
      const k = key.toLowerCase();
      if (k === "z" || k === "y") {
        done();
        if (k === "y" || event.shiftKey) redo(this.view);
        else undo(this.view);
      } else if (k === "b" || k === "i") {
        done();
        this.formatInCell(el, r, c, k === "b" ? "**" : "*");
      } else if (k === "a" && (el.textContent ?? "") === "") {
        // In an empty cell there is nothing to select: the whole table then.
        done();
        this.select({ ar: 0, ac: 0, hr: this.lastRow, hc: this.table.columns - 1 });
      }
      return;
    }
    if (event.altKey) return;

    const caret = caretAt(el);
    // Shift + arrow over the edge of the text selects cells.
    if (event.shiftKey) {
      const extend: Record<string, [number, number, boolean]> = {
        ArrowUp: [-1, 0, true],
        ArrowDown: [1, 0, true],
        ArrowLeft: [0, -1, caret.start],
        ArrowRight: [0, 1, caret.end],
      };
      const e = extend[key];
      if (e && e[2]) {
        done();
        const hr = Math.max(0, Math.min(r + e[0], this.lastRow));
        const hc = Math.max(0, Math.min(c + e[1], this.table.columns - 1));
        this.select({ ar: r, ac: c, hr, hc });
        return;
      }
    }
    switch (key) {
      case "Tab":
        done();
        if (event.shiftKey) {
          if (c > 0) this.focusCell(r, c - 1, true);
          else if (r > 0) this.focusCell(r - 1, this.table.columns - 1, true);
        } else if (c < this.table.columns - 1) this.focusCell(r, c + 1, true);
        else if (r < this.lastRow) this.focusCell(r + 1, 0, true);
        else this.addRow(this.table.rows.length, 0);
        return;
      case "Enter":
        done();
        if (r < this.lastRow) this.focusCell(r + 1, c, true);
        else this.addRow(this.table.rows.length, c);
        return;
      case "ArrowUp":
        done();
        if (r > 0) this.focusCell(r - 1, c, true);
        else this.leave(false);
        return;
      case "ArrowDown":
        done();
        if (r < this.lastRow) this.focusCell(r + 1, c, true);
        else this.leave(true);
        return;
      case "ArrowLeft":
        if (event.shiftKey || !caret.start) return;
        done();
        if (c > 0) this.focusCell(r, c - 1, true);
        else if (r > 0) this.focusCell(r - 1, this.table.columns - 1, true);
        else this.leave(false);
        return;
      case "ArrowRight":
        if (event.shiftKey || !caret.end) return;
        done();
        if (c < this.table.columns - 1) this.focusCell(r, c + 1, false);
        else if (r < this.lastRow) this.focusCell(r + 1, 0, false);
        else this.leave(true);
        return;
      case "Escape":
        done();
        this.leave(true);
        return;
    }
  }

  /** Ctrl+B / Ctrl+I in a cell: around the selected text, or the whole cell when nothing is selected. */
  private formatInCell(el: HTMLElement, r: number, c: number, mark: "**" | "*"): void {
    const selection = el.ownerDocument.getSelection();
    const text = selection && !selection.isCollapsed && el.contains(selection.anchorNode) ? selection.toString() : "";
    if (text) {
      document.execCommand("insertText", false, `${mark}${text}${mark}`);
      return;
    }
    const [next] = toggleMark([this.raw(r, c)], mark);
    this.apply(setCell(this.table, r, c, next));
  }

  private onContextMenu(event: MouseEvent): void {
    const hit = this.cellAt(event.clientX, event.clientY);
    if (!hit) return;
    let rect = this.sel ? rectOf(this.sel) : null;
    if (!rect || !inRect(rect, hit.r, hit.c)) {
      this.clearSelection();
      rect = { r0: hit.r, c0: hit.c, r1: hit.r, c1: hit.c };
    }
    const { r0, c0, r1, c1 } = rect;
    const many = r0 !== r1 || c0 !== c1;
    const items: MenuEntry[] = [];
    // A new row above the header would turn the header into a body row.
    if (r0 > 0) items.push({ label: "Insert row above", action: () => this.addRow(r0, c0) });
    items.push({ label: "Insert row below", action: () => this.addRow(r1 + 1, c0) });
    items.push({ label: "Insert column left", action: () => this.addColumn(c0, r0) });
    items.push({ label: "Insert column right", action: () => this.addColumn(c1 + 1, r0) });
    items.push("separator");
    const columns = Array.from({ length: c1 - c0 + 1 }, (_, i) => c0 + i);
    const label = columns.length > 1 ? "Columns" : "Column";
    items.push({ label: `Align ${label.toLowerCase()} left`, action: () => this.apply(setAlign(this.table, columns, "left")) });
    items.push({ label: `Center ${label.toLowerCase()}`, action: () => this.apply(setAlign(this.table, columns, "center")) });
    items.push({ label: `Align ${label.toLowerCase()} right`, action: () => this.apply(setAlign(this.table, columns, "right")) });
    if (many) {
      items.push("separator");
      items.push({
        label: "Bold",
        action: () => {
          this.select({ ar: r0, ac: c0, hr: r1, hc: c1 });
          this.formatSelected("**");
        },
      });
      items.push({
        label: "Italic",
        action: () => {
          this.select({ ar: r0, ac: c0, hr: r1, hc: c1 });
          this.formatSelected("*");
        },
      });
      items.push({
        label: "Clear contents",
        action: () => {
          this.select({ ar: r0, ac: c0, hr: r1, hc: c1 });
          this.clearSelected();
        },
      });
    }
    const bodyFrom = Math.max(r0, 1);
    const deleteRows = r1 >= 1;
    const deleteCols = c1 - c0 + 1 < this.table.columns;
    if (deleteRows || deleteCols) items.push("separator");
    if (deleteRows) {
      const n = r1 - bodyFrom + 1;
      items.push({ label: n > 1 ? `Delete ${n} rows` : "Delete row", danger: true, action: () => this.removeRows(bodyFrom, r1, c0) });
    }
    if (deleteCols) {
      const n = c1 - c0 + 1;
      items.push({ label: n > 1 ? `Delete ${n} columns` : "Delete column", danger: true, action: () => this.removeColumns(c0, c1, r0) });
    }
    contextMenu.show(event, items);
  }

  /** Where the table starts in the note. Asked each time: text above it may have changed. */
  private get from(): number {
    return this.view.posAtDOM(this.dom);
  }

  private apply(changes: Change | Change[] | null): void {
    const list = (Array.isArray(changes) ? changes : changes ? [changes] : []);
    if (!list.length) return;
    const from = this.from;
    this.view.dispatch({
      changes: list.map((ch) => ({ from: from + ch.from, to: from + ch.to, insert: ch.insert })),
      userEvent: "input",
    });
  }

  /** Writes a new text of the whole table as one change, limited to the part that differs. */
  private applyText(next: string): void {
    this.apply(diff(this.source, next));
  }

  private addRow(at: number, c: number): void {
    this.clearSelection();
    this.apply(insertRow(this.table, Math.max(at, 1)));
    this.focusCell(Math.max(at, 1), c, true);
  }

  private removeRows(from: number, to: number, c: number): void {
    this.clearSelection();
    // One change from the line break in front of the first row to the end of the last.
    const first = this.table.rows[from];
    const last = this.table.rows[to];
    this.apply({ from: first.start - 1, to: last.start + last.length, insert: "" });
    this.focusCell(Math.min(from, this.lastRow), c, true);
  }

  private addColumn(at: number, r: number): void {
    this.clearSelection();
    this.apply(insertColumn(this.table, at));
    this.focusCell(r, at, true);
  }

  private removeColumns(from: number, to: number, r: number): void {
    this.clearSelection();
    // From the right, each on the text the one before left: their pipes can touch.
    let text = this.source;
    let table = this.table;
    for (let c = to; c >= from; c--) {
      text = applyChanges(text, deleteColumn(table, c));
      table = reparse(table, text);
    }
    this.applyText(text);
    this.focusCell(r, Math.min(from, this.table.columns - 1), true);
  }

  focusCell(r: number, c: number, atEnd: boolean): void {
    const el = this.cells[r]?.[c];
    if (!el) return;
    this.clearSelection();
    el.focus();
    this.showSource(el);
    placeCaret(el, atEnd);
  }

  /** Back to the text: the line above (`after` false) or below the table. */
  private leave(after: boolean): void {
    const from = this.from;
    const to = from + this.source.length;
    const { doc } = this.view.state;
    let anchor: number;
    if (!after) {
      if (from === 0) return;
      anchor = from - 1;
    } else if (to >= doc.length) {
      // The table ends the note: a new line below it to write on.
      this.view.dispatch({ changes: { from: to, insert: "\n" } });
      anchor = to + 1;
    } else {
      anchor = to + 1;
    }
    this.view.dispatch({ selection: { anchor }, scrollIntoView: true });
    this.view.focus();
  }
}

/** A markdown table that stays a table while it is edited. */
export class TableWidget extends WidgetType {
  constructor(readonly source: string) {
    super();
  }

  eq(other: TableWidget) {
    return other.source === this.source;
  }

  toDOM(view: EditorView) {
    return new TableEditor(view, this.source).dom;
  }

  /** Keeps the table (and the cell being typed in) when only its text changed. */
  updateDOM(dom: HTMLElement) {
    const editor = editors.get(dom);
    if (!editor) return false;
    editor.update(this.source);
    return true;
  }

  /** The cells handle their own clicks and keys, the editor stays out of them. */
  ignoreEvent() {
    return true;
  }
}

/** Puts the focus into the table drawn at `pos`: its first row, or its last one when coming from below. */
export function focusTable(view: EditorView, pos: number, fromBelow: boolean): boolean {
  for (const el of view.contentDOM.querySelectorAll<HTMLElement>(".cm-lp-table-wrap")) {
    const editor = editors.get(el);
    if (!editor || view.posAtDOM(el) !== pos) continue;
    editor.focusCell(fromBelow ? editor.lastRow : 0, 0, false);
    return true;
  }
  return false;
}
