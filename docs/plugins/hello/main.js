// Example store plugin for dvoix: a panel in the sidebar that counts the
// notes of the vault, a command and a setting. Plain JavaScript, no build
// step needed. See docs/plugins/README.md for the whole API.

const icon =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M18 11V6a2 2 0 0 0-4 0v5"/><path d="M14 10V4a2 2 0 0 0-4 0v6"/><path d="M10 10.5V6a2 2 0 0 0-4 0v8"/><path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15"/></svg>';

export default {
  async onload(dvoix) {
    const settings = await dvoix.loadData({ greeting: "Hallo" });

    const countNotes = () => dvoix.vault.list().filter((e) => !e.isDir && e.extension === "md").length;

    dvoix.addPanel({
      id: "panel",
      label: "Hallo Welt",
      icon,
      mount(el) {
        const p = document.createElement("p");
        p.className = "hello-plugin";
        p.textContent = `${settings.greeting}! Dein Vault hat ${countNotes()} Notizen.`;
        el.append(p);
      },
    });

    dvoix.addCommand({
      id: "count",
      name: "Hallo Welt: Notizen zählen",
      run: () => alert(`${settings.greeting}! ${countNotes()} Notizen.`),
    });

    dvoix.addSettingTab({
      mount(el) {
        const label = document.createElement("label");
        label.className = "hello-plugin";
        label.textContent = "Begrüßung ";
        const input = document.createElement("input");
        input.value = settings.greeting;
        input.onchange = () => {
          settings.greeting = input.value;
          dvoix.saveData(settings);
        };
        label.append(input);
        el.append(label);
      },
    });
  },

  onunload() {
    // Everything registered above is removed by dvoix; only own timers or
    // listeners would need cleaning up here (or via dvoix.register).
  },
};
