import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { link } from "./lib/link";

mount(App, { target: document.getElementById("app")! });
link.start();
if ("serviceWorker" in navigator) navigator.serviceWorker.register("/sw.js").catch(() => {});
