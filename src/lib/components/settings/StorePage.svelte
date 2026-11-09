<script lang="ts">
  // The plugin store page in the settings: browse the plugin list, look at a
  // plugin's details and permissions, install, update, turn on/off and remove
  // store plugins, and the switch for community plugins.
  import { ArrowLeft, BadgeCheck, Download, RefreshCw, Search, ShieldAlert, Trash2, Users } from "@lucide/svelte";
  import Setting from "./Setting.svelte";
  import Toggle from "./Toggle.svelte";
  import type { RegistryEntry } from "../../api";
  import { plugins, pluginStore } from "../../plugins";
  import { PERMISSION_LABELS } from "../../plugins/store/api";

  let { onOptions }: { onOptions: (pluginId: string) => void } = $props();

  let tab = $state<"browse" | "installed">("browse");
  let query = $state("");
  let selected = $state<string | null>(null);
  let confirmCommunity = $state(false);
  let checking = $state(false);

  $effect(() => {
    if (!pluginStore.registry && !pluginStore.loadingRegistry && !pluginStore.registryError) void pluginStore.fetchRegistry();
  });

  const results = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = pluginStore.registry ?? [];
    const hits = q
      ? list.filter((e) => [e.name, e.description, e.author, e.id].some((s) => s.toLowerCase().includes(q)))
      : list;
    // Official first, then by name.
    return [...hits].sort((a, b) => (a.channel === b.channel ? a.name.localeCompare(b.name) : a.channel === "official" ? -1 : 1));
  });

  const entry = $derived(selected ? pluginStore.registry?.find((e) => e.id === selected) : undefined);
  const latest = $derived(selected ? pluginStore.latest[selected] : undefined);

  // The details show version and permissions of the newest release.
  $effect(() => {
    if (entry && !pluginStore.latest[entry.id]) void pluginStore.fetchLatest(entry);
  });

  function errorOf(id: string) {
    return pluginStore.errors[id] ?? plugins.errors[id];
  }

  async function checkUpdates() {
    checking = true;
    try {
      await pluginStore.checkUpdates();
    } finally {
      checking = false;
    }
  }

  function setCommunity(on: boolean) {
    if (on) confirmCommunity = true;
    else void pluginStore.setAllowCommunity(false);
  }

  const btn =
    "inline-flex items-center gap-1.5 rounded-md border border-zinc-700 px-2.5 py-1 text-xs text-zinc-200 hover:bg-zinc-800 disabled:opacity-50 disabled:hover:bg-transparent";
  const primary =
    "inline-flex items-center gap-1.5 rounded-md bg-violet-600 px-2.5 py-1 text-xs text-white hover:bg-violet-500 disabled:opacity-50 disabled:hover:bg-violet-600";
  const tabBtn = (active: boolean) =>
    `rounded-md px-2.5 py-1 text-xs ${active ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:text-zinc-200"}`;
</script>

