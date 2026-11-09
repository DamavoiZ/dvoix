// The properties of a note inside the live preview: the `Properties`
// component mounted into a CodeMirror widget in place of the YAML. Its edits
// go into the editor as normal changes, so undo and saving work as for typing.
import { Facet } from "@codemirror/state";
import { EditorView, WidgetType } from "@codemirror/view";
import { mount, unmount } from "svelte";
import Properties from "./Properties.svelte";

/** What a click on a `[[link]]` in a property does; given by the editor. */
export const openLink = Facet.define<(target: string) => void, ((target: string) => void) | undefined>({
  combine: (values) => values[0],
});

interface Mounted {
  props: { source: string };
  component: ReturnType<typeof mount>;
}

const mounted = new WeakMap<HTMLElement, Mounted>();

export class PropertiesWidget extends WidgetType {
  /** `source` is the note from its start up to the closing `---`. */
  constructor(readonly source: string) {
    super();
  }

  eq(other: PropertiesWidget) {
    return other.source === this.source;
  }

  toDOM(view: EditorView) {
    const dom = document.createElement("div");
    dom.className = "cm-lp-properties";
    // Reactive, so an edit updates the mounted component instead of building a new one.
    const props = $state({ source: this.source });
    const component = mount(Properties, {
      target: dom,
      props: {
        get source() {
          return props.source;
        },
        apply: (change) => view.dispatch({ changes: change, userEvent: "input" }),
        onopen: (target) => view.state.facet(openLink)?.(target),
      },
    });
    mounted.set(dom, { props, component });
    return dom;
  }

  /** Keeps the DOM (and the focus in its fields) when only the values changed. */
  updateDOM(dom: HTMLElement) {
    const entry = mounted.get(dom);
    if (!entry) return false;
    entry.props.source = this.source;
    return true;
  }

  destroy(dom: HTMLElement) {
    const entry = mounted.get(dom);
    if (entry) void unmount(entry.component);
    mounted.delete(dom);
  }

  /** The fields handle their own clicks and keys, the editor stays out of them. */
  ignoreEvent() {
    return true;
  }
}
