<script lang="ts">
  // The right edge of the window. What stands there comes from a plugin (see
  // `ctx.addRightSidebar`), e.g. the sidebar with the outline. The frame knows
  // no plugin: it only gives the right edge a place.
  import { plugins } from "../plugins";
</script>

{#each plugins.rightSidebars as sidebar (sidebar.pluginId + sidebar.id)}
  <sidebar.component />
{/each}
