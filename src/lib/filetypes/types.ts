import type { Component } from "svelte";

/** Props every file-type view receives. */
export interface FileViewProps {
  /** Vault-relative path of the open file. */
  path: string;
  /** Current file content as text. */
  content: string;
  /** Call with the new content whenever the user edits; the app saves it. */
  onchange: (content: string) => void;
  /** Open another file of the vault (e.g. when following a link). */
  onopen: (path: string) => void;
}

/**
 * A file type is one self-contained module: which extensions it handles,
 * how a new file of that type looks, and the view that edits it.
 * Adding a feature like Kanban boards means adding one of these.
 */
export interface FileType {
  /** Stable id, e.g. "markdown" or "canvas". */
  id: string;
  /** Name shown in the UI, e.g. "Note". */
  label: string;
  /** Lower-case extensions without dot, e.g. ["md"]. */
  extensions: string[];
  /** Icon for menus and buttons (any Svelte component, e.g. from @lucide/svelte). */
  icon: Component;
  /** The view that shows and edits the file. */
  view: Component<FileViewProps>;
  /** Content for a freshly created file. Omit if users can't create this type from the app. */
  template?: (name: string) => string;
}
