import type { FileType } from "./types";

const byId = new Map<string, FileType>();
const byExtension = new Map<string, FileType>();
let fallback: FileType | undefined;

export function registerFileType(type: FileType): void {
  if (byId.has(type.id)) throw new Error(`File type "${type.id}" is already registered`);
  byId.set(type.id, type);
  for (const ext of type.extensions) byExtension.set(ext.toLowerCase(), type);
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
