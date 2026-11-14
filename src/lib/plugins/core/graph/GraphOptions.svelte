<script lang="ts">
  // All settings of the graph: in the panel of the graph view and on the
  // plugin's page in the settings dialog. Changes apply right away.
  import Select from "../../../components/Select.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { GRAPH_DEFAULTS, graphSettings, type GraphSettings } from "./settings";

  type BoolKey = { [K in keyof GraphSettings]: GraphSettings[K] extends boolean ? K : never }[keyof GraphSettings];
  type NumKey = { [K in keyof GraphSettings]: GraphSettings[K] extends number ? K : never }[keyof GraphSettings];

  const s = $derived(graphSettings.value);

  function set<K extends keyof GraphSettings>(key: K, value: GraphSettings[K]) {
    graphSettings.value[key] = value;
    graphSettings.save();
  }

  function reset() {
    for (const key of Object.keys(GRAPH_DEFAULTS) as (keyof GraphSettings)[]) {
      (graphSettings.value as Record<string, unknown>)[key] = GRAPH_DEFAULTS[key];
    }
    graphSettings.save();
  }
</script>

{#snippet toggle(key: BoolKey, label: string, hint?: string)}
  <div class="flex items-center justify-between gap-4 py-1.5">
    <div class="min-w-0">
      <div class="text-sm text-zinc-200">{label}</div>
      {#if hint}<div class="text-xs text-zinc-500">{hint}</div>{/if}
    </div>
    <Toggle {label} checked={s[key]} onchange={(v) => set(key, v)} />
  </div>
{/snippet}

{#snippet slider(key: NumKey, label: string, min: number, max: number, step: number)}
  <label class="block py-1.5">
    <div class="flex justify-between text-sm text-zinc-200">
      <span>{label}</span>
      <span class="text-xs text-zinc-500 tabular-nums">{s[key]}</span>
    </div>
    <input
      type="range"
      class="mt-1 w-full accent-violet-500"
      {min}
      {max}
      {step}
      value={s[key]}
      oninput={(e) => set(key, Number(e.currentTarget.value))}
    />
  </label>
{/snippet}

<div class="space-y-4">
  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Filter</h3>
    <input
      class="w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-violet-500"
      placeholder="e.g. projects #idea -archive"
      aria-label="Filter"
      value={s.filter}
      oninput={(e) => set("filter", e.currentTarget.value)}
    />
    <p class="mt-1 text-xs text-zinc-500">Shows only files whose path, title or #tag contains all the words. Put - in front to hide them.</p>
    {@render toggle("orphans", "Single notes", "Notes without connections")}
    {@render toggle("attachments", "Attachments", "Images, PDFs and other files")}
    {@render toggle("unresolved", "Missing notes", "Links to notes that don't exist yet")}
  </section>

  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Connections</h3>
    {@render toggle("links", "Links", "[[Wikilinks]] and Markdown links")}
    {@render toggle("folders", "Folders", "Notes are attached to their folder")}
    {@render toggle("tags", "Tags", "Notes with the same #tag")}
    {#if s.tags}
      <div class="flex items-center justify-between gap-4 py-1.5">
        <span class="text-sm text-zinc-200">Show tags as</span>
        <Select
          label="Show tags as"
          value={s.tagMode}
          options={[
            { value: "nodes", label: "own nodes" },
            { value: "shared", label: "direct lines" },
          ]}
          onchange={(v: GraphSettings["tagMode"]) => set("tagMode", v)}
        />
      </div>
    {/if}
  </section>

  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Appearance</h3>
    <div class="flex items-center justify-between gap-4 py-1.5">
      <span class="text-sm text-zinc-200">Colors by</span>
      <Select
        label="Colors by"
        value={s.colorBy}
        options={[
          { value: "none", label: "none" },
          { value: "folder", label: "Folder" },
          { value: "tag", label: "Tag" },
        ]}
        onchange={(v: GraphSettings["colorBy"]) => set("colorBy", v)}
      />
    </div>
    {@render toggle("sizeByLinks", "Size by connections", "Often linked notes get bigger")}
    {@render toggle("arrows", "Arrows", "Shows the direction of links")}
    {@render slider("nodeSize", "Node size", 0.3, 3, 0.1)}
    {@render slider("linkWidth", "Line width", 0.2, 4, 0.1)}
    {@render slider("labelZoom", "Titles from zoom level", 0, 4, 0.1)}
  </section>

  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Forces</h3>
    {@render slider("repel", "Repulsion", 0, 400, 5)}
    {@render slider("linkDistance", "Line length", 5, 300, 5)}
    {@render slider("linkStrength", "Link strength", 0, 1, 0.05)}
    {@render slider("center", "Pull to center", 0, 0.5, 0.01)}
  </section>

  <button class="rounded-md px-2 py-1 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200" onclick={reset}>
    Reset to defaults
  </button>
</div>
