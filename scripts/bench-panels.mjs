// Measures the costs of the panels: the lookups of links, the tree of the
// vault and the suggestions of links.
//
//   node scripts/bench-panels.mjs
//
// The numbers are the ones in PERF.md.
import { buildTree, mergeEntries, FOLDER_PAGE, visibleChildren } from "../src/lib/features/file-explorer/tree.ts";

/** A vault of 20 000 files in 100 folders. */
function makeEntries(count, folders) {
  const entries = [];
  for (let i = 0; i < folders; i++) entries.push({ path: `folder-${i}`, name: `folder-${i}`, extension: "", isDir: true, size: 0, modified: 0 });
  for (let i = 0; i < count; i++) {
    const folder = `folder-${i % folders}`;
    const name = `note-${i}`;
    entries.push({
      path: `${folder}/${name}.md`,
      name: `${name}.md`,
      extension: "md",
      isDir: false,
      size: 2000,
      modified: 1700000000000 + i,
    });
  }
  return entries;
}

// ---------------------------------------------------------------- resolveLink

/** OLD: the entries are walked for every lookup. */
function resolveLinkOld(entries, target) {
  const files = entries.filter((e) => !e.isDir);
  const t = target.toLowerCase();
  const exact = files.find((f) => f.path.toLowerCase() === t || f.path.toLowerCase() === `${t}.md`);
  if (exact) return exact.path;
  const byName = files.find((f) => {
    const name = f.name.toLowerCase();
    return name === t || name === `${t}.md`;
  });
  return byName?.path;
}

/** NEW: the maps are built once per change of the list. */
function buildMaps(entries) {
  const byPath = new Map();
  const byName = new Map();
  for (const entry of entries) {
    if (entry.isDir) continue;
    byPath.set(entry.path.toLowerCase(), entry);
    if (!byName.has(entry.name.toLowerCase())) byName.set(entry.name.toLowerCase(), entry);
  }
  return { byPath, byName };
}

function resolveLinkNew(maps, target) {
  const t = target.toLowerCase();
  const exact = maps.byPath.get(t) ?? maps.byPath.get(`${t}.md`);
  if (exact) return exact.path;
  return (maps.byName.get(t) ?? maps.byName.get(`${t}.md`))?.path;
}

function benchResolveLink(entries) {
  const targets = ["note-19999", "note-19999.md", "folder-3/note-5", "nothing"];
  const n = 200;
  let t = performance.now();
  for (let i = 0; i < n; i++) for (const target of targets) resolveLinkOld(entries, target);
  const old = (performance.now() - t) / (n * targets.length);
  const maps = buildMaps(entries);
  t = performance.now();
  for (let i = 0; i < n; i++) for (const target of targets) resolveLinkNew(maps, target);
  const now = (performance.now() - t) / (n * targets.length);
  console.log(`  lookup of a link: old ${old.toFixed(4)} ms | new ${now.toFixed(4)} ms`);
  t = performance.now();
  for (let i = 0; i < 20; i++) buildMaps(entries);
  console.log(`  building the maps once: ${((performance.now() - t) / 20).toFixed(3)} ms`);
}

// ---------------------------------------------------------------- the tree

/** OLD: the names are compared with a collator that is built for every call. */
function buildTreeOld(entries) {
  const root = [];
  const folders = new Map();
  for (const entry of entries) {
    const node = { entry, children: [] };
    if (entry.isDir) folders.set(entry.path, node);
    const parent = entry.path.includes("/") ? entry.path.slice(0, entry.path.lastIndexOf("/")) : "";
    (parent ? folders.get(parent)?.children : root)?.push(node);
  }
  const sort = (nodes) => {
    nodes.sort((a, b) => Number(b.entry.isDir) - Number(a.entry.isDir) || a.entry.name.localeCompare(b.entry.name, "en", { numeric: true }));
    nodes.forEach((n) => sort(n.children));
  };
  sort(root);
  return root;
}

/** OLD: the key of the sort is built twice for every comparison. */
function mergeEntriesOld(entries, extra) {
  if (extra.length === 0) return entries;
  const seen = new Set(entries.map((e) => e.path));
  const merged = [...entries, ...extra.filter((e) => !seen.has(e.path))];
  return merged.sort((a, b) => (a.path.toLowerCase() < b.path.toLowerCase() ? -1 : a.path.toLowerCase() > b.path.toLowerCase() ? 1 : 0));
}

