import { mount } from "svelte";
import "./app.css";
import "./lib/filetypes"; // registers the built-in file types (markdown)
import "./lib/plugins"; // makes the core plugins known; they load when a vault opens
import App from "./App.svelte";

const app = mount(App, { target: document.getElementById("app")! });

export default app;
