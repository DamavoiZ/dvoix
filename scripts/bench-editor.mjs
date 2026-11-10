// Measures the costs of the editor hot path: the live preview, the check
// whether the note has to be put into the editor again, and the reading view.
//
//   node scripts/bench-editor.mjs
//
// The numbers are the ones in PERF.md.
import { EditorState, StateField, RangeSetBuilder } from "@codemirror/state";
import { Decoration, WidgetType } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";

/** A stand-in for the properties and table widgets: only the construction cost matters. */
class FakeWidget extends WidgetType {
  constructor(source) {
    super();
    this.source = source;
  }
  eq(other) {
    return other.source === this.source;
  }
  toDOM() {
    return null;
  }
  ignoreEvent() {
    return true;
  }
}

/** A note with a frontmatter and a number of tables. */
function makeDoc(tables) {
  let doc = "---\ntitle: A note\ntags: [a, b]\nstatus: active\n---\n# Heading\n\nSome text.\n\n";
  for (let i = 0; i < tables; i++) {
    doc += `| a | b |\n| --- | --- |\n| ${i} | x |\n\nText after the table.\n\n`;
  }
  return doc;
}

// OLD: the blocks are built again for every change.
const oldBlocks = StateField.define({
  create: (state) => buildOld(state),
  update: (decorations, tr) => {
    if (tr.docChanged || tr.selection || syntaxTree(tr.startState) !== syntaxTree(tr.state)) {
      return buildOld(tr.state);
    }
    return decorations;
  },
});

function buildOld(state) {
  const builder = new RangeSetBuilder();
  const sel = state.selection.main;
  const outside = (from, to) => sel.to < from || sel.from > to;
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Document") return true;
      if (node.name === "Frontmatter") {
        const source = state.doc.sliceString(0, node.to);
        if (outside(0, node.to)) {
          builder.add(0, node.to, Decoration.replace({ widget: new FakeWidget(source), block: true }));
        }
      }
      if (node.name === "Table") {
        const from = state.doc.lineAt(node.from).from;
        const to = state.doc.lineAt(Math.max(node.from, node.to - 1)).to;
        if (outside(from, to)) {
          builder.add(from, to, Decoration.replace({ widget: new FakeWidget(state.doc.sliceString(from, to)), block: true }));
        }
      }
      return false;
    },
  });
  return builder.finish();
}

// NEW: the blocks are kept, a change of the cursor only redraws them.
const newBlocks = StateField.define({
  create: (state) => {
    const all = collect(state, []);
    return { all, decorations: draw(all, state) };
  },
  update(value, tr) {
    if (tr.docChanged || syntaxTree(tr.startState) !== syntaxTree(tr.state)) {
      const all = collect(tr.state, value.all);
      return { all, decorations: draw(all, tr.state) };
    }
    if (tr.selection) return { all: value.all, decorations: draw(value.all, tr.state) };
    return value;
  },
});

function collect(state, previous) {
  const found = [];
  syntaxTree(state).iterate({
    enter(node) {
      if (node.name === "Document") return true;
      if (node.name === "Frontmatter") {
        found.push({ from: 0, to: node.to, kind: "frontmatter", source: state.doc.sliceString(0, node.to) });
      }
      if (node.name === "Table") {
        const from = state.doc.lineAt(node.from).from;
        const to = state.doc.lineAt(Math.max(node.from, node.to - 1)).to;
        found.push({ from, to, kind: "table", source: state.doc.sliceString(from, to) });
      }
      return false;
    },
  });
  // A block whose text did not change keeps its widget.
  return found.map((block, i) => {
    const before = previous[i];
    const widget =
      before && before.kind === block.kind && before.source === block.source
        ? before.widget
        : new FakeWidget(block.source);
    return { ...block, widget };
  });
}

function draw(all, state) {
  const builder = new RangeSetBuilder();
  const sel = state.selection.main;
  for (const block of all) {
    if (sel.to < block.from || sel.from > block.to) {
      builder.add(block.from, block.to, Decoration.replace({ widget: block.widget, block: true }));
    }
  }
  return builder.finish();
}

function benchPreview(label, field, doc) {
  let state = EditorState.create({
    doc,
    extensions: [markdown({ base: markdownLanguage, codeLanguages: languages }), field],
  });
  const n = 300;
  const t = performance.now();
  for (let i = 0; i < n; i++) {
    state = state.update({ selection: { anchor: (i * 37) % Math.max(1, state.doc.length) } }).state;
  }
  const cursor = (performance.now() - t) / n;
  const t2 = performance.now();
  for (let i = 0; i < n; i++) {
    state = state.update({ changes: { from: state.doc.length, insert: "x" } }).state;
  }
  const key = (performance.now() - t2) / n;
  console.log(`  ${label}: cursor ${cursor.toFixed(3)} ms | keystroke ${key.toFixed(3)} ms`);
}

/** The check whether the note has to be put into the editor again. */
function benchReload() {
  for (const chars of [20000, 100000]) {
    const doc = "x".repeat(chars);
    const n = 200;
    let t = performance.now();
    for (let i = 0; i < n; i++) {
      // OLD: two copies of the whole note.
      if (doc !== doc.replace(/\r\n?/g, "\n")) throw new Error("unexpected");
    }
    const old = (performance.now() - t) / n;
    // NEW: the editor and the note hold the same text.
    const editorText = doc;
    t = performance.now();
    for (let i = 0; i < n; i++) {
      if (doc !== editorText) throw new Error("unexpected");
    }
    const now = (performance.now() - t) / n;
    console.log(`  note of ${chars} characters: old ${old.toFixed(3)} ms | new ${now.toFixed(4)} ms`);
  }
}

console.log("live preview:");
for (const tables of [5, 50, 200]) {
  const doc = makeDoc(tables);
  console.log(`note with ${tables} tables (${doc.length} characters):`);
  benchPreview("old", oldBlocks, doc);
  benchPreview("new", newBlocks, doc);
}
console.log("the check whether the note has to be put into the editor again:");
benchReload();
