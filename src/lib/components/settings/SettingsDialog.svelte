<script module lang="ts">
  /** Modal id of the settings dialog (`ui.modal`). */
  export const SETTINGS = "settings";
</script>

<script lang="ts">
  // Settings (Strg+,): editor, appearance, core plugins on/off, the plugin store,
  // and one page per plugin that has settings. Everything is saved in `<vault>/.dvoix/`.
  import { Puzzle, X } from "@lucide/svelte";
  import Select from "../Select.svelte";
  import Setting from "./Setting.svelte";
  import StorePage from "./StorePage.svelte";
  import Toggle from "./Toggle.svelte";
  import { hotkeyLabel, plugins } from "../../plugins";
  import { APPEARANCE_DEFAULTS, settings, type ViewMode } from "../../settings.svelte";
  import { ui } from "../../ui.svelte";
  import { vault } from "../../vault.svelte";

  let page = $state("editor");

  const pluginPages = $derived(
    [...plugins.all, ...plugins.extra]
      .filter((p) => plugins.settingTabs.some((t) => t.pluginId === p.manifest.id))
      .map((p) => ({ id: `plugin:${p.manifest.id}`, label: p.manifest.name, pluginId: p.manifest.id })),
  );
  const activeTab = $derived(
    page.startsWith("plugin:") ? plugins.settingTabs.find((t) => `plugin:${t.pluginId}` === page) : undefined,
  );

  // A plugin page disappears when its plugin is turned off.
  $effect(() => {
    if (page.startsWith("plugin:") && !activeTab) page = "plugins";
  });

  function close() {
    if (ui.modal === SETTINGS) ui.modal = null;
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  function setApp<K extends keyof typeof settings.app>(key: K, value: (typeof settings.app)[K]) {
    settings.app[key] = value;
    settings.saveApp();
  }

  function setSize(key: "fontSize" | "interfaceFontSize", raw: string) {
    const n = Number(raw);
    if (!Number.isFinite(n)) return;
    settings.appearance[key] = Math.min(28, Math.max(10, Math.round(n)));
    settings.saveAppearance();
  }

  function commandsOf(pluginId: string) {
    return plugins.commands.filter((c) => c.pluginId === pluginId && c.hotkeys?.length);
  }

  const navBtn = (active: boolean) =>
    `block w-full truncate rounded-md px-3 py-1.5 text-left text-sm ${active ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200"}`;
  const input =
    "w-20 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-right text-sm text-zinc-200 outline-none focus:border-violet-500";
</script>

<svelte:window onkeydown={ui.modal === SETTINGS ? keydown : undefined} />

{#if ui.modal === SETTINGS}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40 bg-black/50" onclick={close}></div>
  <div
    class="fixed top-1/2 left-1/2 z-50 flex h-[min(640px,88vh)] w-[min(880px,94vw)] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-2xl"
    role="dialog"
    aria-modal="true"
    aria-label="Einstellungen"
  >
    <nav class="w-52 shrink-0 overflow-y-auto border-r border-zinc-800 bg-zinc-950/60 p-2">
      <div class="px-3 pt-2 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">Optionen</div>
      <button class={navBtn(page === "editor")} onclick={() => (page = "editor")}>Editor</button>
      <button class={navBtn(page === "appearance")} onclick={() => (page = "appearance")}>Darstellung</button>
      <button class={navBtn(page === "plugins")} onclick={() => (page = "plugins")}>Core-Plugins</button>
      <button class={navBtn(page === "store")} onclick={() => (page = "store")}>Plugin-Store</button>
      {#if pluginPages.length}
        <div class="px-3 pt-4 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">Plugin-Optionen</div>
        {#each pluginPages as p (p.id)}
          <button class={navBtn(page === p.id)} onclick={() => (page = p.id)}>{p.label}</button>
        {/each}
      {/if}
    </nav>

    <section class="flex min-w-0 flex-1 flex-col">
      <header class="flex items-center justify-between border-b border-zinc-800 px-6 py-3">
        <h2 class="text-sm font-medium text-zinc-100">
          {page === "editor"
            ? "Editor"
            : page === "appearance"
              ? "Darstellung"
              : page === "plugins"
                ? "Core-Plugins"
                : page === "store"
                  ? "Plugin-Store"
                  : (pluginPages.find((p) => p.id === page)?.label ?? "")}
        </h2>
        <button class="rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200" aria-label="Schließen" onclick={close}>
          <X size={16} />
        </button>
      </header>

      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-2">
        {#if page === "editor"}
          <Setting name="Begrenzte Zeilenlänge" description="Text in einer angenehm lesbaren Breite statt über das ganze Fenster.">
            <Toggle label="Begrenzte Zeilenlänge" checked={settings.app.readableLineLength} onchange={(v) => setApp("readableLineLength", v)} />
          </Setting>
          <Setting name="Zeilennummern" description="Zeigt Zeilennummern links neben dem Text.">
            <Toggle label="Zeilennummern" checked={settings.app.lineNumbers} onchange={(v) => setApp("lineNumbers", v)} />
          </Setting>
          <Setting name="Rechtschreibprüfung" description="Markiert Tippfehler mit der Rechtschreibprüfung des Systems.">
            <Toggle label="Rechtschreibprüfung" checked={settings.app.spellcheck} onchange={(v) => setApp("spellcheck", v)} />
          </Setting>
          <Setting name="Standardansicht" description="Wie eine Notiz geöffnet wird: gelesen, in der Live-Vorschau bearbeitet oder als Quelltext.">
            <Select
              label="Standardansicht"
              value={settings.app.viewMode}
              options={[
                { value: "read", label: "Lesen" },
                { value: "edit", label: "Bearbeiten (Live-Vorschau)" },
                { value: "source", label: "Quelltext" },
              ]}
              onchange={(v: ViewMode) => setApp("viewMode", v)}
            />
          </Setting>
        {:else if page === "appearance"}
          <Setting name="Schriftgröße im Editor" description="In Pixeln, Standard {APPEARANCE_DEFAULTS.fontSize}.">
            <input class={input} type="number" min="10" max="28" value={settings.appearance.fontSize} onchange={(e) => setSize("fontSize", e.currentTarget.value)} />
          </Setting>
          <Setting name="Schriftgröße der Oberfläche" description="Seitenleiste, Menüs und Dialoge, in Pixeln, Standard {APPEARANCE_DEFAULTS.interfaceFontSize}.">
            <input
              class={input}
              type="number"
              min="10"
              max="28"
              value={settings.appearance.interfaceFontSize}
              onchange={(e) => setSize("interfaceFontSize", e.currentTarget.value)}
            />
          </Setting>
        {:else if page === "plugins"}
          <p class="py-3 text-xs text-zinc-500">
            Jede Funktion von dvoix ist ein Plugin. Ausgeschaltete Plugins verschwinden aus Seitenleiste, Menüs und Tastenkürzeln,
            deine Dateien bleiben unverändert.
          </p>
          {#each plugins.all as p (p.manifest.id)}
            {@const id = p.manifest.id}
            {@const on = plugins.isEnabled(id)}
            <Setting name={p.manifest.name} description={p.manifest.description}>
              <div class="flex items-center gap-3">
                {#if on && plugins.settingTabs.some((t) => t.pluginId === id)}
                  <button class="text-xs text-violet-300 hover:underline" onclick={() => (page = `plugin:${id}`)}>Optionen</button>
                {/if}
                <Toggle label="{p.manifest.name} ein/aus" checked={on} onchange={(v) => plugins.setEnabled(id, v)} />
              </div>
            </Setting>
            {#if plugins.errors[id]}
              <p class="-mt-2 pb-2 text-xs text-red-400">Konnte nicht geladen werden: {plugins.errors[id]}</p>
            {/if}
            {#if on && commandsOf(id).length}
              <div class="-mt-2 flex flex-wrap gap-x-4 gap-y-1 pb-3 text-[11px] text-zinc-500">
                {#each commandsOf(id) as c (c.fullId)}
                  <span>{c.name}: <kbd class="rounded bg-zinc-800 px-1 py-px text-zinc-400">{hotkeyLabel(c.hotkeys![0])}</kbd></span>
                {/each}
              </div>
            {/if}
          {/each}
        {:else if page === "store"}
          <StorePage onOptions={(id) => (page = `plugin:${id}`)} />
        {:else if activeTab}
          <activeTab.component />
        {/if}
      </div>

      <footer class="flex items-center gap-2 border-t border-zinc-800 px-6 py-2 text-[11px] text-zinc-500">
        <Puzzle size={12} />
        Gespeichert in <code class="truncate text-zinc-400" title="{vault.info?.path}/.dvoix">{vault.info?.name}/.dvoix/</code>
      </footer>
    </section>
  </div>
{/if}
