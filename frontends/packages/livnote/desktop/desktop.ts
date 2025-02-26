import App from "./App.svelte";
import { mount } from "svelte";
import "./tailwind.css";
import "./app.css";

const app = mount(App, {
	target: document.getElementById("app"),
});
export default app;
