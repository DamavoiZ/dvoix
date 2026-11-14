// Live cursor position reported by the active text editor for status bar features.
class EditorStatusStore {
  cursor = $state<{ line: number; col: number } | null>(null);

  setCursor(line: number, col: number) {
    if (this.cursor?.line === line && this.cursor?.col === col) return;
    this.cursor = { line, col };
  }

  clear() {
    this.cursor = null;
  }
}

export const editorStatus = new EditorStatusStore();
