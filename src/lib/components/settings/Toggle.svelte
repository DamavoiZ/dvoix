<script lang="ts">
  // On/off switch.
  let {
    checked,
    onchange,
    label,
    disabled = false,
  }: { checked: boolean; onchange: (checked: boolean) => void; label: string; disabled?: boolean } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  class="relative inline-flex h-5 w-9 items-center rounded-full transition-colors disabled:opacity-50 {checked
    ? 'bg-violet-600'
    : 'bg-zinc-700'}"
  onclick={() => onchange(!checked)}
>
  <span class="inline-block h-4 w-4 rounded-full bg-white shadow transition-transform {checked ? 'translate-x-4.5' : 'translate-x-0.5'}"
  ></span>
</button>
