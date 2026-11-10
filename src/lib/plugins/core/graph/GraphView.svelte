<script lang="ts">
  // The graph view in a tab: notes as nodes, drawn on a canvas by force-graph
  // (d3-force layout), so a few thousand nodes stay smooth. Click opens a note
  // in a new tab, hover highlights the neighbours, the panel on the right holds
  // the settings.
  import type ForceGraph from "force-graph";
  import { onMount } from "svelte";
  import { Maximize, SlidersHorizontal, X } from "@lucide/svelte";
  import { api, type GraphData } from "../../../api";
  import { vault } from "../../../vault.svelte";
  import GraphOptions from "./GraphOptions.svelte";
  import { buildGraph, colorGroups, type EdgeKind, type GraphNode, type NodeKind } from "./model";
  import { graphSettings } from "./settings";

  /** What force-graph holds: our node plus the simulation's position. */
  type SimNode = GraphNode & { x?: number; y?: number; vx?: number; vy?: number; fx?: number; fy?: number };
  type SimLink = { source: string | SimNode; target: string | SimNode; kind: EdgeKind };

  const BASE_RADIUS = 4;
  const GROUP_COLORS = 10;

  let container: HTMLDivElement;
  let graph: ForceGraph<SimNode, SimLink> | undefined;
  let data = $state<GraphData>({ notes: [], links: [] });
  let loadError = $state<string | null>(null);
  /** force-graph itself is missing, so there is no graph to draw, only a hint. */
  let importError = $state<string | null>(null);
  let panelOpen = $state(false);
  let fitted = false;
  let fitTimer: ReturnType<typeof setTimeout> | undefined;

  let hovered: SimNode | null = null;
  let neighbours = new Set<string>();
  /** Lines at each node, to find the neighbours on hover. */
  let adjacency = new Map<string, Set<string>>();
  /** Nodes by id from the last update, so a rebuild keeps their positions. */
  let simNodes = new Map<string, SimNode>();

  const settings = $derived(graphSettings.value);
  // Only the settings that change nodes and lines rebuild the graph (and restart the layout).
  const modelSettings = $derived({
    links: settings.links,
    folders: settings.folders,
    tags: settings.tags,
    tagMode: settings.tagMode,
    filter: settings.filter,
    orphans: settings.orphans,
    attachments: settings.attachments,
    unresolved: settings.unresolved,
  });
  const model = $derived(buildGraph(data, vault.entries, modelSettings, vault.info?.name ?? "Vault"));
  /** Why the graph is missing: the data did not load, or force-graph itself is not there. */
  const error = $derived(importError ?? loadError);

  // ---------------------------------------------------------------- colors

  let colors = readColors();

  /** CSS colors from `app.css` as `rgb()` the canvas surely understands (Tailwind uses oklch). */
  function readColors() {
    const style = getComputedStyle(document.documentElement);
    const probe = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
    const rgb = (name: string, fallback: string) => {
      const css = style.getPropertyValue(name).trim() || fallback;
      if (!probe) return css;
      probe.clearRect(0, 0, 1, 1);
      probe.fillStyle = css;
      probe.fillRect(0, 0, 1, 1);
      const [r, g, b] = probe.getImageData(0, 0, 1, 1).data;
      return `rgb(${r}, ${g}, ${b})`;
    };
    return {
      node: rgb("--graph-node", "#a1a1aa"),
      link: rgb("--graph-link", "#3f3f46"),
      label: rgb("--graph-label", "#d4d4d8"),
      highlight: rgb("--graph-highlight", "#a78bfa"),
      structure: rgb("--graph-structure", "#d4d4d8"),
      kinds: {
        attachment: rgb("--graph-attachment", "#71717a"),
        unresolved: rgb("--graph-unresolved", "#52525b"),
        folder: rgb("--graph-folder", "#fcd34d"),
        tag: rgb("--graph-tag", "#34d399"),
      } as Partial<Record<NodeKind, string>>,
      groups: Array.from({ length: GROUP_COLORS }, (_, i) => rgb(`--graph-group-${i + 1}`, "#a78bfa")),
    };
  }

  /** Color group per node id (see `colorGroups`). */
  let groupIndex = new Map<string, number>();

  function nodeColor(n: SimNode): string {
    const i = groupIndex.get(n.id);
    // Only the most frequent groups get a color of their own; the rest stay neutral.
    if (i !== undefined && i < GROUP_COLORS) return colors.groups[i];
    // With group colors on, folder and tag nodes stay neutral so they don't look like a group.
    if (graphSettings.value.colorBy !== "none" && (n.kind === "folder" || n.kind === "tag")) return colors.structure;
    return colors.kinds[n.kind] ?? colors.node;
  }

  // ---------------------------------------------------------------- drawing

  function radius(n: SimNode): number {
    const s = graphSettings.value;
    // Capped, so a folder or tag with hundreds of notes doesn't cover the graph.
    const grow = s.sizeByLinks ? Math.min(Math.sqrt(n.degree), 5) * 0.9 : 0;
    const kind = n.kind === "folder" || n.kind === "tag" ? 1.2 : 1;
    return BASE_RADIUS * s.nodeSize * kind * (1 + grow * 0.5);
  }

  const isLit = (n: SimNode) => !hovered || n === hovered || neighbours.has(n.id);
  const endId = (end: string | SimNode) => (typeof end === "string" ? end : end.id);
  const linkLit = (l: SimLink) => !!hovered && (endId(l.source) === hovered.id || endId(l.target) === hovered.id);

  function drawNode(n: SimNode, ctx: CanvasRenderingContext2D, scale: number) {
    const r = radius(n);
    const lit = isLit(n);
    ctx.globalAlpha = lit ? 1 : 0.2;
    ctx.beginPath();
    if (n.kind === "folder") ctx.rect(n.x! - r, n.y! - r, r * 2, r * 2);
    else ctx.arc(n.x!, n.y!, r, 0, 2 * Math.PI);
    if (n.kind === "unresolved") {
      ctx.strokeStyle = nodeColor(n);
      ctx.lineWidth = 1.2 / scale;
      ctx.stroke();
    } else {
      ctx.fillStyle = n === hovered ? colors.highlight : nodeColor(n);
      ctx.fill();
    }
    const s = graphSettings.value;
    const showLabel = n === hovered || (hovered && lit) || scale >= s.labelZoom;
    if (showLabel) {
      const size = 12 / scale;
      ctx.font = `${size}px Inter, ui-sans-serif, system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "top";
      ctx.fillStyle = colors.label;
      ctx.fillText(n.label, n.x!, n.y! + r + 2 / scale);
    }
    ctx.globalAlpha = 1;
  }

  function paintArea(n: SimNode, color: string, ctx: CanvasRenderingContext2D) {
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(n.x!, n.y!, radius(n) + 2, 0, 2 * Math.PI);
    ctx.fill();
  }

  function linkColor(l: SimLink): string {
    if (linkLit(l)) return colors.highlight;
    return hovered ? colors.link.replace("rgb(", "rgba(").replace(")", ", 0.25)") : colors.link;
  }

  /** Redraws without restarting the layout (settings that only change looks, hover). */
  function redraw() {
    graph?.nodeCanvasObject(drawNode);
  }

  // ---------------------------------------------------------------- forces

  /** Pulls every node towards the middle so unconnected parts don't drift away. */
  function gravity() {
    let nodes: SimNode[] = [];
    const force = (alpha: number) => {
      const k = graphSettings.value.center * alpha;
      for (const n of nodes) {
        n.vx = (n.vx ?? 0) - (n.x ?? 0) * k;
        n.vy = (n.vy ?? 0) - (n.y ?? 0) * k;
      }
    };
    force.initialize = (ns: SimNode[]) => (nodes = ns);
    return force;
  }

  function applyForces(reheat: boolean) {
    if (!graph) return;
    const s = graphSettings.value;
    graph.d3Force("charge")?.strength(-s.repel);
    graph.d3Force("link")?.distance(s.linkDistance).strength(s.linkStrength);
    if (reheat) graph.d3ReheatSimulation();
  }

  // ---------------------------------------------------------------- lifecycle

  /**
   * Loads force-graph on demand. A missing package (e.g. node_modules that is older
   * than package.json) would otherwise end as an unhandled rejection and an empty tab.
   */
  async function loadForceGraph() {
    try {
      const { default: ForceGraphClass } = await import("force-graph");
      return ForceGraphClass;
    } catch (e) {
      console.error("force-graph could not be loaded", e);
      importError = 'force-graph is missing. Please run "npm install".';
      return null;
    }
  }

  /** Loaded with the first graph tab, not at app start: it is the biggest part of this plugin. */
  async function createGraph() {
    const ForceGraphClass = await loadForceGraph();
    if (!ForceGraphClass) return; // not installed, the view says so
    if (!container.isConnected) return; // tab closed while loading
    graph = new ForceGraphClass<SimNode, SimLink>(container)
      .backgroundColor("rgba(0,0,0,0)")
      .nodeId("id")
      .nodeLabel(() => "")
      .nodeCanvasObject(drawNode)
      .nodePointerAreaPaint(paintArea)
      .linkColor(linkColor)
      .linkWidth((l) => graphSettings.value.linkWidth * (linkLit(l) ? 1.8 : 1))
      .linkDirectionalArrowLength(() => (graphSettings.value.arrows ? 3.5 : 0))
      .linkDirectionalArrowRelPos(1)
      .minZoom(0.05)
      .maxZoom(12)
      .cooldownTime(8000)
      .d3Force("center", null)
      .d3Force("gravity", gravity())
      .onNodeHover((n) => {
        hovered = n;
        neighbours = n ? (adjacency.get(n.id) ?? new Set()) : new Set();
        container.style.cursor = n && n.kind !== "folder" && n.kind !== "tag" ? "pointer" : "";
        redraw();
      })
      // In a new tab, so the graph stays open next to the note (with tabs off it is replaced).
      .onNodeClick((n) => {
        if (n.kind === "note" || n.kind === "attachment") void vault.openFile(n.path, { newTab: true });
        else if (n.kind === "unresolved") void vault.openLink(n.path, { newTab: true });
      });
    applyForces(false);
    graph.width(container.clientWidth).height(container.clientHeight);
    // Hand the current state to the new instance (the effects below ran before it existed).
    version++;
  }

  /** Bumped when the graph instance is created, so the effects that feed it run again. */
  let version = $state(0);

  onMount(() => {
    void createGraph();

    const resize = new ResizeObserver(() => graph?.width(container.clientWidth).height(container.clientHeight));
    resize.observe(container);

    // Colors follow the CSS variables, e.g. when a theme switches between light and dark.
    const scheme = matchMedia("(prefers-color-scheme: dark)");
    const recolor = () => {
      colors = readColors();
      redraw();
    };
    scheme.addEventListener("change", recolor);

    return () => {
      clearTimeout(fitTimer);
      resize.disconnect();
      scheme.removeEventListener("change", recolor);
      graph?._destructor();
      graph = undefined;
    };
  });

  // Reload when files change on disk.
  $effect(() => {
    void vault.changes.notes.number;
    api
      .graph()
      .then((d) => {
        data = d;
        loadError = null;
      })
      .catch((e) => (loadError = String(e)));
  });

  // New nodes and lines: keep the positions of nodes that were there before,
  // so saving a note doesn't shuffle the whole graph.
  $effect(() => {
    const m = model;
    void version;
    if (!graph) return;
    const next = new Map<string, SimNode>();
    for (const n of m.nodes) {
      const old = simNodes.get(n.id);
      next.set(n.id, old ? Object.assign(old, n) : { ...n });
    }
    simNodes = next;
    adjacency = new Map([...next.keys()].map((id) => [id, new Set<string>()]));
    for (const e of m.edges) {
      adjacency.get(e.source)?.add(e.target);
      adjacency.get(e.target)?.add(e.source);
    }
    if (hovered && !next.has(hovered.id)) hovered = null;
    graph.graphData({ nodes: [...next.values()], links: m.edges.map((e) => ({ ...e })) });
    // Frame the graph once, when the layout has roughly settled.
    if (!fitted && m.nodes.length) {
      fitted = true;
      fitTimer = setTimeout(() => graph?.zoomToFit(400, 40), 800);
    }
  });

  $effect(() => {
    void version;
    groupIndex = colorGroups(model.nodes, settings.colorBy);
    redraw();
  });

  // Forces: restart the layout so the change is visible.
  $effect(() => {
    void [settings.repel, settings.linkDistance, settings.linkStrength, settings.center, version];
    applyForces(true);
  });

  // Looks only: redraw.
  $effect(() => {
    void [settings.nodeSize, settings.sizeByLinks, settings.linkWidth, settings.arrows, settings.labelZoom];
    if (!graph) return;
    graph.linkWidth(graph.linkWidth()).linkDirectionalArrowLength(graph.linkDirectionalArrowLength());
    redraw();
  });

  const noteCount = $derived(model.nodes.filter((n) => n.kind === "note").length);
</script>

<div class="relative h-full w-full overflow-hidden bg-zinc-950">
  <div bind:this={container} class="absolute inset-0"></div>

  {#if error}
    <div class="absolute inset-x-0 top-1/2 text-center text-sm text-red-300">Graph could not be loaded: {error}</div>
  {:else if !model.nodes.length}
    <div class="pointer-events-none absolute inset-x-0 top-1/2 text-center text-sm text-zinc-500">
      {settings.filter ? "No note matches the filter." : "No notes in the vault yet."}
    </div>
  {/if}

  <div class="pointer-events-none absolute bottom-2 left-3 text-xs text-zinc-600">
    {noteCount} {noteCount === 1 ? "note" : "notes"} · {model.edges.length} {model.edges.length === 1 ? "connection" : "connections"}
  </div>

  <div class="absolute top-2 right-2 flex flex-col items-end gap-2">
    <div class="flex gap-1">
      <button
        class="flex h-8 w-8 items-center justify-center rounded-md border border-zinc-800 bg-zinc-900/90 text-zinc-400 hover:text-zinc-100"
        title="Show all"
        aria-label="Show all"
        onclick={() => graph?.zoomToFit(400, 40)}
      >
        <Maximize size={15} />
      </button>
      <button
        class="flex h-8 w-8 items-center justify-center rounded-md border border-zinc-800 bg-zinc-900/90 hover:text-zinc-100 {panelOpen
          ? 'text-violet-300'
          : 'text-zinc-400'}"
        title="Graph settings"
        aria-label="Graph settings"
        aria-expanded={panelOpen}
        onclick={() => (panelOpen = !panelOpen)}
      >
        <SlidersHorizontal size={15} />
      </button>
    </div>
    {#if panelOpen}
      <div
        class="max-h-[calc(100vh-10rem)] w-72 overflow-y-auto rounded-lg border border-zinc-800 bg-zinc-900/95 p-3 shadow-xl"
        role="dialog"
        aria-label="Graph settings"
        tabindex="-1"
        onkeydown={(e) => e.key === "Escape" && (panelOpen = false)}
      >
        <div class="mb-2 flex items-center justify-between">
          <span class="text-sm font-medium text-zinc-200">Graph</span>
          <button class="rounded p-0.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200" aria-label="Close" onclick={() => (panelOpen = false)}>
            <X size={14} />
          </button>
        </div>
        <GraphOptions />
      </div>
    {/if}
  </div>
</div>
