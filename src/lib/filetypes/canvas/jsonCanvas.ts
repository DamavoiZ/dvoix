// Types and helpers for the open JSON Canvas format (https://jsoncanvas.org),
// the same format Obsidian uses for `.canvas` files.

export type CanvasColor = string; // "1".."6" preset or "#rrggbb"
export type Side = "top" | "right" | "bottom" | "left";

interface NodeBase {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  color?: CanvasColor;
}

export interface TextNode extends NodeBase { type: "text"; text: string }
export interface FileNode extends NodeBase { type: "file"; file: string; subpath?: string }
export interface LinkNode extends NodeBase { type: "link"; url: string }
export interface GroupNode extends NodeBase { type: "group"; label?: string }

export type CanvasNode = TextNode | FileNode | LinkNode | GroupNode;

export interface CanvasEdge {
  id: string;
  fromNode: string;
  toNode: string;
  fromSide?: Side;
  toSide?: Side;
  color?: CanvasColor;
  label?: string;
}

export interface JsonCanvas {
  nodes: CanvasNode[];
  edges: CanvasEdge[];
}

export function parseCanvas(text: string): JsonCanvas {
  if (!text.trim()) return { nodes: [], edges: [] };
  const data = JSON.parse(text) as Partial<JsonCanvas>;
  return { nodes: data.nodes ?? [], edges: data.edges ?? [] };
}

export function serializeCanvas(canvas: JsonCanvas): string {
  return JSON.stringify(canvas, null, "\t") + "\n";
}

export function newId(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(8)), (b) => b.toString(16).padStart(2, "0")).join("");
}

const PRESETS: Record<string, string> = {
  "1": "#ef4444",
  "2": "#f97316",
  "3": "#eab308",
  "4": "#22c55e",
  "5": "#06b6d4",
  "6": "#a855f7",
};

export function colorOf(color?: CanvasColor): string | undefined {
  if (!color) return undefined;
  return PRESETS[color] ?? color;
}
