<script lang="ts">
  // All settings of the graph: in the panel of the graph view and on the
  // plugin's page in the settings dialog. Changes apply right away.
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

  const select =
    "rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-sm text-zinc-200 outline-none focus:border-violet-500";
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
      placeholder="z. B. projekte #idee -archiv"
      aria-label="Filter"
      value={s.filter}
      oninput={(e) => set("filter", e.currentTarget.value)}
    />
    <p class="mt-1 text-xs text-zinc-500">Zeigt nur Dateien, deren Pfad, Titel oder #Tag alle Wörter enthält. Mit - davor ausblenden.</p>
    {@render toggle("orphans", "Einzelne Notizen", "Notizen ohne Verbindung")}
    {@render toggle("attachments", "Anhänge", "Bilder, PDFs und andere Dateien")}
    {@render toggle("unresolved", "Fehlende Notizen", "Links auf Notizen, die es noch nicht gibt")}
  </section>

  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Verbindungen</h3>
    {@render toggle("links", "Links", "[[Wikilinks]] und Markdown-Links")}
    {@render toggle("folders", "Ordner", "Notizen hängen an ihrem Ordner")}
    {@render toggle("tags", "Tags", "Notizen mit gleichem #Tag")}
    {#if s.tags}
      <div class="flex items-center justify-between gap-4 py-1.5">
        <span class="text-sm text-zinc-200">Tags zeigen als</span>
        <select class={select} value={s.tagMode} onchange={(e) => set("tagMode", e.currentTarget.value as GraphSettings["tagMode"])}>
          <option value="nodes">eigene Knoten</option>
          <option value="shared">direkte Linien</option>
        </select>
      </div>
    {/if}
  </section>

  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Darstellung</h3>
    <div class="flex items-center justify-between gap-4 py-1.5">
      <span class="text-sm text-zinc-200">Farben nach</span>
      <select class={select} value={s.colorBy} onchange={(e) => set("colorBy", e.currentTarget.value as GraphSettings["colorBy"])}>
        <option value="none">keine</option>
        <option value="folder">Ordner</option>
        <option value="tag">Tag</option>
      </select>
    </div>
    {@render toggle("sizeByLinks", "Größe nach Verbindungen", "Viel verlinkte Notizen werden größer")}
    {@render toggle("arrows", "Pfeile", "Zeigt die Richtung von Links")}
    {@render slider("nodeSize", "Knotengröße", 0.3, 3, 0.1)}
    {@render slider("linkWidth", "Linienstärke", 0.2, 4, 0.1)}
    {@render slider("labelZoom", "Titel ab Zoomstufe", 0, 4, 0.1)}
  </section>

  <section>
    <h3 class="mb-1 text-xs font-semibold tracking-wide text-zinc-500 uppercase">Kräfte</h3>
    {@render slider("repel", "Abstoßung", 0, 400, 5)}
    {@render slider("linkDistance", "Linienlänge", 5, 300, 5)}
    {@render slider("linkStrength", "Linienzug", 0, 1, 0.05)}
    {@render slider("center", "Zug zur Mitte", 0, 0.5, 0.01)}
  </section>

  <button class="rounded-md px-2 py-1 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200" onclick={reset}>
    Standard wiederherstellen
  </button>
</div>
