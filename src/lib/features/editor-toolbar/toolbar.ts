// Floating selection formatting toolbar and inline formatting commands
// (Bold/Italic shortcuts) for the Markdown editor.
import { syntaxTree } from "@codemirror/language";
import { EditorSelection, StateField, type EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap, showTooltip, type Tooltip } from "@codemirror/view";

const SVG_NS = "http://www.w3.org/2000/svg";

function iconSvg(paths: string[]): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "14");
  svg.setAttribute("height", "14");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

/**
 * Toggles an inline marker pair (e.g. `**`, `*`, `~~`, `` ` ``, `[[` / `]]`)
 * around every non-empty selection, or inserts a pair at an empty cursor.
 */
function toggleWrap(prefix: string, suffix: string) {
  return (view: EditorView): boolean => {
    const { state } = view;
    const isSingleStar = prefix === "*" && suffix === "*";
    const changes = state.changeByRange((range) => {
      const text = state.doc.sliceString(range.from, range.to);
      // 1. Markers sit inside the selected range.
      if (
        text.length >= prefix.length + suffix.length &&
        text.startsWith(prefix) &&
        text.endsWith(suffix) &&
        (!isSingleStar || !text.startsWith("**") || text.startsWith("***"))
      ) {
        const inner = text.slice(prefix.length, text.length - suffix.length);
        return {
          changes: { from: range.from, to: range.to, insert: inner },
          range: EditorSelection.range(range.from, range.from + inner.length),
        };
      }
      // 2. Markers sit immediately outside the selected range.
      if (range.from >= prefix.length && range.to + suffix.length <= state.doc.length) {
        const before = state.doc.sliceString(range.from - prefix.length, range.from);
        const after = state.doc.sliceString(range.to, range.to + suffix.length);
        const outerBefore =
          isSingleStar && range.from >= 2 ? state.doc.sliceString(range.from - 2, range.from) : "";
        const outerAfter =
          isSingleStar && range.to + 2 <= state.doc.length
            ? state.doc.sliceString(range.to, range.to + 2)
            : "";
        if (
          before === prefix &&
          after === suffix &&
          (!isSingleStar || outerBefore !== "**" || outerAfter !== "**")
        ) {
          return {
            changes: [
              { from: range.from - prefix.length, to: range.from, insert: "" },
              { from: range.to, to: range.to + suffix.length, insert: "" },
            ],
            range: EditorSelection.range(range.from - prefix.length, range.to - prefix.length),
          };
        }
      }
      // 3. Wrap the selection (or insert empty markers and place cursor inside).
      return {
        changes: { from: range.from, to: range.to, insert: `${prefix}${text}${suffix}` },
        range: range.empty
          ? EditorSelection.cursor(range.from + prefix.length)
          : EditorSelection.range(range.from + prefix.length, range.to + prefix.length),
      };
    });
    view.dispatch(changes, { userEvent: "input" });
    return true;
  };
}

