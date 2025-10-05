import App from "./App.svelte";
import { mount } from "svelte";
import "./tailwind.css";

// Import self-hosted fonts
import "@osvauld/fonts/inter";
import "@osvauld/fonts/plus-jakarta-sans";

const app = mount(App, {
	target: document.getElementById("app"),
});
export default app;
