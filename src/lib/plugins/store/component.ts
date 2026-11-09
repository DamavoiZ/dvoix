// Turns the `mount` functions and SVG icons of store plugins into Svelte
// components, so the rest of the app renders them like those of core plugins.
import type { Component } from "svelte";
import { Puzzle } from "@lucide/svelte";
import HostElement from "./HostElement.svelte";
import SvgIcon from "./SvgIcon.svelte";
import type { MountFn } from "./api";

/** A component that passes its props on to `Inner` together with `extra`. */
function bind(Inner: Component<any>, extra: Record<string, unknown>): Component<any> {
  return ((internals: unknown, props: Record<string, unknown>) =>
    (Inner as unknown as (i: unknown, p: unknown) => unknown)(internals, { ...extra, hostProps: props })) as unknown as Component<any>;
}

export function mountComponent<P>(mount: MountFn<P>): Component<any> {
  return bind(HostElement, { mount });
}

export function iconComponent(svg: string | undefined): Component<any> {
  if (!svg) return Puzzle as unknown as Component<any>;
  return ((internals: unknown, props: Record<string, unknown>) =>
    (SvgIcon as unknown as (i: unknown, p: unknown) => unknown)(
      internals,
      new Proxy(props, { get: (t, k) => (k === "svg" ? svg : Reflect.get(t, k)), has: (t, k) => k === "svg" || Reflect.has(t, k) }),
    )) as unknown as Component<any>;
}