function benchTree(entries, extra) {
  const n = 20;
  let t = performance.now();
  for (let i = 0; i < n; i++) buildTreeOld(entries);
  const old = (performance.now() - t) / n;
  t = performance.now();
  for (let i = 0; i < n; i++) buildTree(entries);
  const now = (performance.now() - t) / n;
  console.log(`  building the tree: old ${old.toFixed(2)} ms | new ${now.toFixed(2)} ms`);
  t = performance.now();
  for (let i = 0; i < n; i++) mergeEntriesOld(entries, extra);
  const oldMerge = (performance.now() - t) / n;
  t = performance.now();
  for (let i = 0; i < n; i++) mergeEntries(entries, extra);
  const newMerge = (performance.now() - t) / n;
  console.log(`  adding the entries of the plugins: old ${oldMerge.toFixed(2)} ms | new ${newMerge.toFixed(2)} ms`);
}

// ---------------------------------------------------------------- the link suggestions

/** OLD: the targets are built for every suggestion. */
function linkTextsOld(entries, recent) {
  const files = entries.filter((e) => !e.isDir);
  const count = new Map();
  const nameOf = (path, isNote) => {
    const name = path.split("/").pop() ?? path;
    return isNote ? name.replace(/\.md$/i, "") : name;
  };
  const items = files.map((f) => {
    const isNote = f.path.endsWith(".md");
    const label = isNote ? f.path.replace(/\.md$/i, "") : f.path;
    const name = nameOf(f.path, isNote).toLowerCase();
    count.set(name, (count.get(name) ?? 0) + 1);
    return { path: f.path, label, name, isNote, modified: f.modified };
  });
  const rank = new Map(recent.map((p, i) => [p, i]));
  items.sort((a, b) => (rank.get(a.path) ?? 1e6) - (rank.get(b.path) ?? 1e6) || b.modified - a.modified);
  return items.map((i) => ({
    path: i.path,
    label: i.label,
    isNote: i.isNote,
    link: count.get(i.name) > 1 ? i.label : nameOf(i.path, i.isNote),
  }));
}

/** NEW: the targets are built once per change of the list. */
function buildTargets(entries, recent) {
  const count = new Map();
  const items = [];
  const nameOf = (path, isNote) => {
    const slash = path.lastIndexOf("/");
    const name = slash >= 0 ? path.slice(slash + 1) : path;
    return isNote ? name.replace(/\.md$/i, "") : name;
  };
  for (const entry of entries) {
    if (entry.isDir) continue;
    const isNote = entry.path.endsWith(".md");
    const label = isNote ? entry.path.replace(/\.md$/i, "") : entry.path;
    const name = nameOf(entry.path, isNote);
    const key = name.toLowerCase();
    count.set(key, (count.get(key) ?? 0) + 1);
    items.push({ target: { path: entry.path, label, link: "", isNote }, name, key, rank: 0, modified: entry.modified });
  }
  const rank = new Map(recent.map((p, i) => [p, i]));
  for (const item of items) item.rank = rank.get(item.target.path) ?? 1e6;
  items.sort((a, b) => a.rank - b.rank || b.modified - a.modified);
  const map = new Map();
  for (const item of items) {
    item.target.link = count.get(item.key) > 1 ? item.target.label : item.name;
    map.set(item.target.path, item.target);
  }
  return map;
}

function benchLinkTargets(entries, recent) {
  const n = 20;
  let t = performance.now();
  for (let i = 0; i < n; i++) linkTextsOld(entries, recent);
  const old = (performance.now() - t) / n;
  t = performance.now();
  for (let i = 0; i < n; i++) buildTargets(entries, recent);
  const now = (performance.now() - t) / n;
  console.log(`  building the targets: old ${old.toFixed(2)} ms | new ${now.toFixed(2)} ms (once per change of the vault)`);
  // The suggestion itself: the targets are there, the fuzzy match is the work.
  const map = buildTargets(entries, recent);
  t = performance.now();
  for (let i = 0; i < n; i++) for (const target of map.values()) target.label.includes("note");
  console.log(`  walking the targets for a suggestion: ${((performance.now() - t) / n).toFixed(3)} ms`);
}

