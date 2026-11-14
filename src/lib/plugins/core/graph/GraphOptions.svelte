<script lang="ts">
  // All settings of the graph: in the panel of the graph view and on the
  // plugin's page in the settings dialog. Changes apply right away.
  import { RotateCcw, Search, X } from "@lucide/svelte";
  import Select from "../../../components/Select.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { GRAPH_DEFAULTS, graphSettings, type GraphSettings } from "./settings";

  type BoolKey = { [Key in keyof GraphSettings]: GraphSettings[Key] extends boolean ? Key : never }[keyof GraphSettings];
  type NumKey = { [Key in keyof GraphSettings]: GraphSettings[Key] extends number ? Key : never }[keyof GraphSettings];

  const settings = $derived(graphSettings.value);

  function set<Key extends keyof GraphSettings>(key: Key, value: GraphSettings[Key]) {
    graphSettings.value[key] = value;
    graphSettings.save();
  }

  function reset() {
    for (const key of Object.keys(GRAPH_DEFAULTS) as (keyof GraphSettings)[]) {
      (graphSettings.value as Record<string, unknown>)[key] = GRAPH_DEFAULTS[key];
    }
    graphSettings.save();
  }

  function formatNumber(value: number): number {
    return Number(value.toFixed(2));
  }
</script>

{#snippet toggle(key: BoolKey, label: string, hint?: string)}
  <div class="flex items-start justify-between gap-3 py-2">
    <div class="min-w-0">
      <div class="text-sm text-zinc-200">{label}</div>
      {#if hint}<div class="mt-0.5 text-xs leading-4 text-zinc-400">{hint}</div>{/if}
    </div>
    <div class="pt-0.5">
      <Toggle {label} checked={settings[key]} onchange={(value) => set(key, value)} />
    </div>
  </div>
{/snippet}

{#snippet slider(key: NumKey, label: string, min: number, max: number, step: number)}
  <label class="block py-2">
    <div class="flex justify-between text-sm text-zinc-200">
      <span>{label}</span>
      <span class="text-xs text-zinc-400 tabular-nums">{formatNumber(settings[key])}</span>
    </div>
    <input
      type="range"
      class="mt-1.5 w-full accent-violet-500"
      {min}
      {max}
      {step}
      value={settings[key]}
      oninput={(event) => set(key, Number(event.currentTarget.value))}
    />
  </label>
{/snippet}

<div class="space-y-5">
  <section>
    <h3 class="mb-2 text-xs font-semibold tracking-wide text-zinc-400 uppercase">Filter</h3>
    <div class="relative">
      <Search size={14} class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-zinc-500" />
      <input
        class="w-full rounded-md border border-zinc-700 bg-zinc-950 py-1.5 pr-8 pl-8 text-sm text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-violet-500"
        placeholder="projects #idea -archive"
        aria-label="Filter notes"
        value={settings.filter}
        oninput={(event) => set("filter", event.currentTarget.value)}
      />
      {#if settings.filter}
        <button
          class="absolute top-1/2 right-1.5 -translate-y-1/2 rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
          aria-label="Clear filter"
          onclick={() => set("filter", "")}
        >
          <X size={13} />
        </button>
      {/if}
    </div>
    <p class="mt-1.5 text-xs leading-4 text-zinc-400">Match path, title or #tag. Prefix a word with - to hide it.</p>
    {@render toggle("orphans", "Single notes", "Notes without connections")}
    {@render toggle("attachments", "Attachments", "Images, PDFs and other files")}
    {@render toggle("unresolved", "Missing notes", "Links to notes that don't exist yet")}
  </section>

  <section class="border-t border-zinc-800 pt-4">
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-400 uppercase">Connections</h3>
    {@render toggle("links", "Links", "[[Wikilinks]] and Markdown links")}
    {@render toggle("folders", "Folders", "Notes are attached to their folder")}
    {@render toggle("tags", "Tags", "Notes with the same #tag")}
    {#if settings.tags}
      <div class="flex items-center justify-between gap-3 py-2">
        <span class="text-sm text-zinc-200">Show tags as</span>
        <Select
          label="Show tags as"
          class="w-32 shrink-0"
          value={settings.tagMode}
          options={[
            { value: "nodes", label: "Own nodes" },
            { value: "shared", label: "Direct lines" },
          ]}
          onchange={(value: GraphSettings["tagMode"]) => set("tagMode", value)}
        />
      </div>
    {/if}
  </section>

  <section class="border-t border-zinc-800 pt-4">
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-400 uppercase">Focus</h3>
    <div class="flex items-center justify-between gap-3 py-2">
      <span class="text-sm text-zinc-200">Neighborhood depth</span>
      <Select
        label="Neighborhood depth"
        class="w-28 shrink-0"
        value={String(settings.focusDepth)}
        options={[
          { value: "1", label: "1 hop" },
          { value: "2", label: "2 hops" },
          { value: "3", label: "3 hops" },
        ]}
        onchange={(value) => set("focusDepth", Number(value))}
      />
    </div>
    <p class="text-xs leading-4 text-zinc-400">Right-click a node, or use Focus in its card, to keep only its neighborhood.</p>
  </section>

  <section class="border-t border-zinc-800 pt-4">
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-400 uppercase">Appearance</h3>
    <div class="flex items-center justify-between gap-3 py-2">
      <span class="text-sm text-zinc-200">Colors by</span>
      <Select
        label="Colors by"
        class="w-28 shrink-0"
        value={settings.colorBy}
        options={[
          { value: "none", label: "None" },
          { value: "folder", label: "Folder" },
          { value: "tag", label: "Tag" },
        ]}
        onchange={(value: GraphSettings["colorBy"]) => set("colorBy", value)}
      />
    </div>
    {@render toggle("sizeByLinks", "Size by connections", "Often linked notes get bigger")}
    {@render toggle("labels", "Labels", "Show titles when zoomed in")}
    {@render toggle("arrows", "Arrows", "Shows the direction of links")}
    {@render slider("nodeSize", "Node size", 0.3, 3, 0.1)}
    {@render slider("linkWidth", "Line width", 0.2, 4, 0.1)}
    {@render slider("labelZoom", "Titles from zoom level", 0, 4, 0.1)}
  </section>

  <section class="border-t border-zinc-800 pt-4">
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-400 uppercase">Forces</h3>
    {@render slider("repel", "Repulsion", 0, 400, 5)}
    {@render slider("linkDistance", "Line length", 5, 300, 5)}
    {@render slider("linkStrength", "Link strength", 0, 1, 0.05)}
    {@render slider("center", "Pull to center", 0, 0.5, 0.01)}
  </section>

  <button
    class="flex w-full items-center justify-center gap-1.5 rounded-md border border-zinc-700 px-3 py-1.5 text-xs text-zinc-300 hover:border-zinc-600 hover:bg-zinc-800 hover:text-zinc-100"
    onclick={reset}
  >
    <RotateCcw size={13} />
    Reset to defaults
  </button>
</div>
