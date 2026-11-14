// Callout definitions shared between the reading view and CodeMirror live preview.

export type CalloutKind = "note" | "tip" | "important" | "warning" | "caution";

export interface CalloutInfo {
  kind: CalloutKind;
  defaultTitle: string;
}

const CALLOUT_ALIASES: Record<string, CalloutKind> = {
  note: "note",
  info: "note",
  abstract: "note",
  summary: "note",
  tldr: "note",
  tip: "tip",
  hint: "tip",
  success: "tip",
  check: "tip",
  done: "tip",
  important: "important",
  todo: "important",
  question: "important",
  help: "important",
  faq: "important",
  example: "important",
  warning: "warning",
  attention: "warning",
  caution: "caution",
  danger: "caution",
  error: "caution",
  fail: "caution",
  bug: "caution",
};

/** Resolves a callout keyword (e.g. `NOTE`, `TIP`, `WARNING`) to its visual kind and fallback title. */
export function resolveCallout(rawType: string): CalloutInfo {
  const lower = rawType.toLowerCase();
  const kind = CALLOUT_ALIASES[lower] ?? "note";
  const defaultTitle = lower.charAt(0).toUpperCase() + lower.slice(1);
  return { kind, defaultTitle };
}

const SVG_NS = "http://www.w3.org/2000/svg";

const ICON_PATHS: Record<CalloutKind, string[]> = {
  note: ["M12 16v-4", "M12 8h.01"],
  tip: [
    "M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5",
    "M9 18h6",
    "M10 22h4",
  ],
  important: [
    "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z",
    "M12 7v2",
    "M12 13h.01",
  ],
  warning: [
    "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3",
    "M12 9v4",
    "M12 17h.01",
  ],
  caution: [
    "M12 16h.01",
    "M12 8v4",
    "M15.312 2a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586l-4.688-4.688A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2z",
  ],
};

/** Creates a crisp 24x24 Lucide-style SVG icon element for a callout kind. */
export function createCalloutIcon(kind: CalloutKind): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  if (kind === "note") {
    const circle = document.createElementNS(SVG_NS, "circle");
    circle.setAttribute("cx", "12");
    circle.setAttribute("cy", "12");
    circle.setAttribute("r", "10");
    svg.appendChild(circle);
  }
  for (const d of ICON_PATHS[kind]) {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    svg.appendChild(path);
  }
  return svg;
}
