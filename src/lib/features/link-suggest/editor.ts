// Suggestions for `[[wikilinks]]`: typing `[[` lists the files of the vault,
// filtered with the same fuzzy match as the quick switcher.
import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { fileTypeFor } from "../../filetypes";
import { fuzzyMatch } from "../../fuzzy";
import { vault } from "../../vault.svelte";

const LIMIT = 50;

/** Code is not markdown: no link suggestions in there. */
const CODE = new Set(["InlineCode", "FencedCode", "CodeBlock", "CodeText"]);

/**
 * What goes between the brackets: the bare name, like in Obsidian, unless
 * another file has the same name. Then the path makes it unique, because
 * `vault.resolveLink` takes the first file with a matching name.
 */
function linkTexts(): { path: string; label: string; link: string; isNote: boolean }[] {
  const files = vault.entries.filter((e) => !e.isDir);
  const count = new Map<string, number>();
  const nameOf = (path: string, isNote: boolean) => {
    const name = path.split("/").pop() ?? path;
    return isNote ? name.replace(/\.md$/i, "") : name;
  };
  const items = files.map((f) => {
    const isNote = fileTypeFor(f.path)?.id === "markdown";
    const label = isNote ? f.path.replace(/\.md$/i, "") : f.path;
    const name = nameOf(f.path, isNote).toLowerCase();
    count.set(name, (count.get(name) ?? 0) + 1);
    return { path: f.path, label, name, isNote, modified: f.modified };
  });
  const rank = new Map(vault.recent.map((p, i) => [p, i]));
  // Recently opened first, then newest changes, like the quick switcher.
  items.sort((a, b) => (rank.get(a.path) ?? 1e6) - (rank.get(b.path) ?? 1e6) || b.modified - a.modified);
  return items.map((i) => ({
    path: i.path,
    label: i.label,
    isNote: i.isNote,
    link: count.get(i.name)! > 1 ? i.label : nameOf(i.path, i.isNote),
  }));
}

function wikilinkCompletions(context: CompletionContext): CompletionResult | null {
  // From `[[` to the cursor, before any `|` (alias) or `#` (section).
  const before = context.matchBefore(/\[\[[^[\]|#\n]*$/);
  if (!before) return null;
  const node = syntaxTree(context.state).resolveInner(context.pos, -1);
  for (let n: typeof node | null = node; n; n = n.parent) if (CODE.has(n.name)) return null;

  const query = before.text.slice(2).trim();
  const from = before.from + 2;
  // A `]]` right behind the cursor is reused, not doubled.
  const closed = context.state.sliceDoc(context.pos, context.pos + 2) === "]]";

  const scored: { option: Completion; score: number }[] = [];
  for (const item of linkTexts()) {
    const match = fuzzyMatch(query, item.label);
    if (!match) continue;
    const slash = item.label.lastIndexOf("/");
    scored.push({
      score: match.score,
      option: {
        label: slash >= 0 ? item.label.slice(slash + 1) : item.label,
        detail: slash >= 0 ? item.label.slice(0, slash) : undefined,
        type: item.isNote ? "text" : "file",
        apply: (view: EditorView, _c: Completion, start: number, end: number) => {
          const insert = `${item.link}]]`;
          view.dispatch({
            changes: { from: start, to: closed ? end + 2 : end, insert },
            selection: { anchor: start + insert.length },
          });
        },
      },
    });
  }
  if (query) scored.sort((a, b) => b.score - a.score);
  // Each entry keeps its order: CodeMirror would otherwise sort by its own score.
  const options = scored.slice(0, LIMIT).map((s, i) => ({ ...s.option, boost: LIMIT - i }));
  return { from, options, filter: false };
}

const theme = EditorView.theme({
  ".cm-tooltip.cm-tooltip-autocomplete": {
    backgroundColor: "var(--color-zinc-900)",
    border: "1px solid var(--color-zinc-800)",
    borderRadius: "8px",
    boxShadow: "0 10px 30px rgba(0,0,0,0.4)",
    padding: "4px",
    fontFamily: "var(--font-sans)",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul": { maxHeight: "16rem", minWidth: "16rem" },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li": {
    color: "var(--color-zinc-300)",
    borderRadius: "5px",
    padding: "3px 8px",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "var(--color-zinc-800)",
    color: "var(--color-zinc-100)",
  },
  ".cm-completionDetail": { color: "var(--color-zinc-500)", fontStyle: "normal", marginLeft: "0.75em" },
  ".cm-completionMatchedText": { textDecoration: "none", color: "var(--color-violet-400)" },
});

export function linkSuggestExtensions(): Extension[] {
  return [
    autocompletion({ icons: false }),
    // As language data instead of `override`, so other plugins can add their own sources.
    EditorState.languageData.of(() => [{ autocomplete: wikilinkCompletions }]),
    theme,
  ];
}
