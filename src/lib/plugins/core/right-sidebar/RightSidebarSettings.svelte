<script lang="ts">
  // The settings of the right sidebar: which panels stand in it.
  import Setting from "../../../components/settings/Setting.svelte";
  import Toggle from "../../../components/settings/Toggle.svelte";
  import { OUTLINE_PANEL, rightSidebarSettings } from "./settings";

  const open = $derived(rightSidebarSettings.value.items.some((i) => i.kind === "panel" && i.id === OUTLINE_PANEL));

  function toggleOutline(checked: boolean) {
    const value = rightSidebarSettings.value;
    value.items = checked
      ? [...value.items, { kind: "panel", id: OUTLINE_PANEL }]
      : value.items.filter((i) => !(i.kind === "panel" && i.id === OUTLINE_PANEL));
    rightSidebarSettings.save();
  }
</script>

<Setting name="Outline" description="Shows the headings of the open note in the right sidebar.">
  <Toggle label="Outline" checked={open} onchange={toggleOutline} />
</Setting>