/** Cycles H1 -> H2 -> H3 -> plain on single line, or toggles H2 on multi-line selections. */
function toggleHeading(view: EditorView): boolean {
  const { state } = view;
  const sel = state.selection.main;
  const firstLine = state.doc.lineAt(sel.from).number;
  const lastLine = state.doc.lineAt(Math.max(sel.from, sel.to - (sel.to > sel.from && state.doc.lineAt(sel.to).from === sel.to ? 1 : 0))).number;

  if (firstLine === lastLine) {
    const line = state.doc.line(firstLine);
    const match = line.text.match(/^(#{1,6})[ \t]+/);
    let nextPrefix = "# ";
    if (match) {
      const level = match[1].length;
      nextPrefix = level < 3 ? `${"#".repeat(level + 1)} ` : "";
    }
    const from = line.from;
    const to = line.from + (match ? match[0].length : 0);
    view.dispatch({
      changes: { from, to, insert: nextPrefix },
      userEvent: "input",
    });
    return true;
  }

  const lines = [];
  for (let n = firstLine; n <= lastLine; n++) lines.push(state.doc.line(n));
  const allHeadings = lines.every((l) => /^#{1,6}[ \t]+/.test(l.text));
  const changes = lines.map((line) => {
    const match = line.text.match(/^(#{1,6})[ \t]+/);
    if (allHeadings && match) {
      return { from: line.from, to: line.from + match[0].length, insert: "" };
    }
    if (!allHeadings && !match && line.text.trim().length > 0) {
      return { from: line.from, to: line.from, insert: "## " };
    }
    return { from: line.from, to: line.from, insert: "" };
  });
  view.dispatch({ changes, userEvent: "input" });
  return true;
}

/** Toggles blockquote (`> `) on every line covered by the main selection. */
function toggleQuote(view: EditorView): boolean {
  const { state } = view;
  const sel = state.selection.main;
  const firstLine = state.doc.lineAt(sel.from).number;
  const lastLine = state.doc.lineAt(Math.max(sel.from, sel.to - (sel.to > sel.from && state.doc.lineAt(sel.to).from === sel.to ? 1 : 0))).number;
  const lines = [];
  for (let n = firstLine; n <= lastLine; n++) lines.push(state.doc.line(n));
  const allQuoted = lines.every((l) => /^\s*> ?/.test(l.text));
  const changes = lines.map((line) => {
    if (allQuoted) {
      const match = line.text.match(/^(\s*> ?)/);
      return match ? { from: line.from, to: line.from + match[1].length, insert: "" } : { from: line.from, to: line.from, insert: "" };
    }
    return { from: line.from, to: line.from, insert: "> " };
  });
  view.dispatch({ changes, userEvent: "input" });
  return true;
}

const toggleBold = toggleWrap("**", "**");
const toggleItalic = toggleWrap("*", "*");
const toggleStrike = toggleWrap("~~", "~~");
const toggleCode = toggleWrap("`", "`");
const toggleWikilink = toggleWrap("[[", "]]");

interface ToolbarButtonSpec {
  title: string;
  ariaLabel: string;
  label?: string;
  icon?: () => HTMLElement | SVGSVGElement;
  run: (view: EditorView) => boolean;
}

const ITEMS: (ToolbarButtonSpec | "sep")[] = [
  { title: "Bold (Ctrl+B)", ariaLabel: "Bold", label: "B", run: toggleBold },
  {
    title: "Italic (Ctrl+I)",
    ariaLabel: "Italic",
    icon: () => {
      const em = document.createElement("span");
      em.style.fontStyle = "italic";
      em.style.fontFamily = "var(--font-serif)";
      em.textContent = "I";
      return em;
    },
    run: toggleItalic,
  },
  {
    title: "Strikethrough",
    ariaLabel: "Strikethrough",
    icon: () => {
      const s = document.createElement("span");
      s.style.textDecoration = "line-through";
      s.textContent = "S";
      return s;
    },
    run: toggleStrike,
  },
  "sep",
  {
    title: "Inline code",
    ariaLabel: "Inline code",
    icon: () => iconSvg(["m18 16 4-4-4-4", "m6 8-4 4 4 4", "m14.5 4-5 16"]),
    run: toggleCode,
  },
  {
    title: "Wikilink",
    ariaLabel: "Wikilink",
    label: "[[ ]]",
    run: toggleWikilink,
  },
  "sep",
  { title: "Heading", ariaLabel: "Heading", label: "H", run: toggleHeading },
  {
    title: "Quote",
    ariaLabel: "Quote",
    icon: () =>
      iconSvg([
        "M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z",
        "M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z",
      ]),
    run: toggleQuote,
  },
];

function createToolbarDOM(view: EditorView): HTMLElement {
  const dom = document.createElement("div");
  dom.className = "cm-selection-toolbar";
  dom.setAttribute("role", "toolbar");
  dom.setAttribute("aria-label", "Text formatting");
  // Prevent mousedown from stealing focus or clearing the editor selection.
  dom.addEventListener("mousedown", (e) => e.preventDefault());

  for (const item of ITEMS) {
    if (item === "sep") {
      const sep = document.createElement("div");
      sep.className = "cm-selection-toolbar-sep";
      dom.appendChild(sep);
      continue;
    }
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "cm-selection-toolbar-btn";
    btn.title = item.title;
    btn.setAttribute("aria-label", item.ariaLabel);
    if (item.icon) btn.appendChild(item.icon());
    else if (item.label) btn.textContent = item.label;
    btn.addEventListener("click", () => {
      item.run(view);
      view.focus();
    });
    dom.appendChild(btn);
  }
  return dom;
}

function selectionTooltip(state: EditorState): readonly Tooltip[] {
  const sel = state.selection.main;
  if (sel.empty || state.selection.ranges.length > 1) return [];
  if (state.doc.sliceString(sel.from, sel.to).trim().length === 0) return [];
  if (syntaxTree(state).resolveInner(sel.from, 1).name === "Frontmatter") return [];
  return [
    {
      pos: sel.from,
      end: sel.to,
      above: true,
      create: (view) => ({
        dom: createToolbarDOM(view),
        offset: { x: 0, y: 6 },
      }),
    },
  ];
}

const toolbarTooltips = StateField.define<readonly Tooltip[]>({
  create: selectionTooltip,
  update(tooltips, tr) {
    if (!tr.docChanged && !tr.selection) return tooltips;
    return selectionTooltip(tr.state);
  },
  provide: (f) => showTooltip.computeN([f], (state) => state.field(f)),
});

const formattingKeymap = keymap.of([
  { key: "Mod-b", run: toggleBold },
  { key: "Mod-i", run: toggleItalic },
]);

/** Returns the CodeMirror extensions for the floating selection toolbar and formatting shortcuts. */
export function editorToolbarExtensions(): Extension {
  return [toolbarTooltips, formattingKeymap];
}
