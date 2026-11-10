// "Show this line": set by e.g. the task panel or the outline, picked up by
// the editor of that file.

interface RevealTarget {
  path: string;
  /** 0-based line. */
  line: number;
  /** The text of the heading, to scroll to it in the reading view. */
  heading?: string;
}

class RevealStore {
  target = $state<RevealTarget | null>(null);

  show(path: string, line: number, heading?: string) {
    this.target = { path, line, heading };
  }

  /** Returns the target for `path` (once), or null. */
  take(path: string): RevealTarget | null {
    const t = this.target;
    if (!t || t.path !== path) return null;
    this.target = null;
    return t;
  }

  /**
   * Returns the heading to show for `path` (once), or null. A target without
   * a heading stays: the editor may still show its line later.
   */
  takeHeading(path: string): string | null {
    const t = this.target;
    if (!t || t.path !== path || !t.heading) return null;
    this.target = null;
    return t.heading;
  }
}

export const reveal = new RevealStore();
