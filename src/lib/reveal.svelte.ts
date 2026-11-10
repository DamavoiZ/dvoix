// "Show this line": set by e.g. the task panel or the outline, picked up by
// the editor of that file.

interface RevealTarget {
  path: string;
  /** 0-based line. */
  line: number;
  /** The text of the heading, to scroll to it in the reading view. */
  heading?: string;
  /** Which heading of the note it is (0-based), when several have the same text. */
  headingIndex?: number;
}

class RevealStore {
  target = $state<RevealTarget | null>(null);

  show(path: string, line: number, heading?: string, headingIndex?: number) {
    this.target = { path, line, heading, headingIndex };
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
  takeHeading(path: string): { text: string; index?: number } | null {
    const t = this.target;
    if (!t || t.path !== path || !t.heading) return null;
    this.target = null;
    return { text: t.heading, index: t.headingIndex };
  }
}

export const reveal = new RevealStore();
