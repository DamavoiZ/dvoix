// "Show this line": set by e.g. the task panel, picked up by the editor of that file.

interface RevealTarget {
  path: string;
  /** 0-based line. */
  line: number;
}

class RevealStore {
  target = $state<RevealTarget | null>(null);

  show(path: string, line: number) {
    this.target = { path, line };
  }

  /** Returns the line to show for `path` (once), or null. */
  take(path: string): number | null {
    const t = this.target;
    if (!t || t.path !== path) return null;
    this.target = null;
    return t.line;
  }
}

export const reveal = new RevealStore();