{#snippet badge(channel: RegistryEntry["channel"])}
  {#if channel === "official"}
    <span class="inline-flex items-center gap-1 rounded bg-violet-500/15 px-1.5 py-px text-[10px] font-medium text-violet-300">
      <BadgeCheck size={11} /> Offiziell
    </span>
  {:else}
    <span class="inline-flex items-center gap-1 rounded bg-zinc-800 px-1.5 py-px text-[10px] font-medium text-zinc-400">
      <Users size={11} /> Community
    </span>
  {/if}
{/snippet}

{#snippet communityWarning()}
  <div class="my-3 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-100">
    <div class="mb-1 flex items-center gap-1.5 font-medium"><ShieldAlert size={14} /> Enable community plugins?</div>
    <p class="text-amber-100/80">
      Community plugins are written by other people. They run with the same rights as dvoix itself and could read your files,
      change them or send them to the internet. The permissions in the manifest show what a plugin intends, but they are not a lock.
      Only install plugins from authors you trust.
    </p>
    <div class="mt-2 flex gap-2">
      <button
        class={primary}
        onclick={() => {
          confirmCommunity = false;
          void pluginStore.setAllowCommunity(true);
        }}>Verstanden, einschalten</button
      >
      <button class={btn} onclick={() => (confirmCommunity = false)}>Abbrechen</button>
    </div>
  </div>
{/snippet}

{#if entry}
  {@const id = entry.id}
  {@const installed = pluginStore.installedManifest(id)}
  {@const blocked = entry.channel === "community" && !pluginStore.file.allowCommunity}
  {@const busy = pluginStore.busy[id]}
  <div class="py-3">
    <button class="mb-3 inline-flex items-center gap-1 text-xs text-zinc-400 hover:text-zinc-200" onclick={() => (selected = null)}>
      <ArrowLeft size={13} /> Back
    </button>
    <div class="flex items-center gap-2">
      <h3 class="text-base font-medium text-zinc-100">{entry.name}</h3>
      {@render badge(entry.channel)}
    </div>
    <div class="mt-0.5 text-xs text-zinc-500">
      by {entry.author} · <span class="select-text">github.com/{entry.repo}</span>
      {#if latest} · Version {latest.version}{/if}
      {#if installed} · installed: {installed.version}{/if}
    </div>
    <p class="mt-3 text-sm text-zinc-300">{entry.description}</p>

    <div class="mt-4 text-[11px] font-medium tracking-wide text-zinc-500 uppercase">Permissions</div>
    {#if !latest}
      <p class="mt-1 text-xs text-zinc-500">{errorOf(id) ? "Could not be loaded." : "Loading …"}</p>
    {:else if latest.permissions.length}
      <ul class="mt-1 list-disc pl-5 text-xs text-zinc-300">
        {#each latest.permissions as p (p)}<li>{PERMISSION_LABELS[p]}</li>{/each}
      </ul>
    {:else}
      <p class="mt-1 text-xs text-zinc-400">None: the plugin does not want access to your files or the editor.</p>
    {/if}
    {#if entry.channel === "community"}
      <p class="mt-2 text-xs text-zinc-500">
        Community plugin: not reviewed by dvoix. The permissions are stated by the author and are not a lock.
      </p>
    {/if}
    {#if pluginStore.needsNewerApp(latest)}
      <p class="mt-2 text-xs text-amber-300">Needs dvoix {latest?.minAppVersion} or newer (you have {pluginStore.appVersion}).</p>
    {/if}

    <div class="mt-4 flex flex-wrap items-center gap-2">
      {#if blocked && !installed}
        <span class="text-xs text-zinc-400">To install, first allow community plugins.</span>
        <button class={btn} onclick={() => setCommunity(true)}>Allow community plugins</button>
      {:else if !installed}
        <button
          class={primary}
          disabled={!!busy || !latest || pluginStore.needsNewerApp(latest) || pluginStore.clashesWithCore(id)}
          onclick={() => pluginStore.install(entry)}
        >
          <Download size={13} /> Installieren
        </button>
      {:else}
        {#if pluginStore.hasUpdate(id)}
          <button class={primary} disabled={!!busy || pluginStore.needsNewerApp(latest)} onclick={() => pluginStore.update(id)}>
            <RefreshCw size={13} /> Update to {latest?.version}
          </button>
        {/if}
        <button class={btn} disabled={!!busy} onclick={() => pluginStore.uninstall(id)}><Trash2 size={13} /> Uninstall</button>
      {/if}
      {#if busy}<span class="text-xs text-zinc-400">{busy}</span>{/if}
    </div>
    {#if pluginStore.clashesWithCore(id)}
      <p class="mt-2 text-xs text-red-400">The ID "{id}" already belongs to a core plugin.</p>
    {/if}
    {#if errorOf(id)}<p class="mt-2 text-xs text-red-400">{errorOf(id)}</p>{/if}
    {#if confirmCommunity}{@render communityWarning()}{/if}
  </div>
{:else}
  <div class="flex items-center gap-2 pt-3">
    <button class={tabBtn(tab === "browse")} onclick={() => (tab = "browse")}>Browse</button>
    <button class={tabBtn(tab === "installed")} onclick={() => (tab = "installed")}>
      Installiert ({pluginStore.installed.length})
    </button>
    <div class="flex-1"></div>
    {#if tab === "browse"}
      <button class={btn} disabled={pluginStore.loadingRegistry} onclick={() => pluginStore.fetchRegistry()}>
        <RefreshCw size={13} /> Reload
      </button>
    {:else}
      <button class={btn} disabled={checking || !pluginStore.installed.length} onclick={checkUpdates}>
        <RefreshCw size={13} /> {checking ? "Checking …" : "Check for updates"}
      </button>
    {/if}
  </div>

  <Setting
    name="Allow community plugins"
    description="Plugins by other people. Official plugins are always allowed, community plugins only with this switch."
  >
    <Toggle label="Allow community plugins" checked={pluginStore.file.allowCommunity} onchange={setCommunity} />
  </Setting>
  {#if confirmCommunity}{@render communityWarning()}{/if}

  {#if tab === "browse"}
    <label class="mt-3 flex items-center gap-2 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 focus-within:border-violet-500">
      <Search size={14} class="text-zinc-500" />
      <input class="flex-1 bg-transparent text-sm text-zinc-200 outline-none" placeholder="Search plugins" bind:value={query} />
    </label>
    {#if pluginStore.loadingRegistry && !pluginStore.registry}
      <p class="py-4 text-xs text-zinc-500">Loading plugin list …</p>
    {:else if pluginStore.registryError}
      <p class="py-4 text-xs text-red-400">The plugin list could not be loaded: {pluginStore.registryError}</p>
    {:else if !results.length}
      <p class="py-4 text-xs text-zinc-500">{query ? "No plugin found." : "No plugins in the list yet."}</p>
    {:else}
      <ul class="py-2">
        {#each results as e (e.id)}
          <li>
            <button
              class="flex w-full items-start gap-3 rounded-md px-2 py-2.5 text-left hover:bg-zinc-800/60"
              onclick={() => (selected = e.id)}
            >
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="truncate text-sm text-zinc-100">{e.name}</span>
                  {@render badge(e.channel)}
                </div>
                <div class="mt-0.5 line-clamp-2 text-xs text-zinc-500">{e.description}</div>
                <div class="mt-0.5 text-[11px] text-zinc-600">by {e.author}</div>
              </div>
              <span class="shrink-0 pt-0.5 text-[11px] text-zinc-500">
                {#if pluginStore.hasUpdate(e.id)}Update{:else if pluginStore.isInstalled(e.id)}Installiert{/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    {#if !pluginStore.installed.length}
      <p class="py-4 text-xs text-zinc-500">No plugins from the store installed yet.</p>
    {/if}
    {#each pluginStore.installed as p (p.manifest.id)}
      {@const id = p.manifest.id}
      {@const blocked = pluginStore.isBlocked(id)}
      {@const on = pluginStore.isEnabled(id)}
      <Setting name="{p.manifest.name} {p.manifest.version}" description={p.manifest.description}>
        <div class="flex items-center gap-3">
          {#if pluginStore.hasUpdate(id)}
            <button class="text-xs text-violet-300 hover:underline" disabled={!!pluginStore.busy[id]} onclick={() => pluginStore.update(id)}>
              Update to {pluginStore.latest[id]?.version}
            </button>
          {/if}
          {#if on && plugins.settingTabs.some((t) => t.pluginId === id)}
            <button class="text-xs text-violet-300 hover:underline" onclick={() => onOptions(id)}>Options</button>
          {/if}
          <button
            class="rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
            aria-label="Uninstall {p.manifest.name}"
            title="Uninstall"
            disabled={!!pluginStore.busy[id]}
            onclick={() => pluginStore.uninstall(id)}
          >
            <Trash2 size={14} />
          </button>
          <Toggle
            label="{p.manifest.name} on/off"
            checked={on}
            disabled={blocked || pluginStore.needsNewerApp(p.manifest)}
            onchange={(v) => pluginStore.setEnabled(id, v)}
          />
        </div>
      </Setting>
      <div class="-mt-2 flex flex-wrap items-center gap-2 pb-3 text-[11px] text-zinc-500">
        {@render badge(pluginStore.channelOf(id))}
        {#if blocked}<span>Locked until community plugins are allowed.</span>{/if}
        {#if pluginStore.needsNewerApp(p.manifest)}<span class="text-amber-300">Needs dvoix {p.manifest.minAppVersion}.</span>{/if}
        {#if pluginStore.busy[id]}<span>{pluginStore.busy[id]}</span>{/if}
        {#if errorOf(id)}<span class="text-red-400">{errorOf(id)}</span>{/if}
      </div>
    {/each}
  {/if}
{/if}
