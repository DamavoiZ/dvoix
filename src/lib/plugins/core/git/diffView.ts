// The CodeMirror views behind the git diff: old and new version side by side
// with one shared scrollbar, or both in one column with the removed lines
// shown above the added ones. Both are read-only.
import { MergeView, unifiedMergeView } from "@codemirror/merge";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";

export type DiffLayout = "split" | "inline";

/** Unchanged runs longer than this are folded, with a few lines of context kept. */
const collapseUnchanged = { margin: 3, minSize: 6 };

const theme = EditorView.theme(
  {
    "&": { color: "var(--color-zinc-200)", backgroundColor: "transparent", fontSize: "12px" },
    ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.6" },
    ".cm-content": { padding: "0.5rem 0" },
    ".cm-gutters": { backgroundColor: "var(--color-zinc-900)", border: "none", color: "var(--color-zinc-600)" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
      backgroundColor: "color-mix(in oklab, var(--color-violet-400) 30%, transparent) !important",
    },
    // Side by side: the left editor holds the old version.
    "&.cm-merge-a .cm-changedLine, .cm-deletedChunk": { backgroundColor: "var(--diff-delete-line)" },
    "&.cm-merge-a .cm-changedText, .cm-deletedChunk .cm-deletedText": {
      background: "var(--diff-delete-text)",
    },
    "&.cm-merge-b .cm-changedLine, .cm-inlineChangedLine": { backgroundColor: "var(--diff-insert-line)" },
    "&.cm-merge-b .cm-changedText, .cm-inlineChangedLine .cm-changedText": {
      background: "var(--diff-insert-text)",
    },
    "&.cm-merge-a .cm-changedLineGutter, .cm-deletedLineGutter": { backgroundColor: "var(--diff-delete-gutter)" },
    "&.cm-merge-b .cm-changedLineGutter, .cm-inlineChangedLineGutter": { backgroundColor: "var(--diff-insert-gutter)" },
    ".cm-collapsedLines": {
      color: "var(--diff-collapsed)",
      background: "var(--color-zinc-900)",
      borderTop: "1px solid var(--color-zinc-800)",
      borderBottom: "1px solid var(--color-zinc-800)",
    },
  },
  { dark: true },
);

/** Everything both sides share: read-only, line numbers, wrapped lines, the theme. */
function base(): Extension[] {
  return [
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
    EditorView.lineWrapping,
    lineNumbers(),
    EditorState.phrases.of({ "$ unchanged lines": "$ unveränderte Zeilen" }),
    theme,
  ];
}

/** Something that can be taken down again when the diff closes or the layout changes. */
export interface DiffHandle {
  destroy(): void;
}

/** Shows the diff of `oldText` and `newText` in `parent`. */
export function createDiffView(parent: HTMLElement, oldText: string, newText: string, layout: DiffLayout): DiffHandle {
  if (layout === "split") {
    const view = new MergeView({
      parent,
      a: { doc: oldText, extensions: base() },
      b: { doc: newText, extensions: base() },
      gutter: true,
      highlightChanges: true,
      collapseUnchanged,
    });
    // Both editors sit in one scroll container, so they always scroll together.
    view.dom.style.height = "100%";
    return view;
  }
  return new EditorView({
    parent,
    state: EditorState.create({
      doc: newText,
      extensions: [
        base(),
        unifiedMergeView({
          original: oldText,
          gutter: true,
          highlightChanges: true,
          mergeControls: false,
          collapseUnchanged,
        }),
      ],
    }),
  });
}
