<script lang="ts">
  // A first, minimal canvas: pan the board, drag cards, double-click empty
  // space for a new text card. Good enough to prove the file-type module
  // structure; a full editor (tldraw, Svelte Flow, ...) can replace it later
  // while keeping the same `.canvas` files.
  import type { FileViewProps } from "../../../filetypes/types";
  import { colorOf, newId, parseCanvas, serializeCanvas, type CanvasNode, type JsonCanvas } from "./jsonCanvas";

  let { content, onchange, onopen }: FileViewProps = $props();

  let canvas = $state<JsonCanvas>({ nodes: [], edges: [] });
  let error = $state<string | undefined>();
  let lastWritten = "";

  // (Re)load whenever the content differs from what we last wrote ourselves.
  $effect(() => {
    if (content === lastWritten) return;
    try {
      canvas = parseCanvas(content);
      error = undefined;
    } catch (e) {
      error = `Invalid canvas file: ${e}`;
    }
  });

  function commit() {
    lastWritten = serializeCanvas($state.snapshot(canvas) as JsonCanvas);
    onchange(lastWritten);
  }

  let pan = $state({ x: 0, y: 0 });
  type Drag = { kind: "pan" | "node"; startX: number; startY: number; origX: number; origY: number; node?: CanvasNode };
  let drag: Drag | undefined;

  function startPan(e: PointerEvent) {
    if (e.target !== e.currentTarget) return;
    drag = { kind: "pan", startX: e.clientX, startY: e.clientY, origX: pan.x, origY: pan.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function startNodeDrag(e: PointerEvent, node: CanvasNode) {
    if ((e.target as HTMLElement).closest("textarea, a, button")) return;
    e.stopPropagation();
    drag = { kind: "node", startX: e.clientX, startY: e.clientY, origX: node.x, origY: node.y, node };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const dx = e.clientX - drag.startX;
    const dy = e.clientY - drag.startY;
    if (drag.kind === "pan") pan = { x: drag.origX + dx, y: drag.origY + dy };
    else if (drag.node) {
      drag.node.x = Math.round(drag.origX + dx);
      drag.node.y = Math.round(drag.origY + dy);
    }
  }

  function end() {
    if (drag?.kind === "node") commit();
    drag = undefined;
  }

  function addTextNode(e: MouseEvent) {
    if (e.target !== e.currentTarget) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    canvas.nodes.push({
      id: newId(),
      type: "text",
      text: "",
      x: Math.round(e.clientX - rect.left - pan.x - 125),
      y: Math.round(e.clientY - rect.top - pan.y - 30),
      width: 250,
      height: 60,
    });
    commit();
  }

  function removeNode(node: CanvasNode) {
    canvas.nodes = canvas.nodes.filter((n) => n.id !== node.id);
    canvas.edges = canvas.edges.filter((ed) => ed.fromNode !== node.id && ed.toNode !== node.id);
    commit();
  }

  const center = (n: CanvasNode) => ({ x: n.x + n.width / 2, y: n.y + n.height / 2 });
  let nodesById = $derived(new Map(canvas.nodes.map((n) => [n.id, n])));
</script>

{#if error}
  <div class="p-8 text-sm text-red-400">{error}</div>
{:else}
  <div
    class="relative h-full cursor-grab overflow-hidden bg-zinc-950 bg-[radial-gradient(circle,var(--color-zinc-800)_1px,transparent_1px)] [background-size:24px_24px] select-none"
    style:background-position="{pan.x}px {pan.y}px"
    role="application"
    aria-label="Canvas"
    onpointerdown={startPan}
    onpointermove={move}
    onpointerup={end}
    ondblclick={addTextNode}
  >
    <div class="pointer-events-none absolute top-0 left-0" style:transform="translate({pan.x}px, {pan.y}px)">
      <svg class="absolute overflow-visible" width="1" height="1">
        {#each canvas.edges as edge (edge.id)}
          {@const a = nodesById.get(edge.fromNode)}
          {@const b = nodesById.get(edge.toNode)}
          {#if a && b}
            <line
              x1={center(a).x}
              y1={center(a).y}
              x2={center(b).x}
              y2={center(b).y}
              stroke={colorOf(edge.color) ?? "var(--color-zinc-600)"}
              stroke-width="2"
            />
          {/if}
        {/each}
      </svg>

      {#each canvas.nodes as node (node.id)}
        <div
          class="group pointer-events-auto absolute flex cursor-move flex-col rounded-lg border bg-zinc-900 text-sm shadow-lg"
          class:bg-transparent={node.type === "group"}
          style:left="{node.x}px"
          style:top="{node.y}px"
          style:width="{node.width}px"
          style:height="{node.height}px"
          style:border-color={colorOf(node.color) ?? "var(--color-zinc-700)"}
          role="group"
          onpointerdown={(e) => startNodeDrag(e, node)}
        >
          <button
            class="absolute -top-2 -right-2 hidden h-5 w-5 rounded-full bg-zinc-700 text-xs text-zinc-200 group-hover:block"
            title="Remove"
            onclick={() => removeNode(node)}>×</button
          >
          {#if node.type === "text"}
            <textarea
              class="h-full w-full resize-none bg-transparent p-3 text-zinc-200 outline-none"
              placeholder="Text …"
              bind:value={node.text}
              onblur={commit}
            ></textarea>
          {:else if node.type === "file"}
            <button class="h-full w-full p-3 text-left text-violet-300 hover:underline" onclick={() => onopen(node.file)}>
              {node.file}
            </button>
          {:else if node.type === "link"}
            <a class="p-3 break-all text-violet-300 hover:underline" href={node.url} target="_blank" rel="noreferrer">{node.url}</a>
          {:else}
            <span class="-mt-6 text-xs text-zinc-400">{node.label ?? ""}</span>
          {/if}
        </div>
      {/each}
    </div>

    <div class="pointer-events-none absolute bottom-3 left-3 text-xs text-zinc-500">
      Double-click: new card · Drag: move
    </div>
  </div>
{/if}
