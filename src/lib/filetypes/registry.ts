import { SvelteMap } from "svelte/reactivity";
import type { FileType } from "./types";

// Reactive, so views and file icons update when a plugin adds or removes a type.
const byId = new SvelteMap<string, FileType>();
const byExtension = new SvelteMap<string, FileType>();
let fallback: FileType | undefined;

/** Registers a file type; returns a function that removes it again. */
export function registerFileType(type: FileType): () => void {
  if (byId.has(type.id)) throw new Error(`File type "${type.id}" is already registered`);
  byId.set(type.id, type);
  for (const ext of type.extensions) byExtension.set(ext.toLowerCase(), type);
  return () => {
    if (byId.get(type.id) !== type) return;
    byId.delete(type.id);
    for (const ext of type.extensions) if (byExtension.get(ext.toLowerCase()) === type) byExtension.delete(ext.toLowerCase());
  };
}

/** The type used for files no other type claims (shows a hint, no editing). */
export function setFallbackFileType(type: FileType): void {
  fallback = type;
}

export function extensionOf(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function fileTypeFor(path: string): FileType | undefined {
  return byExtension.get(extensionOf(path)) ?? fallback;
}

export function isKnownFile(path: string): boolean {
  return byExtension.has(extensionOf(path));
}

/** Types users can create new files of (they have a template). */
export function creatableFileTypes(): FileType[] {
  return [...byId.values()].filter((t) => t.template);
}
