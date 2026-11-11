// Measures the work the live preview does on a scroll step: the OLD path
// rebuilds the decorations of the whole rendered window on every move of the
// viewport, the NEW path only decorates the lines that came into view and
// leaves the rest untouched until the scroll settles.
//
//   node scripts/bench-scroll.mjs
//
// The numbers are the ones in PERF.md. This models the decoration logic of
// `src/lib/filetypes/markdown/livePreview.ts` on the real markdown parser; the
// frame-time effect in the app itself (WebKitGTK re-measuring widget heights
// on every rebuilt decoration) cannot be measured without the webview.
import { EditorState, RangeSetBuilder } from "@codemirror/state";
import { Decoration, WidgetType } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";

const WIKILINK = /\[\[[^\]\n]+\]\]/g;

/** Stand-ins for the bullet and rule widgets: only the construction cost matters. */
class BulletWidget extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    return null;
  }
}
class RuleWidget extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    return null;
  }
}

/** A long note: headings, wrapped paragraphs, lists, code, tables, embeds. */
function makeNote(sections) {
  let doc = "---\ntitle: A long note\ntags: [bench, scroll]\n---\n";
  for (let i = 0; i < sections; i++) {
    doc += `## Section ${i}\n\n`;
    doc +=
      `Some **bold** text with a [[Note ${i}]] and \`inline code\`, long enough to wrap over a few lines of the editor when it is drawn at a normal width. `.repeat(2) +
      "\n\n";
    doc += "- a list item with *emphasis*\n- another item with a [link](https://example.com)\n- [ ] a task\n\n";
    doc += "```ts\nconst x = 1;\nconsole.log(x);\n```\n\n";
    doc += `| a | b |\n| --- | --- |\n| ${i} | value |\n\n`;
    if (i % 10 === 0) doc += `![[bench-image-${i % 3}.png]]\n\n`;
  }
  return doc;
}

/** The syntax of a window, collected the way `collectSyntax`/`collectWikilinks` do. */
function collect(state, ranges) {
  const hides = [];
  const bullets = [];
  const rules = [];
  const lineClasses = [];
  const marks = [];
  const images = [];
  const tree = syntaxTree(state);

  for (const range of ranges) {
    const text = state.doc.sliceString(range.from, range.to);
    for (const match of text.matchAll(WIKILINK)) {
      const start = range.from + match.index;
      const end = start + match[0].length;
      if (tree.resolveInner(start).name === "InlineCode") continue;
      const inner = match[0].slice(2, -2);
      if (start > range.from && state.doc.sliceString(start - 1, start) === "!") {
        images.push({ from: start - 1, to: end });
        hides.push({ from: start - 1, to: end, priority: 3 });
        continue;
      }
      const pipe = inner.indexOf("|");
      const label = pipe >= 0 ? inner.slice(pipe + 1) : inner;
      const offset = pipe >= 0 ? pipe + 1 : 0;
      const hash = label.indexOf("#");
      const shown = hash >= 0 ? label.slice(0, hash) : label;
      const visibleFrom = start + 2 + offset;
      hides.push({ from: start, to: visibleFrom, priority: 2 });
      hides.push({ from: visibleFrom + shown.length, to: end, priority: 2 });
    }

    tree.iterate({
      from: range.from,
      to: range.to,
      enter(node) {
        const { from, to, name } = node;
        const parent = node.node.parent?.name;
        const hide = (a, b, withSpace = false) => {
          const line = state.doc.lineAt(b);
          hides.push({ from: a, to: withSpace ? Math.min(b + 1, line.to) : b, priority: 1 });
        };
        if (name === "Table") return false;
        switch (name) {
          case "HeaderMark":
            hide(from, to, true);
            break;
          case "EmphasisMark":
          case "StrikethroughMark":
          case "CodeMark":
            hide(from, to);
            break;
          case "CodeInfo":
            marks.push({ from, to, class: "cm-lp-code-info" });
            break;
          case "QuoteMark":
            hide(from, to, true);
            break;
          case "ListMark": {
            const item = node.node.parent;
            if (item?.parent?.name === "OrderedList") marks.push({ from, to, class: "cm-lp-number" });
            else if (item?.getChild("Task")) hide(from, to, true);
            else bullets.push({ from, to, priority: 1 });
            break;
          }
          case "LinkMark":
            hide(from, to);
            break;
          case "URL":
            if (parent === "Link") hide(from, to);
            break;
          case "HorizontalRule":
            rules.push({ from, to, priority: 1 });
            break;
          case "Blockquote":
          case "FencedCode":
          case "CodeBlock":
          case "Frontmatter": {
            let pos = Math.max(from, range.from);
            const stop = Math.min(to, range.to);
            while (pos <= stop) {
              const line = state.doc.lineAt(pos);
              lineClasses.push({ from: line.from, class: "cm-lp-block" });
              if (line.to >= stop) break;
              pos = line.to + 1;
            }
            break;
          }
        }
      },
    });
  }

  // The cursor sits at the top in this bench, so no line is "active" and every
  // marker is hidden, exactly as during a scroll away from the cursor.
  const sorted = [...hides].sort((a, b) => a.from - b.from || b.to - a.to || b.priority - a.priority);
  const accepted = [];
  let end = -1;
  for (const h of sorted) {
    if (h.from < end) continue;
    accepted.push(h);
    end = h.to;
  }
  const replaces = [];
  const imageAt = (from, to) => images.find((i) => i.from === from && i.to === to);
  for (const h of accepted) replaces.push({ from: h.from, to: h.to, deco: Decoration.replace(imageAt(h.from, h.to) ? { widget: new RuleWidget() } : {}) });
  for (const b of bullets) replaces.push({ from: b.from, to: b.to, deco: Decoration.replace({ widget: new BulletWidget() }) });
  for (const r of rules) replaces.push({ from: r.from, to: r.to, deco: Decoration.replace({ widget: new RuleWidget() }) });
  for (const m of marks) replaces.push({ from: m.from, to: m.to, deco: Decoration.mark({ class: m.class }) });
  const lines = [];
  let last = -1;
  for (const l of [...lineClasses].sort((a, b) => a.from - b.from)) {
    if (l.from === last) continue;
    last = l.from;
    lines.push({ from: l.from, to: l.from, deco: Decoration.line({ class: l.class }) });
  }
  const entries = [...lines, ...replaces].sort((a, b) => a.from - b.from || a.to - b.to);
  const builder = new RangeSetBuilder();
  for (const e of entries) builder.add(e.from, e.to, e.deco);
  return builder.finish();
}

