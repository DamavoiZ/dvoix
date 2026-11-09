// A GFM table as rows and cells with their positions in the text, and the
// edits on it (type in a cell, add or remove a row or column) as small text
// changes. Only the touched cells change; the padding of the other cells and
// the style of the table (with or without outer pipes) stay as they are.
// All positions are relative to the start of the table's first line, with
// "\n" line breaks (the way CodeMirror keeps the text).

export type Align = "left" | "center" | "right" | null;

export interface Cell {
  /** The markdown of the cell, trimmed. `\|` stays escaped. */
  text: string;
  /** The trimmed content within the line. */
  from: number;
  to: number;
  /** Everything between the two pipes around the cell. */
  segFrom: number;
  segTo: number;
}

export interface Row {
  /** The line itself. */
  text: string;
  cells: Cell[];
  /** Start of the line in the table. */
  start: number;
  /** Length of the line. */
  length: number;
  /** Whether the line starts / ends with a pipe. */
  lead: boolean;
  trail: boolean;
  /** Position of the trailing pipe in the line, if there is one. */
  trailPipe: number;
}

export interface Table {
  /** The header (row 0) and the body rows (1 …). The delimiter row is not in here. */
  rows: Row[];
  delimiter: Row;
  aligns: Align[];
  /** As many as the header has cells. */
  columns: number;
}

export interface Change {
  from: number;
  to: number;
  insert: string;
}

/** Splits a line at its pipes. `\|` and pipes inside `code` are text. */
export function parseRow(line: string, start = 0): Row {
  const pipes: number[] = [];
  let inCode = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === "\\") i++;
    else if (ch === "`") inCode = !inCode;
    else if (ch === "|" && !inCode) pipes.push(i);
  }
  const first = line.search(/\S/);
  const last = line.trimEnd().length - 1;
  const lead = pipes.length > 0 && pipes[0] === first;
  const trail = pipes.length > 0 && pipes[pipes.length - 1] === last && !(lead && pipes.length === 1);

  const points = [-1, ...pipes, line.length];
  const segments: [number, number][] = [];
  for (let i = 0; i + 1 < points.length; i++) segments.push([points[i] + 1, points[i + 1]]);
  if (lead) segments.shift();
  if (trail) segments.pop();

  const cells = segments.map(([segFrom, segTo]): Cell => {
    const seg = line.slice(segFrom, segTo);
    const text = seg.trim();
    if (!text) {
      // An empty cell: typing goes after the first space, so `|  |` becomes `| x |`.
      const at = Math.min(segFrom + (seg.startsWith(" ") ? 1 : 0), segTo);
      return { text, from: at, to: at, segFrom, segTo };
    }
    const from = segFrom + (seg.length - seg.trimStart().length);
    return { text, from, to: from + text.length, segFrom, segTo };
  });
  return { text: line, cells, start, length: line.length, lead, trail, trailPipe: trail ? pipes[pipes.length - 1] : -1 };
}

function alignOf(cell: string): Align {
  const left = cell.startsWith(":");
  const right = cell.endsWith(":");
  if (left && right) return "center";
  if (right) return "right";
  if (left) return "left";
  return null;
}

export function parseTable(source: string): Table {
  const rows: Row[] = [];
  let delimiter: Row | null = null;
  let start = 0;
  source.split("\n").forEach((line, i) => {
    const row = parseRow(line, start);
    if (i === 1) delimiter = row;
    else rows.push(row);
    start += line.length + 1;
  });
  delimiter ??= parseRow("", start);
  return {
    rows,
    delimiter,
    aligns: delimiter.cells.map((c) => alignOf(c.text)),
    columns: rows[0]?.cells.length ?? 0,
  };
}

/** Escapes pipes the user typed, so they do not split the cell. */
export function escapeCell(text: string): string {
  return text.replace(/\\\\|\\\||\|/g, (m) => (m === "|" ? "\\|" : m)).replace(/\n/g, " ");
}

/** Writes the text of one cell. A row with fewer cells gets the missing ones. */
export function setCell(table: Table, r: number, c: number, text: string): Change {
  const row = table.rows[r];
  const cell = row.cells[c];
  if (cell) return { from: row.start + cell.from, to: row.start + cell.to, insert: text };
  const missing = c - row.cells.length;
  if (row.trail) {
    const at = row.start + row.trailPipe + 1;
    return { from: at, to: at, insert: `${"  |".repeat(missing)} ${text} |` };
  }
  const at = row.start + row.length;
  return { from: at, to: at, insert: `${" |".repeat(missing)} | ${text}` };
}

function emptyRow(table: Table): string {
  return `|${Array.from({ length: table.columns }, () => "  ").join("|")}|`;
}

/** Adds an empty row in front of body row `at` (1 … rows.length; rows.length = at the end). */
export function insertRow(table: Table, at: number): Change {
  const row = emptyRow(table);
  if (at < table.rows.length) {
    const pos = table.rows[Math.max(at, 1)].start;
    return { from: pos, to: pos, insert: `${row}\n` };
  }
  const last = table.rows.length > 1 ? table.rows[table.rows.length - 1] : table.delimiter;
  const end = last.start + last.length;
  return { from: end, to: end, insert: `\n${row}` };
}

