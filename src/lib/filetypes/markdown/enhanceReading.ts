// Post-processes rendered markdown HTML in reading views: turns `[!TYPE]`
// blockquotes into styled callouts and adds a copy button to fenced code blocks.
import { createCalloutIcon, resolveCallout } from "./callouts";

const SVG_NS = "http://www.w3.org/2000/svg";
const CALLOUT_START = /^\s*\[!([a-zA-Z-]+)\][ \t]*/;

function makeSvg(paths: string[], rect?: { x: string; y: string; width: string; height: string; rx: string }): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "13");
  svg.setAttribute("height", "13");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  if (rect) {
    const r = document.createElementNS(SVG_NS, "rect");
    r.setAttribute("x", rect.x);
    r.setAttribute("y", rect.y);
    r.setAttribute("width", rect.width);
    r.setAttribute("height", rect.height);
    r.setAttribute("rx", rect.rx);
    svg.appendChild(r);
  }
  for (const d of paths) {
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", d);
    svg.appendChild(p);
  }
  return svg;
}

function copyIcon(): SVGSVGElement {
  return makeSvg(
    ["M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"],
    { x: "8", y: "8", width: "14", height: "14", rx: "2" },
  );
}

function checkIcon(): SVGSVGElement {
  return makeSvg(["M20 6 9 17l-5-5"]);
}

function enhanceCallouts(root: HTMLElement): void {
  for (const bq of root.querySelectorAll<HTMLElement>("blockquote")) {
    if (bq.classList.contains("callout")) continue;
    const first = bq.firstElementChild;
    if (!first || first.tagName !== "P") continue;
    const lead = first.firstChild;
    if (!lead || lead.nodeType !== Node.TEXT_NODE) continue;
    const match = (lead.textContent ?? "").match(CALLOUT_START);
    if (!match) continue;

    const info = resolveCallout(match[1]);
    lead.textContent = (lead.textContent ?? "").slice(match[0].length);

    const titleText = document.createElement("span");
    while (first.firstChild) {
      const child = first.firstChild;
      if (child.nodeType === Node.ELEMENT_NODE && (child as Element).tagName === "BR") {
        child.remove();
        break;
      }
      if (child.nodeType === Node.TEXT_NODE) {
        const text = child.textContent ?? "";
        const nl = text.indexOf("\n");
        if (nl >= 0) {
          const before = text.slice(0, nl);
          const after = text.slice(nl + 1);
          if (before) titleText.append(document.createTextNode(before));
          if (after) child.textContent = after;
          else child.remove();
          break;
        }
        if (text) titleText.appendChild(child);
        else child.remove();
      } else {
        titleText.appendChild(child);
      }
    }

    if (!titleText.textContent?.trim()) {
      titleText.textContent = info.defaultTitle;
    }

    if (!first.hasChildNodes() || !first.textContent?.trim()) {
      first.remove();
    }

    const titleBar = document.createElement("div");
    titleBar.className = "callout-title";
    titleBar.append(createCalloutIcon(info.kind), titleText);

    bq.classList.add("callout", `callout-${info.kind}`);
    bq.prepend(titleBar);
  }
}

function enhanceCodeBlocks(root: HTMLElement): void {
  for (const pre of root.querySelectorAll<HTMLElement>("pre")) {
    if (pre.querySelector(":scope > .dvoix-code-copy")) continue;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "dvoix-code-copy";
    button.setAttribute("aria-label", "Copy code");
    button.title = "Copy code";
    button.appendChild(copyIcon());

    let resetTimer: ReturnType<typeof setTimeout> | undefined;
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const text = pre.querySelector("code")?.textContent ?? "";
      if (!navigator.clipboard) return;
      void navigator.clipboard.writeText(text).then(() => {
        clearTimeout(resetTimer);
        button.classList.add("copied");
        button.setAttribute("aria-label", "Copied");
        button.title = "Copied";
        button.replaceChildren(checkIcon());
        resetTimer = setTimeout(() => {
          button.classList.remove("copied");
          button.setAttribute("aria-label", "Copy code");
          button.title = "Copy code";
          button.replaceChildren(copyIcon());
        }, 1500);
      });
    });

    pre.appendChild(button);
  }
}

/** Adds callout formatting and code copy buttons to a `.prose` reading container. */
export function enhanceReading(root: HTMLElement): void {
  enhanceCallouts(root);
  enhanceCodeBlocks(root);
}