/** The lines of a window, widened to whole lines. */
function windowLines(state, from, to) {
  return [{ from: state.doc.lineAt(from).from, to: state.doc.lineAt(to).to }];
}

/** Sorts ranges and joins the ones that touch or overlap. */
function mergeRanges(ranges) {
  const sorted = [...ranges].sort((a, b) => a.from - b.from);
  const out = [];
  for (const r of sorted) {
    const last = out[out.length - 1];
    if (last && r.from <= last.to + 1) last.to = Math.max(last.to, r.to);
    else out.push({ ...r });
  }
  return out;
}

/** The part of `range` the covered ranges do not cover, widened to whole lines. */
function uncovered(state, range, covered) {
  let rest = [range];
  for (const old of covered) {
    const next = [];
    for (const r of rest) {
      if (old.to < r.from || old.from > r.to) next.push(r);
      else {
        if (r.from < old.from) next.push({ from: r.from, to: old.from - 1 });
        if (r.to > old.to) next.push({ from: old.to + 1, to: r.to });
      }
    }
    rest = next;
  }
  return rest.map((r) => ({ from: state.doc.lineAt(r.from).from, to: state.doc.lineAt(r.to).to }));
}

/** Times `fn` over `iters` runs and returns the mean in milliseconds. */
function time(fn, iters) {
  // Warm up so the parser and the JIT are not part of the measurement.
  for (let i = 0; i < 5; i++) fn();
  const t = performance.now();
  for (let i = 0; i < iters; i++) fn();
  return (performance.now() - t) / iters;
}

function bench(sections) {
  const state = EditorState.create({ doc: makeNote(sections), extensions: [markdown({ base: markdownLanguage })] });
  const doc = state.doc;
  // A rendered window of ~160 lines, the size CodeMirror keeps around the
  // viewport. A scroll burst travels ~30 steps of 8 lines (about two screens)
  // before it settles; the settle resets the coverage to the viewport. The
  // bench repeats that burst down the note, the way a long read scrolls.
  const WINDOW = 160;
  const STEP = 8;
  const BURST = 30;
  const win = (s) => windowLines(state, doc.line(Math.min(doc.lines, 1 + s * STEP)).from, doc.line(Math.min(doc.lines, 1 + s * STEP + WINDOW)).to);
  const maxStep = Math.floor((doc.lines - WINDOW) / STEP);

  // OLD: every step rebuilds the whole window.
  const oldMs = time(() => {
    for (let s = 0; s <= maxStep; s++) collect(state, win(s));
  }, 3);

  // NEW: a step decorates only the lines that came into view and joins them
  // into the kept set; every BURST steps (the settle) the set is rebuilt for
  // the viewport alone, dropping what scrolled away.
  const newMs = time(() => {
    let covered = win(0);
    let set = collect(state, covered);
    for (let s = 1; s <= maxStep; s++) {
      if (s % BURST === 0) {
        covered = win(s);
        set = collect(state, covered);
        continue;
      }
      const fresh = uncovered(state, win(s)[0], covered);
      if (fresh.length) {
        const added = [];
        collect(state, fresh).between(0, doc.length, (from, to, value) => added.push({ from, to, value }));
        set = set.update({ add: added });
        covered = mergeRanges([...covered, ...fresh]);
      }
    }
  }, 3);

  console.log(
    `${String(sections).padStart(5)} sections (${doc.lines} lines): full-window rebuild ${oldMs.toFixed(2)} ms | incremental cover ${newMs.toFixed(2)} ms | ${(oldMs / newMs).toFixed(1)}x`,
  );
}

for (const sections of [200, 400, 1500]) bench(sections);
