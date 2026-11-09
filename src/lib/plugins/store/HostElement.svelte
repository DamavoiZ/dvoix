<script lang="ts">
  // Gives a store plugin a plain DOM element to draw into. Store plugins are
  // not compiled with the app, so they can't hand over Svelte components;
  // `component.ts` wraps their `mount` functions in this.
  import { untrack } from "svelte";
  import type { MountFn, Mounted } from "./api";

  let { mount, hostProps }: { mount: MountFn<any>; hostProps?: Record<string, unknown> } = $props();

  let el: HTMLDivElement;
  let error = $state<string | null>(null);
  let handle: Mounted | undefined;
  let lastContent: unknown;

  /** Plain copy of the props, so a plugin never holds Svelte's reactive object. */
  const snapshot = () => ({ ...(hostProps ?? {}) });

  // Mount again when the file changes (footers and file views get `path`).
  const path = $derived(hostProps?.path);

  $effect(() => {
    void path;
    return untrack(() => {
      error = null;
      el.replaceChildren();
      try {
        const props = snapshot();
        lastContent = props.content;
        const result = mount(el, props);
        handle = typeof result === "function" ? { destroy: result } : (result ?? undefined);
      } catch (e) {
        console.error(e);
        error = String(e);
      }
      const h = handle;
      return () => {
        handle = undefined;
        try {
          h?.destroy?.();
        } catch (e) {
          console.error(e);
        }
        el.replaceChildren();
      };
    });
  });

  // File views: tell the plugin when the file changed on disk.
  $effect(() => {
    const content = hostProps?.content;
    untrack(() => {
      if (content === lastContent) return;
      lastContent = content;
      try {
        handle?.update?.(snapshot());
      } catch (e) {
        console.error(e);
      }
    });
  });
</script>

{#if error}
  <p class="p-3 text-xs text-red-400">Plugin error: {error}</p>
{/if}
<div bind:this={el} class="dvoix-plugin-host h-full min-h-0 w-full"></div>