// ---------------------------------------------------------------- the task filter

/** A vault with many checkbox tasks, as the tasks panel gets them. */
function makeTasks(count) {
  const tasks = [];
  for (let i = 0; i < count; i++) {
    tasks.push({
      path: `folder-${i % 100}/note-${i % 500}.md`,
      noteTitle: `Note ${i % 500}`,
      line: i % 40,
      done: i % 3 === 0,
      text: `Write the report for client ${i} and send it out`,
      due: i % 4 === 0 ? "2026-11-20" : null,
      priority: null,
      tags: ["work", `client-${i % 20}`],
      indent: 0,
    });
  }
  return tasks;
}

/** OLD: the text of every task is lower-cased again for every query. */
function filterTasksOld(tasks, q) {
  return tasks.filter(
    (t) =>
      t.text.toLowerCase().includes(q) || t.noteTitle.toLowerCase().includes(q) || t.tags.some((tag) => tag.includes(q.replace(/^#/, ""))),
  );
}

/** NEW: the search text of every task is built once per load. */
function taskHaystacks(tasks) {
  return tasks.map((t) => `${t.text}\n${t.noteTitle}\n${t.tags.map((tag) => `#${tag}`).join(" ")}`.toLowerCase());
}

function filterTasksNew(tasks, haystacks, q) {
  const plain = q.replace(/^#/, "");
  return tasks.filter((t, i) => haystacks[i].includes(q) || (plain !== q && haystacks[i].includes(`#${plain}`)));
}

function benchTaskFilter(tasks) {
  const queries = ["report", "client-1", "nothing", "#work"];
  const n = 50;
  let t = performance.now();
  for (let i = 0; i < n; i++) for (const q of queries) filterTasksOld(tasks, q);
  const old = (performance.now() - t) / (n * queries.length);
  const haystacks = taskHaystacks(tasks);
  t = performance.now();
  for (let i = 0; i < n; i++) for (const q of queries) filterTasksNew(tasks, haystacks, q);
  const now = (performance.now() - t) / (n * queries.length);
  console.log(`  filter a query: old ${old.toFixed(3)} ms | new ${now.toFixed(3)} ms`);
  t = performance.now();
  for (let i = 0; i < n; i++) taskHaystacks(tasks);
  console.log(`  building the search texts once: ${((performance.now() - t) / n).toFixed(2)} ms`);
  // Both ways must find the same tasks.
  for (const q of queries) {
    const a = filterTasksOld(tasks, q).length;
    const b = filterTasksNew(tasks, haystacks, q).length;
    if (a !== b) throw new Error(`the filters disagree on "${q}": ${a} vs ${b}`);
  }
}

// ---------------------------------------------------------------- big folders

function benchFolderWindow(children) {
  const n = 200;
  let t = performance.now();
  for (let i = 0; i < n; i++) children.map((child) => child.entry.path);
  const all = (performance.now() - t) / n;
  t = performance.now();
  for (let i = 0; i < n; i++) visibleChildren(children, FOLDER_PAGE).map((child) => child.entry.path);
  const page = (performance.now() - t) / n;
  console.log(`  keys of the rows to draw: all ${all.toFixed(3)} ms | first page ${page.toFixed(3)} ms`);
  if (visibleChildren(children, FOLDER_PAGE).length !== FOLDER_PAGE) throw new Error("the page is not full");
  if (visibleChildren(children.slice(0, 10), FOLDER_PAGE).length !== 10) throw new Error("a small folder is cut");
}

const entries = makeEntries(20000, 100);
const extra = [{ path: ".hidden/notes.md", name: "notes.md", extension: "md", isDir: false, size: 10, modified: 1 }];
const recent = ["folder-3/note-5.md", "folder-7/note-9.md"];

console.log(`a vault of ${entries.length} entries:`);
console.log("the lookup of a link:");
benchResolveLink(entries);
console.log("the tree of the vault:");
benchTree(entries, extra);
console.log("the suggestions of links:");
benchLinkTargets(entries, recent);
console.log("the task filter (5000 tasks):");
benchTaskFilter(makeTasks(5000));
console.log("a folder with 5000 entries:");
benchFolderWindow(buildTree(makeEntries(5000, 1))[0].children);
