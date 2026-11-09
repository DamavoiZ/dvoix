<script lang="ts">
  // Icon of a store plugin, given as SVG text. Store plugins run with the same
  // rights as the app anyway, so rendering their markup adds no risk.
  let { svg, size = 16, class: cls = "" }: { svg: string; size?: number; strokeWidth?: number; class?: string } = $props();
</script>

<span class="inline-flex shrink-0 items-center justify-center [&>svg]:h-full [&>svg]:w-full {cls}" style="width:{size}px;height:{size}px">
  {@html svg}
</span>
