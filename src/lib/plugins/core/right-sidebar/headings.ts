// The headings of a note, with the line they stand on. The outline shows them
// as a tree and jumps to the line when one of them is clicked.

export interface Heading {
  /** 1 for "#", 2 for "##" and so on. */
  level: number;
  /** The text of the heading, without the # and without inline markdown. */
  text: string;
  /** 0-based line of the heading in the note. */
  line: number;
}

/** Takes the markers away that stand in front of a line: block quotes and list items. */
function stripPrefix(text: string): string {
  let out = text;
  for (;;) {
    const before = out;
    out = out.replace(/^[ \t]*(?:>[ \t]?)+/, "");
    out = out.replace(/^[ \t]*(?:[-*+]|\d{1,9}[.)])[ \t]+/, "");
    if (out === before) return out;
  }
}

/** Takes the inline markdown away, so the outline shows plain text. */
function plainText(text: string): string {
  return text
    .replace(/`([^`]*)`/g, "$1")
    .replace(/\*\*([^*]*)\*\*/g, "$1")
    .replace(/\*([^*]*)\*/g, "$1")
    .replace(/~~([^~]*)~~/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .trim();
}

/**
 * The headings of a markdown note, in the order they appear. Headings inside
 * a code block and the frontmatter are not headings, so they are left out.
 */
export function parseHeadings(markdown: string): Heading[] {
  const lines = markdown.split("\n");
  const headings: Heading[] = [];
  /** The marker of the code fence that is open, if any. */
  let fence = "";
  /** Whether the frontmatter at the start of the note is still open. */
  let frontmatter = false;
  /** The text of the line before, to recognize a heading written with "=" or "-". */
  let before = "";
  /** Whether the line before was a heading itself. */
  let beforeIsHeading = false;

  for (let i = 0; i < lines.length; i++) {
    const line = stripPrefix(lines[i]).trim();
    const trimmed = line.trim();

    // The frontmatter at the very start of a note is metadata, not text.
    if (i === 0 && trimmed === "---") {
      frontmatter = true;
      before = "";
      beforeIsHeading = false;
      continue;
    }
    if (frontmatter) {
      if (trimmed === "---" || trimmed === "...") frontmatter = false;
      continue;
    }

    // Inside a code block nothing is a heading.
    const marker = /^(?:```|~~~)/.exec(trimmed);
    if (marker) {
      if (!fence) fence = marker[0];
      else if (trimmed.startsWith(fence)) fence = "";
      before = "";
      beforeIsHeading = false;
      continue;
    }
    if (fence) continue;

    // A heading written with # ("## Heading")
    const atx = /^(#{1,6})(?:[ \t]+(.*))?$/.exec(trimmed);
    if (atx) {
      const text = (atx[2] ?? "").replace(/[ \t]+#+[ \t]*$/, "");
      headings.push({ level: atx[1].length, text: plainText(text), line: i });
      before = "";
      beforeIsHeading = true;
      continue;
    }

    // A heading written with a line of = or - under the text.
    if (/^=+$/.test(trimmed) || /^-+$/.test(trimmed)) {
      const text = plainText(before);
      if (text && !beforeIsHeading) {
        headings.push({ level: trimmed[0] === "=" ? 1 : 2, text, line: i - 1 });
      }
      before = "";
      beforeIsHeading = false;
      continue;
    }

    before = line;
    beforeIsHeading = false;
  }
  return headings;
}

/** One heading of the outline with how deep it stands in the tree. */
export interface OutlineEntry {
  heading: Heading;
  /** 0 for a heading at the top, 1 for one below it, and so on. */
  depth: number;
}

/** Puts the headings of a note in the order they are nested in. */
export function outlineEntries(headings: Heading[]): OutlineEntry[] {
  /** The levels of the headings the current one stands under. */
  const open: number[] = [];
  return headings.map((heading) => {
    while (open.length && open[open.length - 1] >= heading.level) open.pop();
    const depth = open.length;
    open.push(heading.level);
    return { heading, depth };
  });
}