/** Removes a body row (not the header) with its line break. */
export function deleteRow(table: Table, r: number): Change {
  const row = table.rows[r];
  return { from: row.start - 1, to: row.start + row.length, insert: "" };
}

/** Every line of the table: header, delimiter, body. */
function allLines(table: Table): { row: Row; delimiter: boolean }[] {
  const [header, ...body] = table.rows;
  return [
    { row: header, delimiter: false },
    { row: table.delimiter, delimiter: true },
    ...body.map((row) => ({ row, delimiter: false })),
  ];
}

/** Adds an empty column in front of column `at` (columns = at the end). */
export function insertColumn(table: Table, at: number): Change[] {
  const changes: Change[] = [];
  for (const { row, delimiter } of allLines(table)) {
    const content = delimiter ? " --- " : "  ";
    const cell = row.cells[at];
    if (cell) {
      changes.push({ from: row.start + cell.segFrom, to: row.start + cell.segFrom, insert: `${content}|` });
    } else if (row.trail) {
      const pos = row.start + row.trailPipe + 1;
      changes.push({ from: pos, to: pos, insert: `${content}|` });
    } else {
      const pos = row.start + row.length;
      changes.push({ from: pos, to: pos, insert: ` |${content}` });
    }
  }
  return changes;
}

/** Removes a column with one of the pipes next to it. */
export function deleteColumn(table: Table, c: number): Change[] {
  const changes: Change[] = [];
  for (const { row } of allLines(table)) {
    const cell = row.cells[c];
    if (!cell) continue;
    const line = row.start;
    const pipeAfter = cell.segTo < row.length;
    // With a pipe behind the cell, that one goes; otherwise (last cell without outer pipe) the one in front.
    changes.push(
      pipeAfter
        ? { from: line + cell.segFrom, to: line + cell.segTo + 1, insert: "" }
        : { from: line + cell.segFrom - 1, to: line + cell.segTo, insert: "" },
    );
  }
  return changes;
}

/** A cell to write, by row (0 = header) and column. */
export interface CellUpdate {
  r: number;
  c: number;
  text: string;
}

/** A line written fresh, for a row that is missing cells. */
function writeRow(texts: string[]): string {
  return `| ${texts.join(" | ")} |`;
}

/**
 * Writes several cells at once (paste, clear, format, move). Cells that
 * exist change in place; a row that is missing some of them is written anew.
 */
export function setCells(table: Table, updates: CellUpdate[]): Change[] {
  const byRow = new Map<number, CellUpdate[]>();
  for (const u of updates) {
    if (!table.rows[u.r]) continue;
    byRow.set(u.r, [...(byRow.get(u.r) ?? []), u]);
  }
  const changes: Change[] = [];
  for (const [r, list] of byRow) {
    const row = table.rows[r];
    if (list.every((u) => u.c < row.cells.length)) {
      for (const u of list) {
        const cell = row.cells[u.c];
        if (cell.text !== u.text) changes.push({ from: row.start + cell.from, to: row.start + cell.to, insert: u.text });
      }
      continue;
    }
    const width = Math.max(table.columns, ...list.map((u) => u.c + 1));
    const texts = Array.from({ length: width }, (_, c) => row.cells[c]?.text ?? "");
    for (const u of list) texts[u.c] = u.text;
    changes.push({ from: row.start, to: row.start + row.length, insert: writeRow(texts) });
  }
  return changes.sort((a, b) => a.from - b.from);
}

/**
 * Moves the body rows `from` … `to` in front of row `before` (1 … rows.length).
 * The header stays where it is. Only the lines in between change.
 */
export function moveRows(table: Table, from: number, to: number, before: number): Change | null {
  if (from < 1 || to < from || (before >= from && before <= to + 1)) return null;
  const lo = Math.min(from, before);
  const hi = Math.max(to, before - 1);
  const lines = table.rows.slice(lo, hi + 1).map((r) => r.text);
  const moved = lines.splice(from - lo, to - from + 1);
  lines.splice(before < from ? before - lo : before - lo - moved.length, 0, ...moved);
  const first = table.rows[lo];
  const last = table.rows[hi];
  return { from: first.start, to: last.start + last.length, insert: lines.join("\n") };
}

