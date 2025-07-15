import App from "./Mobile.svelte";
import { mount } from "svelte";
import "../tailwind.css";

const app = mount(App, {
	target: document.body,
})
export default app;

