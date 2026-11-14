<script module lang="ts">
  /** Modal id of the settings dialog (`ui.modal`). */
  export const SETTINGS = "settings";
</script>

<script lang="ts">
  // Settings (Ctrl+,): editor, appearance, keyboard shortcuts, core plugins on/off,
  // the plugin store, and one page per core feature or plugin that has settings. Everything is saved in `<vault>/.dvoix/`.
  import { Check, FilePenLine, Keyboard, Palette, Puzzle, RotateCcw, Store, X } from "@lucide/svelte";
  import Select from "../Select.svelte";
  import Setting from "./Setting.svelte";
  import StorePage from "./StorePage.svelte";
  import Toggle from "./Toggle.svelte";
  import { hotkeyLabel, plugins, type Plugin } from "../../plugins";
  import {
    APPEARANCE_DEFAULTS,
    settings,
    type AccentColor,
    type EditorFont,
    type InterfaceFont,
    type ThemeId,
    type ViewMode,
  } from "../../settings.svelte";
  import { ui } from "../../ui.svelte";
  import { vault } from "../../vault.svelte";

  const THEMES: { id: ThemeId; label: string; bg: string; panel: string; text: string }[] = [
    { id: "dark", label: "Dark", bg: "#09090b", panel: "#18181b", text: "#e4e4e7" },
    { id: "midnight", label: "Midnight", bg: "#050507", panel: "#0b0b10", text: "#f4f4fa" },
    { id: "slate", label: "Slate", bg: "#0b1120", panel: "#1e293b", text: "#e2e8f0" },
    { id: "light", label: "Light", bg: "#ffffff", panel: "#f4f4f6", text: "#18181b" },
  ];

  const ACCENTS: { id: AccentColor; label: string; color: string }[] = [
    { id: "violet", label: "Violet", color: "#8b5cf6" },
    { id: "sky", label: "Sky", color: "#0ea5e9" },
    { id: "emerald", label: "Emerald", color: "#10b981" },
    { id: "rose", label: "Rose", color: "#f43f5e" },
    { id: "amber", label: "Amber", color: "#f59e0b" },
  ];

  let page = $state("editor");

  /** Settings pages of everything in `list` that registered one. */
  const pagesOf = (list: Plugin[]) =>
    list
      .filter((p) => plugins.settingTabs.some((t) => t.pluginId === p.manifest.id))
      .map((p) => ({ id: `plugin:${p.manifest.id}`, label: p.manifest.name, pluginId: p.manifest.id }));
  const featurePages = $derived(pagesOf(plugins.features));
  const pluginPages = $derived(pagesOf([...plugins.all, ...plugins.extra]));
  const allPages = $derived([...featurePages, ...pluginPages]);
  /** Everything that registered commands with shortcuts, core features first. */
  const shortcutGroups = $derived(
    [...plugins.features, ...plugins.all, ...plugins.extra]
      .map((p) => ({ id: p.manifest.id, name: p.manifest.name, commands: commandsOf(p.manifest.id) }))
      .filter((g) => g.commands.length),
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

  function setAppearance<K extends keyof typeof settings.appearance>(key: K, value: (typeof settings.appearance)[K]) {
    settings.appearance[key] = value;
    settings.saveAppearance();
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
    `flex w-full items-center gap-2 truncate rounded-md px-2.5 py-1.5 text-left text-sm transition-colors ${active ? "bg-zinc-800 font-medium text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200"}`;
</script>

<svelte:window onkeydown={ui.modal === SETTINGS ? keydown : undefined} />

{#if ui.modal === SETTINGS}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-40 bg-black/50 backdrop-blur-[2px]" onclick={close}></div>
  <div
    class="fixed top-1/2 left-1/2 z-50 flex h-[min(640px,88vh)] w-[min(880px,94vw)] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-2xl"
    role="dialog"
    aria-modal="true"
    aria-label="Settings"
  >
    <nav class="w-54 shrink-0 overflow-y-auto border-r border-zinc-800 bg-zinc-950/60 p-2">
      <div class="px-2.5 pt-2 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">Options</div>
      <button class={navBtn(page === "editor")} onclick={() => (page = "editor")}>
        <FilePenLine size={14} strokeWidth={1.75} class={page === "editor" ? "text-violet-400" : "text-zinc-500"} />
        <span class="truncate">Editor</span>
      </button>
      <button class={navBtn(page === "appearance")} onclick={() => (page = "appearance")}>
        <Palette size={14} strokeWidth={1.75} class={page === "appearance" ? "text-violet-400" : "text-zinc-500"} />
        <span class="truncate">Appearance</span>
      </button>
      <button class={navBtn(page === "shortcuts")} onclick={() => (page = "shortcuts")}>
        <Keyboard size={14} strokeWidth={1.75} class={page === "shortcuts" ? "text-violet-400" : "text-zinc-500"} />
        <span class="truncate">Keyboard shortcuts</span>
      </button>
      {#each featurePages as p (p.id)}
        <button class={navBtn(page === p.id)} onclick={() => (page = p.id)}>
          <span class="truncate">{p.label}</span>
        </button>
      {/each}
      <div class="px-2.5 pt-4 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">Plugins</div>
      <button class={navBtn(page === "plugins")} onclick={() => (page = "plugins")}>
        <Puzzle size={14} strokeWidth={1.75} class={page === "plugins" ? "text-violet-400" : "text-zinc-500"} />
        <span class="truncate">Core plugins</span>
      </button>
      <button class={navBtn(page === "store")} onclick={() => (page = "store")}>
        <Store size={14} strokeWidth={1.75} class={page === "store" ? "text-violet-400" : "text-zinc-500"} />
        <span class="truncate">Plugin store</span>
      </button>
      {#if pluginPages.length}
        <div class="px-2.5 pt-4 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">Plugin options</div>
        {#each pluginPages as p (p.id)}
          <button class={navBtn(page === p.id)} onclick={() => (page = p.id)}>
            <span class="truncate">{p.label}</span>
          </button>
        {/each}
      {/if}
    </nav>

    <section class="flex min-w-0 flex-1 flex-col">
      <header class="flex items-center justify-between border-b border-zinc-800 px-6 py-3">
        <h2 class="text-sm font-medium text-zinc-100">
          {page === "editor"
            ? "Editor"
            : page === "appearance"
              ? "Appearance"
              : page === "shortcuts"
                ? "Keyboard shortcuts"
                : page === "plugins"
                  ? "Core plugins"
                  : page === "store"
                    ? "Plugin store"
                    : (allPages.find((p) => p.id === page)?.label ?? "")}
        </h2>
        <button class="rounded-md p-1 text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200" aria-label="Close" onclick={close}>
          <X size={16} />
        </button>
      </header>

      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-2">
        {#if page === "editor"}
          <Setting name="Limit line length" description="Shows text at a comfortable reading width instead of across the whole window.">
            <Toggle label="Limit line length" checked={settings.app.readableLineLength} onchange={(v) => setApp("readableLineLength", v)} />
          </Setting>
          <Setting name="Line numbers" description="Shows line numbers to the left of the text.">
            <Toggle label="Line numbers" checked={settings.app.lineNumbers} onchange={(v) => setApp("lineNumbers", v)} />
          </Setting>
          <Setting name="Spellcheck" description="Marks typos with the system spellchecker.">
            <Toggle label="Spellcheck" checked={settings.app.spellcheck} onchange={(v) => setApp("spellcheck", v)} />
          </Setting>
          <Setting name="Default view" description="How a note opens: read, edited in the live preview, or as source.">
            <Select
              label="Default view"
              value={settings.app.viewMode}
              options={[
                { value: "read", label: "Read" },
                { value: "edit", label: "Edit (live preview)" },
                { value: "source", label: "Source" },
              ]}
              onchange={(v: ViewMode) => setApp("viewMode", v)}
            />
          </Setting>
        {:else if page === "appearance"}
          <div class="border-b border-zinc-800/70 py-3">
            <div class="text-sm text-zinc-100">Theme</div>
            <div class="mt-0.5 text-xs text-zinc-500">Color scheme for panels, editor, and reading view.</div>
            <div class="mt-3 grid grid-cols-4 gap-2">
              {#each THEMES as t (t.id)}
                {@const active = settings.appearance.theme === t.id}
                <button
                  type="button"
                  class="flex items-center gap-2 rounded-lg border px-2.5 py-2 text-left text-xs font-medium transition-colors {active
                    ? 'border-violet-500 bg-violet-500/10 text-zinc-100'
                    : 'border-zinc-800 bg-zinc-950/60 text-zinc-400 hover:border-zinc-700 hover:text-zinc-200'}"
                  aria-pressed={active}
                  onclick={() => setAppearance("theme", t.id)}
                >
                  <span
                    class="flex h-5 w-5 shrink-0 items-center justify-center rounded-md border border-black/15 shadow-inner"
                    style:background={t.bg}
                  >
                    <span class="h-2 w-2 rounded-full" style:background={t.panel} style:outline="1px solid {t.text}"></span>
                  </span>
                  <span class="truncate">{t.label}</span>
                </button>
              {/each}
            </div>
          </div>

          <Setting name="Accent color" description="Highlight color for active tabs, links, buttons, and focus rings.">
            <div class="flex items-center gap-2" role="radiogroup" aria-label="Accent color">
              {#each ACCENTS as a (a.id)}
                {@const active = settings.appearance.accentColor === a.id}
                <button
                  type="button"
                  class="flex h-6 w-6 items-center justify-center rounded-full border transition-transform hover:scale-110 {active
                    ? 'border-zinc-100 ring-2 ring-zinc-500/40'
                    : 'border-black/20'}"
                  style:background-color={a.color}
                  title={a.label}
                  aria-label={a.label}
                  aria-checked={active}
                  role="radio"
                  onclick={() => setAppearance("accentColor", a.id)}
                >
                  {#if active}
                    <Check size={12} strokeWidth={2.5} class="text-white drop-shadow" />
                  {/if}
                </button>
              {/each}
            </div>
          </Setting>

          <Setting name="Editor font" description="Font family used in notes and the reading view.">
            <Select
              label="Editor font"
              value={settings.appearance.editorFont}
              options={[
                { value: "sans", label: "Sans (Inter)" },
                { value: "serif", label: "Serif (Georgia)" },
                { value: "mono", label: "Monospace (JetBrains Mono)" },
              ]}
              onchange={(v: EditorFont) => setAppearance("editorFont", v)}
            />
          </Setting>

          <Setting name="Interface font" description="Font family used in sidebars, tabs, and menus.">
            <Select
              label="Interface font"
              value={settings.appearance.interfaceFont}
              options={[
                { value: "sans", label: "Sans (Inter)" },
                { value: "system", label: "System UI" },
                { value: "mono", label: "Monospace (JetBrains Mono)" },
              ]}
              onchange={(v: InterfaceFont) => setAppearance("interfaceFont", v)}
            />
          </Setting>

          <Setting name="Editor font size" description="Note text size in pixels (default {APPEARANCE_DEFAULTS.fontSize} px).">
            <div class="flex items-center gap-2.5">
              <input
                type="range"
                min="10"
                max="28"
                step="1"
                aria-label="Editor font size"
                class="w-36 accent-violet-500"
                value={settings.appearance.fontSize}
                oninput={(e) => setSize("fontSize", e.currentTarget.value)}
              />
              <span class="w-11 text-right text-xs text-zinc-300 tabular-nums">{settings.appearance.fontSize} px</span>
              {#if settings.appearance.fontSize !== APPEARANCE_DEFAULTS.fontSize}
                <button
                  type="button"
                  class="rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                  title="Reset to {APPEARANCE_DEFAULTS.fontSize} px"
                  aria-label="Reset editor font size"
                  onclick={() => setAppearance("fontSize", APPEARANCE_DEFAULTS.fontSize)}
                >
                  <RotateCcw size={12} />
                </button>
              {/if}
            </div>
          </Setting>

          <Setting name="Interface font size" description="Sidebar, menus, and dialogs (default {APPEARANCE_DEFAULTS.interfaceFontSize} px).">
            <div class="flex items-center gap-2.5">
              <input
                type="range"
                min="10"
                max="28"
                step="1"
                aria-label="Interface font size"
                class="w-36 accent-violet-500"
                value={settings.appearance.interfaceFontSize}
                oninput={(e) => setSize("interfaceFontSize", e.currentTarget.value)}
              />
              <span class="w-11 text-right text-xs text-zinc-300 tabular-nums">{settings.appearance.interfaceFontSize} px</span>
              {#if settings.appearance.interfaceFontSize !== APPEARANCE_DEFAULTS.interfaceFontSize}
                <button
                  type="button"
                  class="rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                  title="Reset to {APPEARANCE_DEFAULTS.interfaceFontSize} px"
                  aria-label="Reset interface font size"
                  onclick={() => setAppearance("interfaceFontSize", APPEARANCE_DEFAULTS.interfaceFontSize)}
                >
                  <RotateCcw size={12} />
                </button>
              {/if}
            </div>
          </Setting>
        {:else if page === "shortcuts"}
          {#each shortcutGroups as g (g.id)}
            <div class="pt-4 pb-1 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">{g.name}</div>
            {#each g.commands as c (c.fullId)}
              <div class="flex items-center justify-between gap-4 border-b border-zinc-800/40 py-1.5 text-sm text-zinc-300 last:border-b-0">
                <span class="truncate">{c.name}</span>
                <span class="flex shrink-0 gap-1">
                  {#each c.hotkeys! as h, i (i)}
                    <kbd>{hotkeyLabel(h)}</kbd>
                  {/each}
                </span>
              </div>
            {/each}
          {/each}
        {:else if page === "plugins"}
          <p class="py-3 text-xs leading-relaxed text-zinc-400">
            Optional functions that come with dvoix. Turned-off plugins disappear from the sidebar, menus and keyboard
            shortcuts, your files stay unchanged. Basic functions such as the file explorer, search, tags, backlinks, checkboxes,
            tabs and views are always on.
          </p>
          {#each plugins.all as p (p.manifest.id)}
            {@const id = p.manifest.id}
            {@const on = plugins.isEnabled(id)}
            <Setting name={p.manifest.name} description={p.manifest.description}>
              <div class="flex items-center gap-3">
                {#if on && plugins.settingTabs.some((t) => t.pluginId === id)}
                  <button class="text-xs text-violet-300 hover:underline" onclick={() => (page = `plugin:${id}`)}>Options</button>
                {/if}
                <Toggle label="{p.manifest.name} on/off" checked={on} onchange={(v) => plugins.setEnabled(id, v)} />
              </div>
            </Setting>
            {#if plugins.errors[id]}
              <p class="-mt-2 pb-2 text-xs text-red-400">Could not be loaded: {plugins.errors[id]}</p>
            {/if}
            {#if on && commandsOf(id).length}
              <div class="-mt-2 flex flex-wrap gap-x-4 gap-y-1 pb-3 text-[11px] text-zinc-500">
                {#each commandsOf(id) as c (c.fullId)}
                  <span class="inline-flex items-center gap-1">{c.name}: <kbd>{hotkeyLabel(c.hotkeys![0])}</kbd></span>
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

      <footer class="flex items-center gap-2 border-t border-zinc-800 bg-zinc-950/40 px-6 py-2 text-[11px] text-zinc-500">
        <Puzzle size={12} />
        Saved in <code class="truncate text-zinc-400" title="{vault.info?.path}/.dvoix">{vault.info?.name}/.dvoix/</code>
      </footer>
    </section>
  </div>
{/if}