/** Moves the columns `from` … `to` in front of column `before` (0 … columns). */
export function moveColumns(table: Table, from: number, to: number, before: number): Change[] {
  if (to < from || (before >= from && before <= to + 1)) return [];
  const changes: Change[] = [];
  for (const { row, delimiter } of allLines(table)) {
    const width = Math.max(table.columns, row.cells.length);
    let segments = row.cells.map((c) => row.text.slice(c.segFrom, c.segTo));
    const short = segments.length < width;
    // A row missing cells gets empty ones, so every column has something to move.
    while (segments.length < width) segments.push(delimiter ? " --- " : "  ");
    const moved = segments.splice(from, to - from + 1);
    segments.splice(before < from ? before : before - moved.length, 0, ...moved);
    if (short) {
      segments = segments.map((s) => s.trim());
      const insert = delimiter ? `|${segments.map((s) => ` ${s || "---"} `).join("|")}|` : writeRow(segments);
      changes.push({ from: row.start, to: row.start + row.length, insert });
    } else {
      const firstCell = row.cells[0];
      const lastCell = row.cells[row.cells.length - 1];
      changes.push({ from: row.start + firstCell.segFrom, to: row.start + lastCell.segTo, insert: segments.join("|") });
    }
  }
  return changes;
}

/** Sets the alignment of columns in the delimiter row: `:---`, `:---:`, `---:` or `---`. */
export function setAlign(table: Table, columns: number[], align: Align): Change[] {
  const changes: Change[] = [];
  const row = table.delimiter;
  for (const c of columns) {
    const cell = row.cells[c];
    if (!cell) continue;
    const dashes = "-".repeat(Math.max(3, cell.text.replace(/:/g, "").length));
    const text = align === "left" ? `:${dashes}` : align === "center" ? `:${dashes}:` : align === "right" ? `${dashes}:` : dashes;
    if (text !== cell.text) changes.push({ from: row.start + cell.from, to: row.start + cell.to, insert: text });
  }
  return changes;
}

/** Whether a cell is wrapped in a mark: `**bold**`, or `*italic*` but not `**bold**`. */
function hasMark(text: string, mark: "**" | "*"): boolean {
  if (text.length <= mark.length * 2 || !text.startsWith(mark) || !text.endsWith(mark)) return false;
  if (mark === "**") return true;
  // `***both***` is italic too, `**bold**` is not.
  return !text.startsWith("**") || text.startsWith("***");
}

/** Bold or italic for several cells: on for all of them, or off if all have it. Empty cells stay empty. */
export function toggleMark(texts: string[], mark: "**" | "*"): string[] {
  const filled = texts.filter((t) => t !== "");
  const off = filled.length > 0 && filled.every((t) => hasMark(t, mark));
  return texts.map((t) => {
    if (t === "") return t;
    if (off) return t.slice(mark.length, -mark.length);
    return hasMark(t, mark) ? t : `${mark}${t}${mark}`;
  });
}

/** A block of cells: rows `r0` … `r1`, columns `c0` … `c1`. */
export interface Rect {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

/**
 * Moves the contents of the cells in `rect` so its top left lands on
 * `(r, c)`. The cells there take the place of the moved ones, so nothing is
 * lost: without overlap the two blocks swap, with overlap the pushed-out
 * contents fill the cells that became free.
 */
export function moveBlock(table: Table, rect: Rect, r: number, c: number): CellUpdate[] {
  const h = rect.r1 - rect.r0;
  const w = rect.c1 - rect.c0;
  // The block stays inside the table.
  r = Math.max(0, Math.min(r, table.rows.length - 1 - h));
  c = Math.max(0, Math.min(c, table.columns - 1 - w));
  if (r === rect.r0 && c === rect.c0) return [];
  const text = (rr: number, cc: number) => table.rows[rr]?.cells[cc]?.text ?? "";
  const key = (rr: number, cc: number) => `${rr}:${cc}`;
  const result = new Map<string, CellUpdate>();
  const inTarget = (rr: number, cc: number) => rr >= r && rr <= r + h && cc >= c && cc <= c + w;
  const inSource = (rr: number, cc: number) => rr >= rect.r0 && rr <= rect.r1 && cc >= rect.c0 && cc <= rect.c1;
  const freed: [number, number][] = [];
  const displaced: string[] = [];
  for (let dr = 0; dr <= h; dr++) {
    for (let dc = 0; dc <= w; dc++) {
      result.set(key(r + dr, c + dc), { r: r + dr, c: c + dc, text: text(rect.r0 + dr, rect.c0 + dc) });
      if (!inTarget(rect.r0 + dr, rect.c0 + dc)) freed.push([rect.r0 + dr, rect.c0 + dc]);
      if (!inSource(r + dr, c + dc)) displaced.push(text(r + dr, c + dc));
    }
  }
  freed.forEach(([rr, cc], i) => result.set(key(rr, cc), { r: rr, c: cc, text: displaced[i] ?? "" }));
  return [...result.values()].filter((u) => u.text !== text(u.r, u.c));
}

/** The cells as tab-separated text, the way spreadsheets put them on the clipboard. */
export function toTSV(texts: string[][]): string {
  return texts.map((row) => row.map((t) => t.replace(/\\\|/g, "|")).join("\t")).join("\n");
}

/** Tab-separated text from the clipboard as rows of cells, escaped for the table. */
export function fromTSV(text: string): string[][] {
  const lines = text.replace(/\r\n?/g, "\n").replace(/\n$/, "").split("\n");
  return lines.map((line) => line.split("\t").map((t) => escapeCell(t.trim())));
}
