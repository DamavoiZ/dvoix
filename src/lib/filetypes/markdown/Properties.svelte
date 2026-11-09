<script lang="ts">
  // The properties (frontmatter) of a note as an editable list, like in
  // Obsidian. Used by the reading view and by the live preview. Every edit
  // becomes one small text change (see `frontmatter.ts`), so the rest of the
  // YAML keeps its formatting.
  import { Plus, X } from "@lucide/svelte";
  import {
    addListItem,
    addProperty,
    parseFrontmatter,
    removeListItem,
    removeProperty,
    renameProperty,
    setScalar,
    type Change,
    type Property,
  } from "./frontmatter";

  interface Props {
    /** The note, or at least its start up to the closing `---`. */
    source: string;
    apply: (change: Change) => void;
    onopen: (target: string) => void;
  }

  let { source, apply, onopen }: Props = $props();

  const fm = $derived(parseFrontmatter(source));

  /** Which value is being typed in: `<index>` of the property. */
  let editing = $state<number | null>(null);
  /** Set by Esc: removing the field also blurs it, and that blur must not save. */
  let cancelled = false;
  let adding = $state(false);
  let newKey = $state("");
  let error = $state("");

  const WIKILINK = /\[\[([^\]\n]+)\]\]/g;
  const dateFormat = new Intl.DateTimeFormat("en-US", { day: "numeric", month: "long", year: "numeric" });

  /** Splits a text into plain parts and `[[links]]`, so links can be clicked. */
  function parts(text: string): { text: string; link?: string }[] {
    const out: { text: string; link?: string }[] = [];
    let last = 0;
    for (const m of text.matchAll(WIKILINK)) {
      if (m.index! > last) out.push({ text: text.slice(last, m.index) });
      const inner = m[1];
      const pipe = inner.indexOf("|");
      const target = (pipe >= 0 ? inner.slice(0, pipe) : inner).split("#")[0].trim();
      out.push({ text: pipe >= 0 ? inner.slice(pipe + 1) : inner, link: target });
      last = m.index! + m[0].length;
    }
    if (last < text.length) out.push({ text: text.slice(last) });
    return out;
  }

  function formatDate(value: string): string {
    const [y, m, d] = value.split("-").map(Number);
    const date = new Date(y, m - 1, d);
    return Number.isNaN(date.getTime()) ? value : dateFormat.format(date);
  }

  const isTags = (p: Property) => p.key.toLowerCase() === "tags";

  /** A value over several lines without its YAML marks: no `|`/`>` line, no common indentation. */
  function rawText(value: string): string {
    let lines = value.split("\n");
    if (/^[|>][+-]?\d*$/.test(lines[0].trim())) lines = lines.slice(1);
    const indent = Math.min(...lines.filter((l) => l.trim()).map((l) => /^\s*/.exec(l)![0].length));
    return lines.map((l) => l.slice(Number.isFinite(indent) ? indent : 0)).join("\n");
  }

  function focus(el: HTMLInputElement) {
    requestAnimationFrame(() => {
      el.focus();
      if (el.type === "text") el.select();
    });
  }

  /** Enter keeps, Esc throws the typing away. */
  function keys(event: KeyboardEvent, commit: () => void, cancel: () => void) {
    if (event.key === "Enter") {
      event.preventDefault();
      commit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cancel();
    }
  }

  function commitValue(prop: Property, input: HTMLInputElement) {
    editing = null;
    if (cancelled) {
      cancelled = false;
      return;
    }
    let value = input.value.trim();
    if (prop.kind === "number") {
      value = value.replace(",", ".");
      if (!/^-?\d+(?:\.\d+)?$/.test(value)) return;
    }
    if (prop.kind === "date" && !/^\d{4}-\d{2}-\d{2}$/.test(value)) return;
    if (value === prop.value) return;
    apply(setScalar(prop, prop.kind, value));
  }

  function commitKey(prop: Property, input: HTMLInputElement) {
    const key = input.value.trim();
    error = "";
    if (key === prop.key) return;
    if (!key || fm?.properties.some((p) => p !== prop && p.key === key)) {
      error = key ? `"${key}" already exists.` : "";
      input.value = prop.key;
      return;
    }
    apply(renameProperty(prop, key));
  }

  function addItem(prop: Property, input: HTMLInputElement) {
    if (!fm) return;
    // `#idea` in the tags is stored as `idea`, the way Obsidian does it.
    const item = isTags(prop) ? input.value.trim().replace(/^#/, "") : input.value.trim();
    if (!item) return;
    input.value = "";
    apply(addListItem(fm, prop, item));
  }

  function commitNewKey() {
    const key = newKey.trim();
    if (!fm || !key) {
      adding = false;
      return;
    }
    if (fm.properties.some((p) => p.key === key)) {
      error = `"${key}" already exists.`;
      return;
    }
    error = "";
    adding = false;
    newKey = "";
    apply(addProperty(fm, key));
  }

  function toggle(prop: Property) {
    apply(setScalar(prop, "boolean", prop.value === "true" ? "false" : "true"));
  }

  const field =
    "rounded bg-transparent px-1.5 py-0.5 text-zinc-200 outline-none hover:bg-zinc-800/60 focus:bg-zinc-800";
</script>

{#if fm}
  <section class="properties" aria-label="Properties">
    <div class="mb-1 px-1.5 text-xs font-medium tracking-wide text-zinc-500 uppercase">Properties</div>
    {#each fm.properties as prop, i (`${i}:${prop.key}`)}
      <div class="group flex items-start gap-2 rounded py-px">
        <input
          class="{field} w-[min(10rem,35%)] shrink-0 truncate text-zinc-400"
          value={prop.key}
          aria-label="Property name"
          spellcheck="false"
          onblur={(e) => commitKey(prop, e.currentTarget)}
          onkeydown={(e) =>
            keys(
              e,
              () => e.currentTarget.blur(),
              () => {
                e.currentTarget.value = prop.key;
                e.currentTarget.blur();
              },
            )}
        />

        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1 {prop.kind === 'list' ? 'pl-1.5' : ''}">
          {#if prop.kind === "boolean"}
            <input
              type="checkbox"
              class="checkbox mx-1.5 my-1"
              checked={prop.value === "true"}
              aria-label={prop.key}
              onchange={() => toggle(prop)}
            />
          {:else if prop.kind === "list"}
            {#each prop.items as item, j (j)}
              <span class="chip">
                {#if isTags(prop)}
                  <span class="text-violet-300">#{item}</span>
                {:else}
                  <!-- One line: whitespace between the parts would show as spaces. -->
                  <span>{#each parts(item) as part, k (k)}{#if part.link}<button class="link" onclick={() => onopen(part.link!)}>{part.text}</button>{:else}{part.text}{/if}{/each}</span>
                {/if}
                <button
                  class="rounded text-zinc-500 hover:text-zinc-200"
                  aria-label={`Remove "${item}"`}
                  onclick={() => fm && apply(removeListItem(fm, prop, j))}
                >
                  <X size={12} />
                </button>
              </span>
            {/each}
            <input
              class="{field} w-28 min-w-28 flex-1 text-sm"
              placeholder={isTags(prop) ? "Add tag" : "Add"}
              aria-label="Add entry to {prop.key}"
              onkeydown={(e) =>
                keys(
                  e,
                  () => addItem(prop, e.currentTarget),
                  () => {
                    e.currentTarget.value = "";
                    e.currentTarget.blur();
                  },
                )}
              onblur={(e) => addItem(prop, e.currentTarget)}
            />
          {:else if prop.kind === "raw"}
            <pre
              class="min-w-0 flex-1 overflow-x-auto px-1.5 py-0.5 font-mono text-xs text-zinc-500"
              title="Edit this value in source view">{rawText(prop.value)}</pre>
          {:else if editing === i}
            <input
              use:focus
              class="{field} w-full min-w-0"
              type={prop.kind === "date" ? "date" : "text"}
              inputmode={prop.kind === "number" ? "decimal" : undefined}
              value={prop.value}
              aria-label={prop.key}
              onblur={(e) => commitValue(prop, e.currentTarget)}
              onkeydown={(e) =>
                keys(
                  e,
                  () => e.currentTarget.blur(),
                  () => {
                    cancelled = true;
                    e.currentTarget.blur();
                  },
                )}
            />
          {:else}
            <!-- The value is shown formatted; a click turns it into a field. -->
            <div
              class="{field} w-full min-w-0 cursor-text"
              role="button"
              tabindex="0"
              aria-label="Edit {prop.key}"
              onclick={() => (editing = i)}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  editing = i;
                }
              }}
            >
              {#if prop.value === ""}
                <span class="text-zinc-600">Empty</span>
              {:else if prop.kind === "date"}
                {formatDate(prop.value)}
              {:else}
                <!-- One line: whitespace between the parts would show as spaces. -->
                {#each parts(prop.value) as part, k (k)}{#if part.link}<button class="link" onclick={(e) => { e.stopPropagation(); onopen(part.link!); }}>{part.text}</button>{:else}{part.text}{/if}{/each}
              {/if}
            </div>
          {/if}
        </div>

        <button
          class="mt-1 rounded p-0.5 text-zinc-600 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 hover:text-zinc-200"
          aria-label={`Remove property "${prop.key}"`}
          title="Remove property"
          onclick={() => fm && apply(removeProperty(fm, prop))}
        >
          <X size={14} />
        </button>
      </div>
    {/each}

    {#if adding}
      <input
        use:focus
        class="{field} w-40"
        placeholder="Name"
        aria-label="Name of the new property"
        spellcheck="false"
        bind:value={newKey}
        onblur={commitNewKey}
        onkeydown={(e) =>
          keys(e, commitNewKey, () => {
            adding = false;
            newKey = "";
            error = "";
          })}
      />
    {:else}
      <button
        class="mt-0.5 flex items-center gap-1 rounded px-1.5 py-0.5 text-sm text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-300"
        onclick={() => (adding = true)}
      >
        <Plus size={14} /> Add property
      </button>
    {/if}
    {#if error}
      <p class="px-1.5 pt-1 text-xs text-red-400">{error}</p>
    {/if}
  </section>
{/if}

<style>
  .properties {
    /* In the editor the widget sits in CodeMirror's content, which keeps
       every space and line break of the markup as text. */
    white-space: normal;
    font-family: var(--font-sans);
    font-size: 0.9rem;
    line-height: 1.6;
    padding-bottom: 0.75rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid var(--color-zinc-800);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    border-radius: 9999px;
    background-color: var(--color-zinc-800);
    padding: 0 0.35rem 0 0.6rem;
    font-size: 0.85em;
    color: var(--color-zinc-200);
    white-space: nowrap;
  }
  .link {
    color: var(--color-violet-300);
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--color-violet-400) 40%, transparent);
    cursor: pointer;
  }
  .checkbox {
    accent-color: var(--color-violet-500);
  }
</style>
