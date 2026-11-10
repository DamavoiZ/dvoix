<script lang="ts">
  // The buttons in the title row of the window that show and hide the sidebars
  // on the right edge. They come from the plugins (see `RightSidebarSpec.toggle`),
  // the frame only gives them their place next to the window buttons.
  import { plugins } from "../plugins";
</script>

{#each plugins.rightSidebars as sidebar (sidebar.pluginId + sidebar.id)}
  {#if sidebar.toggle}
    <sidebar.toggle />
  {/if}
{/each}
