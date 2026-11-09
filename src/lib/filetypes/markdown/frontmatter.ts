// Reads the frontmatter of a note line by line and turns edits of a property
// into the smallest possible text change. A full YAML parser would lose the
// formatting and comments of the user's file on every save, so this only
// understands the shapes Obsidian writes (`key: value`, `key: [a, b]`,
// `key:` + `  - item` lines). Anything else stays as it is and is shown as raw text.

/** How a property is shown and edited. */
export type PropertyKind = "text" | "number" | "boolean" | "date" | "list" | "raw";

/** One item of a list property and where it is in the text. */
export interface ListItem {
  value: string;
  /** The spaces in front of its `-`, so a new item lines up with it. */
  indent: string;
  /** Start of its line and end of its line (without the line break). */
  from: number;
  to: number;
}

export interface Property {
  key: string;
  kind: PropertyKind;
  /** The value as text: unquoted for scalars, the YAML itself for `raw`. */
  value: string;
  /** Only for lists. */
  items: string[];
  /** How a list is written: `[a, b]`, one `- item` per line, or a single value (`tags: a`). */
  listStyle?: "flow" | "block" | "scalar";
  /** The positions of the block list items, for removing one without touching the others. */
  blockItems: ListItem[];
  /** Start of the key line, end of the property's last line (without the line break). */
  from: number;
  to: number;
  /** End of the key, i.e. the position of the colon. */
  keyTo: number;
  /** End of the value on the key line, before a `# comment`. Edits stop here, so the comment stays. */
  valueTo: number;
  /** End of the key line. */
  lineTo: number;
}

export interface Frontmatter {
  properties: Property[];
  /** Start of the closing `---` line. New properties go in front of it. */
  closeFrom: number;
  /** End of the closing `---` line. */
  to: number;
  /** The line break of the file, so new lines match the old ones. */
  eol: string;
}

/** A replacement in the note, in the same form as a CodeMirror change. */
export interface Change {
  from: number;
  to: number;
  insert: string;
}

/** Keys that Obsidian always treats as lists. */
const LIST_KEYS = new Set(["tags", "aliases", "cssclasses"]);

