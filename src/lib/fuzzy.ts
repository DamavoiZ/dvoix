// Small fuzzy matcher for the quick switcher: "mtg plan" finds "Projekte/Meeting-Planung.md".
// Every word of the query must appear in order (letters may be spread out) in the path.
// Matches in the file name, at word starts and in a row score higher.

export interface FuzzyMatch {
  score: number;
  /** Indices of matched characters in the target string (sorted). */
  indices: number[];
}

const isBoundary = (s: string, i: number) => i === 0 || /[\s/\-_.()[\]]/.test(s[i - 1]) || (/[a-zäöü]/.test(s[i - 1]) && /[A-ZÄÖÜ]/.test(s[i]));

/** Matches one word as a subsequence of `target` starting at `from`; null if it doesn't fit. */
function matchWord(word: string, target: string, lower: string, from: number, jump = true): FuzzyMatch | null {
  const indices: number[] = [];
  let score = 0;
  let pos = from;
  for (let k = 0; k < word.length; k++) {
    const ch = word[k];
    // Prefer the next occurrence at a word start, if the plain next one isn't directly adjacent.
    let i = lower.indexOf(ch, pos);
    if (i < 0) return jump ? matchWord(word, target, lower, from, false) : null;
    const prev = indices[indices.length - 1];
    if (jump && (prev === undefined || i !== prev + 1)) {
      for (let j = i; j < lower.length; j++) {
        if (lower[j] === ch && isBoundary(target, j)) {
          i = j;
          break;
        }
      }
    }
    if (prev !== undefined && i === prev + 1) score += 6;
    if (isBoundary(target, i)) score += 4;
    score += 1;
    indices.push(i);
    pos = i + 1;
  }
  // Gaps cost a little, so tight matches win.
  score -= (indices[indices.length - 1] - indices[0] - word.length + 1) * 0.1;
  return { score, indices };
}

export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return { score: 0, indices: [] };
  const lower = target.toLowerCase();
  const nameStart = target.lastIndexOf("/") + 1;
  let score = 0;
  const all = new Set<number>();
  for (const word of words) {
    const inName = matchWord(word, target, lower, nameStart);
    const m = inName ?? matchWord(word, target, lower, 0);
    if (!m) return null;
    score += m.score + (inName ? 8 : 0);
    if (inName && inName.indices[0] === nameStart) score += 6;
    for (const i of m.indices) all.add(i);
  }
  // Shorter paths are usually what one means.
  score -= target.length * 0.02;
  return { score, indices: [...all].sort((a, b) => a - b) };
}

/** Splits `text` into plain and highlighted parts for rendering. */
export function highlight(text: string, indices: number[], offset = 0): { text: string; hit: boolean }[] {
  const set = new Set(indices);
  const parts: { text: string; hit: boolean }[] = [];
  for (let i = 0; i < text.length; i++) {
    const hit = set.has(i + offset);
    const last = parts[parts.length - 1];
    if (last && last.hit === hit) last.text += text[i];
    else parts.push({ text: text[i], hit });
  }
  return parts;
}
