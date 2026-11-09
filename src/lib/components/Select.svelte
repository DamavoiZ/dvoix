<script lang="ts" generics="T extends string">
  // Dropdown drawn by the app instead of a native <select>. The popup of a
  // native select is drawn by the system: on Linux (WebKitGTK) it follows the
  // GTK theme and stays white in the dark app, whatever the CSS says.
  import { ChevronDown, Check } from "@lucide/svelte";
  import { tick } from "svelte";

  let {
    value,
    options,
    onchange,
    label,
    title,
    small = false,
    class: cls = "",
  }: {
    value: T;
    options: { value: T; label: string }[];
    onchange: (value: T) => void;
    /** Name for screen readers. */
    label: string;
    title?: string;
    /** Smaller text, for tight places like a panel header. */
    small?: boolean;
    class?: string;
  } = $props();

  let trigger: HTMLButtonElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();
  let open = $state(false);
  let active = $state(0);
  let pos = $state({ left: 0, top: 0, width: 0 });

  const current = $derived(options.find((o) => o.value === value));
  const text = $derived(small ? "text-xs" : "text-sm");

  /** Moves the popup into <body>, so no `overflow` or `transform` of a parent clips or shifts it. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  async function show() {
    if (!trigger || options.length === 0) return;
    active = Math.max(0, options.findIndex((o) => o.value === value));
    const r = trigger.getBoundingClientRect();
    pos = { left: r.left, top: r.bottom + 4, width: r.width };
    open = true;
    await tick();
    if (!list) return;
    // Open upwards when there is no room below, and stay inside the window.
    const h = list.offsetHeight;
    const w = list.offsetWidth;
    if (pos.top + h > window.innerHeight - 4 && r.top - h - 4 >= 4) pos.top = r.top - h - 4;
    pos.left = Math.max(4, Math.min(pos.left, window.innerWidth - w - 4));
    list.focus();
    scrollActive();
  }

  function close(refocus = true) {
    open = false;
    if (refocus) trigger?.focus();
  }

  function pick(v: T) {
    close();
    if (v !== value) onchange(v);
  }

  function scrollActive() {
    list?.querySelectorAll("[role='option']")[active]?.scrollIntoView({ block: "nearest" });
  }

  function triggerKey(e: KeyboardEvent) {
    if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
      e.preventDefault();
      void show();
    }
  }

  function listKey(e: KeyboardEvent) {
    // Keep keys away from the window, so Esc doesn't also close a surrounding dialog.
    e.stopPropagation();
    if (e.key === "Escape") close();
    else if (e.key === "Tab") close(false);
    else if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
      e.preventDefault();
      const last = options.length - 1;
      if (e.key === "ArrowDown") active = Math.min(last, active + 1);
      else if (e.key === "ArrowUp") active = Math.max(0, active - 1);
      else active = e.key === "Home" ? 0 : last;
      scrollActive();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(options[active].value);
    }
  }
</script>

<svelte:window onresize={() => open && close(false)} onblur={() => open && close(false)} />

<button
  bind:this={trigger}
  type="button"
  class="flex max-w-full min-w-0 items-center gap-1.5 rounded-md border border-zinc-700 bg-zinc-950 py-1 pr-1.5 text-left text-zinc-200 outline-none focus:border-violet-500 {small
    ? 'pl-1.5'
    : 'pl-2'} {text} {cls}"
  {title}
  aria-label={label}
  aria-haspopup="listbox"
  aria-expanded={open}
  onclick={() => (open ? close() : void show())}
  onkeydown={triggerKey}
>
  <span class="min-w-0 flex-1 truncate">{current?.label ?? value}</span>
  <ChevronDown size={small ? 12 : 14} strokeWidth={1.75} class="shrink-0 text-zinc-500" />
</button>

{#if open}
  <div use:portal>
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="fixed inset-0 z-40" onclick={() => close()} oncontextmenu={(e) => { e.preventDefault(); close(); }}></div>
    <div
      bind:this={list}
      class="fixed z-50 max-h-72 overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-900 p-1 shadow-xl outline-none {text}"
      style:left="{pos.left}px"
      style:top="{pos.top}px"
      style:min-width="{pos.width}px"
      role="listbox"
      aria-label={label}
      tabindex="-1"
      onkeydown={listKey}
    >
      {#each options as o, i (o.value)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="flex cursor-default items-center gap-2 rounded px-2 py-1.5 whitespace-nowrap text-zinc-200 {i === active ? 'bg-zinc-800' : ''}"
          role="option"
          tabindex="-1"
          aria-selected={o.value === value}
          onmouseenter={() => (active = i)}
          onclick={() => pick(o.value)}
        >
          <span class="flex-1">{o.label}</span>
          <Check size={14} strokeWidth={1.75} class="shrink-0 text-violet-400 {o.value === value ? '' : 'invisible'}" />
        </div>
      {/each}
    </div>
  </div>
{/if}