const KEY_LINE = /^([^\s#\-][^:]*?|"[^"]*"|'[^']*'):(?:[ \t]+(.*?))?[ \t]*$/;
const ITEM_LINE = /^([ \t]*)-(?:[ \t]+(.*?))?[ \t]*$/;
const NUMBER = /^-?\d+(?:\.\d+)?$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;

interface Line {
  text: string;
  from: number;
  /** End of the text, before `\r\n` or `\n`. */
  to: number;
}

function* linesOf(doc: string): Generator<Line> {
  let from = 0;
  while (from <= doc.length) {
    const nl = doc.indexOf("\n", from);
    const end = nl < 0 ? doc.length : nl;
    const to = end > from && doc[end - 1] === "\r" ? end - 1 : end;
    yield { text: doc.slice(from, to), from, to };
    if (nl < 0) return;
    from = nl + 1;
  }
}

/** Removes YAML quotes. Double quotes may contain escapes, single quotes double themselves. */
export function unquote(raw: string): string {
  const s = raw.trim();
  if (s.length >= 2 && s.startsWith('"') && s.endsWith('"')) {
    try {
      return JSON.parse(s) as string;
    } catch {
      return s.slice(1, -1);
    }
  }
  if (s.length >= 2 && s.startsWith("'") && s.endsWith("'")) return s.slice(1, -1).replace(/''/g, "'");
  return s;
}

const isQuoted = (raw: string) => /^(["']).*\1$/.test(raw.trim());

/** The length of a value without a trailing `# comment` (a `#` after a space, outside of quotes). */
function withoutComment(raw: string): number {
  let quote = "";
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (quote) {
      if (ch === quote) quote = "";
    } else if (ch === '"' || ch === "'") {
      if (i === 0 || /[\s[,]/.test(raw[i - 1])) quote = ch;
    } else if (ch === "#" && i > 0 && /\s/.test(raw[i - 1])) {
      return raw.slice(0, i).trimEnd().length;
    }
  }
  return raw.length;
}

/** Splits `a, "b, c", d` at the commas outside of quotes. */
function splitFlow(inner: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quote = "";
  for (const ch of inner) {
    if (quote) {
      if (ch === quote) quote = "";
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === ",") {
      out.push(cur);
      cur = "";
      continue;
    }
    cur += ch;
  }
  out.push(cur);
  return out.map(unquote).filter((s, i, all) => s !== "" || all.length > 1);
}

/** Reads the frontmatter at the start of a note. `null` if there is none or it is not closed. */
export function parseFrontmatter(doc: string): Frontmatter | null {
  const lines = linesOf(doc);
  const first = lines.next();
  if (first.done || first.value.text.trimEnd() !== "---") return null;
  const eol = doc.includes("\r\n") ? "\r\n" : "\n";

  const body: Line[] = [];
  let close: Line | null = null;
  for (const line of lines) {
    if (line.text.trimEnd() === "---") {
      close = line;
      break;
    }
    body.push(line);
  }
  if (!close) return null;

  // Group the lines: a key line and everything indented or `- ` below it.
  const groups: { head: Line; rest: Line[] }[] = [];
  for (const line of body) {
    if (KEY_LINE.test(line.text)) groups.push({ head: line, rest: [] });
    else if (groups.length && (/^[ \t-]/.test(line.text) || line.text.trim() === "")) groups[groups.length - 1].rest.push(line);
    // A comment or something unknown at the top level belongs to no property and stays untouched.
  }

  const properties = groups.map(({ head, rest }) => readProperty(head, rest));
  return { properties, closeFrom: close.from, to: close.to, eol };
}

function readProperty(head: Line, rest: Line[]): Property {
  const m = KEY_LINE.exec(head.text)!;
  const key = unquote(m[1]);
  const full = m[2] ?? "";
  const raw = full.slice(0, withoutComment(full));
  const rawFrom = full ? head.from + head.text.indexOf(full, m[1].length + 1) : head.to;
  // Trailing blank lines are no part of the value.
  while (rest.length && rest[rest.length - 1].text.trim() === "") rest.pop();
  const last = rest.length ? rest[rest.length - 1] : head;
  const prop: Property = {
    key,
    kind: "text",
    value: "",
    items: [],
    blockItems: [],
    from: head.from,
    to: last.to,
    keyTo: head.from + m[1].length,
    valueTo: raw ? rawFrom + raw.length : head.to,
    lineTo: head.to,
  };

  if (raw === "" && rest.length === 0) {
    if (LIST_KEYS.has(key.toLowerCase())) {
      prop.kind = "list";
      prop.listStyle = "block";
    }
    return prop;
  }

  if (raw === "") {
    const items = rest.map((l) => ({ line: l, m: ITEM_LINE.exec(l.text) }));
    if (items.every((i) => i.m || i.line.text.trim() === "")) {
      prop.kind = "list";
      prop.listStyle = "block";
      for (const { line, m: im } of items) {
        if (!im) continue;
        const value = unquote(im[2] ?? "");
        prop.items.push(value);
        prop.blockItems.push({ value, indent: im[1], from: line.from, to: line.to });
      }
      return prop;
    }
    return { ...prop, kind: "raw", value: rest.map((l) => l.text).join("\n") };
  }

  // A value that goes on over more lines (`|`, `>`, nested maps) is not edited here.
  if (rest.length || /^[|>{&*!]/.test(raw)) {
    return { ...prop, kind: "raw", value: [raw, ...rest.map((l) => l.text)].join("\n") };
  }

  if (raw.startsWith("[") && raw.endsWith("]")) {
    return { ...prop, kind: "list", listStyle: "flow", items: splitFlow(raw.slice(1, -1)) };
  }

  const value = unquote(raw);
  if (LIST_KEYS.has(key.toLowerCase())) {
    return { ...prop, kind: "list", listStyle: "scalar", items: value ? [value] : [] };
  }
  if (!isQuoted(raw)) {
    if (/^(true|false)$/i.test(value)) return { ...prop, kind: "boolean", value: value.toLowerCase() };
    if (NUMBER.test(value)) return { ...prop, kind: "number", value };
  }
  if (DATE.test(value)) return { ...prop, kind: "date", value };
  return { ...prop, value };
}

/**
 * Writes a text so YAML reads it back as the same text. Quotes only when
 * needed, so a value such as `status: aktiv` stays as people write it in Obsidian.
 */
export function formatScalar(value: string, inList = false): string {
  if (value === "") return '""';
  const special =
    /^[\s\-?:,[\]{}#&*!|>'"%@`]/.test(value) ||
    /\s$/.test(value) ||
    /: |\s#/.test(value) ||
    /:$/.test(value) ||
    /^(true|false|yes|no|on|off|null|~)$/i.test(value) ||
    NUMBER.test(value) ||
    (inList && /[,[\]{}]/.test(value));
  // `[[Link]]` starts with a bracket, but Obsidian writes it quoted too.
  return special ? JSON.stringify(value) : value;
}

/** Writes a key so YAML reads it back as the same key. */
function formatKey(key: string): string {
  return /^[^\s#\-"'][^:]*$/.test(key) && !/\s$/.test(key) ? key : JSON.stringify(key);
}

/** The text of a property's value behind `key:`. */
function scalarText(kind: PropertyKind, value: string): string {
  if (kind === "boolean" || kind === "number" || kind === "date") return value;
  return formatScalar(value);
}

/** Sets a single value (text, number, date, true/false) on the key line. */
export function setScalar(prop: Property, kind: PropertyKind, value: string): Change {
  const text = value === "" && kind === "text" ? "" : ` ${scalarText(kind, value)}`;
  return { from: prop.keyTo + 1, to: prop.valueTo, insert: text };
}

/** Renames a property. Only the key changes, the value keeps its formatting. */
export function renameProperty(prop: Property, key: string): Change {
  return { from: prop.from, to: prop.keyTo, insert: formatKey(key) };
}

/** Removes a property together with its line break. */
export function removeProperty(fm: Frontmatter, prop: Property): Change {
  return { from: prop.from, to: prop.to + fm.eol.length, insert: "" };
}

/** Adds a property in front of the closing `---`. */
export function addProperty(fm: Frontmatter, key: string): Change {
  return { from: fm.closeFrom, to: fm.closeFrom, insert: `${formatKey(key)}:${fm.eol}` };
}

/** Adds an item to the end of a list, in the style the list already has. */
export function addListItem(fm: Frontmatter, prop: Property, item: string): Change {
  if (prop.listStyle === "block") {
    const last = prop.blockItems[prop.blockItems.length - 1];
    // A new item gets the indentation of the ones before it.
    const indent = last ? last.indent : "  ";
    return { from: prop.to, to: prop.to, insert: `${fm.eol}${indent}- ${formatScalar(item, true)}` };
  }
  return writeFlow(prop, [...prop.items, item]);
}

/** Removes one item of a list. A block list loses just that line. */
export function removeListItem(fm: Frontmatter, prop: Property, index: number): Change {
  if (prop.listStyle === "block") {
    const item = prop.blockItems[index];
    // The line break in front of the item goes with it, so no empty line stays behind.
    return { from: item.from - fm.eol.length, to: item.to, insert: "" };
  }
  return writeFlow(prop, prop.items.filter((_, i) => i !== index));
}

function writeFlow(prop: Property, items: string[]): Change {
  const text = items.map((i) => formatScalar(i, true)).join(", ");
  return { from: prop.keyTo + 1, to: prop.valueTo, insert: ` [${text}]` };
}

/** Applies a change to a text, e.g. the note shown in the reading view. */
export function applyChange(doc: string, change: Change): string {
  return doc.slice(0, change.from) + change.insert + doc.slice(change.to);
}
