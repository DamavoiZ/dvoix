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
import type { VaultEntry } from "../../api";

const LIMIT = 50;

/** Code is not markdown: no link suggestions in there. */
const CODE = new Set(["InlineCode", "FencedCode", "CodeBlock", "CodeText"]);

/** A file a wikilink can point to. */
interface Target {
  path: string;
  /** What is shown and searched: the path, notes without ".md". */
  label: string;
  /** What goes between the brackets. */
  link: string;
  isNote: boolean;
}

/** The name of a file, without its folder. */
function nameOf(path: string, isNote: boolean): string {
  const slash = path.lastIndexOf("/");
  const name = slash >= 0 ? path.slice(slash + 1) : path;
  return isNote ? name.replace(/\.md$/i, "") : name;
}

/**
 * The targets of the vault, built once per change of the list of the vault.
 * Building them for every suggestion walks over all the files of the vault.
 */
let targets: { version: number; recent: string[]; map: Map<string, Target> } | null = null;

/** The files a wikilink can point to, in the order they are offered. */
function linkTargets(): Map<string, Target> {
  const version = vault.entriesVersion;
  const recent = vault.recent;
  if (!targets || targets.version !== version || targets.recent !== recent) {
    targets = { version, recent, map: buildTargets(vault.entries, recent) };
  }
  return targets.map;
}

/**
 * What goes between the brackets: the bare name, like in Obsidian, unless
 * another file has the same name. Then the path makes it unique, because
 * `vault.resolveLink` takes the first file with a matching name.
 */
function buildTargets(entries: VaultEntry[], recent: string[]): Map<string, Target> {
  const count = new Map<string, number>();
  const items: { target: Target; name: string; key: string; rank: number; modified: number }[] = [];
  for (const entry of entries) {
    if (entry.isDir) continue;
    const isNote = fileTypeFor(entry.path)?.id === "markdown";
    const label = isNote ? entry.path.replace(/\.md$/i, "") : entry.path;
    const name = nameOf(entry.path, isNote);
    const key = name.toLowerCase();
    count.set(key, (count.get(key) ?? 0) + 1);
    items.push({ target: { path: entry.path, label, link: "", isNote }, name, key, rank: 0, modified: entry.modified });
  }
  const rank = new Map(recent.map((p, i) => [p, i]));
  // The rank is put into the item, so the sort does not look it up again for
  // every comparison.
  for (const item of items) item.rank = rank.get(item.target.path) ?? 1e6;
  // Recently opened first, then newest changes, like the quick switcher.
  items.sort((a, b) => a.rank - b.rank || b.modified - a.modified);
  const map = new Map<string, Target>();
  for (const item of items) {
    item.target.link = count.get(item.key)! > 1 ? item.target.label : item.name;
    map.set(item.target.path, item.target);
  }
  return map;
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
  for (const item of linkTargets().values()) {
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
