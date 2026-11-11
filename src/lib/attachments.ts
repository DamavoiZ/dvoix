// Attachments: images and other files that notes embed with `![[file]]`.
// Shared by the markdown views (showing embeds) and the attachments feature
// (saving pasted and dropped files). The webview cannot read vault files
// itself, so the bytes come through the Rust core and become blob URLs.
import { api } from "./api";
import { vault } from "./vault.svelte";

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "ico"]);
const MIME: Record<string, string> = { svg: "image/svg+xml", jpg: "image/jpeg", ico: "image/x-icon" };

export function extensionOf(path: string): string {
  const name = path.split("/").pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function isImage(path: string): boolean {
  return IMAGE_EXTENSIONS.has(extensionOf(path));
}

/**
 * Splits the inside of `![[...]]` into the file and an optional width
 * (`![[photo.png|300]]`, like in Obsidian).
 */
export function parseEmbed(inner: string): { target: string; width?: number } {
  const pipe = inner.indexOf("|");
  const target = (pipe >= 0 ? inner.slice(0, pipe) : inner).split("#")[0].trim();
  const size = pipe >= 0 ? /^\s*(\d+)(?:x\d+)?\s*$/.exec(inner.slice(pipe + 1)) : null;
  return { target, width: size ? Number(size[1]) : undefined };
}

// One blob URL per file and version; a changed file (same path, newer
// modification time) gets a new URL and the old one is released.
const cache = new Map<string, { modified: number; url: Promise<string> }>();

/** A URL the webview can show for an image in the vault, or null if it is missing. */
export async function attachmentUrl(target: string): Promise<string | null> {
  const path = vault.resolveLink(target);
  if (!path) return null;
  const modified = vault.entryAt(path)?.modified ?? 0;
  const hit = cache.get(path);
  if (hit && hit.modified === modified) return hit.url;
  if (hit) void hit.url.then((u) => URL.revokeObjectURL(u), () => {});
  const ext = extensionOf(path);
  const url = api
    .readBinary(path)
    .then((bytes) => URL.createObjectURL(new Blob([bytes], { type: MIME[ext] ?? `image/${ext}` })));
  cache.set(path, { modified, url });
  url.catch(() => cache.delete(path));
  return url.catch(() => null);
}

/** An `<img>` for an embedded image; it shows the file name until (or if not) loaded. */
export function embedImage(target: string, width?: number): HTMLElement {
  const img = document.createElement("img");
  img.className = "dvoix-embed-image";
  img.alt = target;
  img.draggable = false;
  if (width) img.style.width = `${width}px`;
  void attachmentUrl(target).then((url) => {
    if (url) img.src = url;
    else {
      img.replaceWith(Object.assign(document.createElement("span"), { className: "dvoix-embed-missing", textContent: target }));
    }
  });
  return img;
}

/** A folder path as typed by the user, made vault-relative: no leading, trailing or double slashes. */
export function cleanFolder(text: string): string {
  return text
    .replace(/\\/g, "/")
    .split("/")
    .map((s) => s.trim())
    .filter((s) => s && s !== "." && s !== "..")
    .join("/");
}

/** The folder of a vault path ("" for the root). */
export function folderOf(path: string | null): string {
  return path && path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
}
