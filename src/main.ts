import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/globals.css";

if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  await import("./dev/browser-mock");
}

document.documentElement.classList.toggle("mac", navigator.userAgent.includes("Mac"));

const dark = window.matchMedia("(prefers-color-scheme: dark)");
const applyTheme = () => document.documentElement.classList.toggle("dark", dark.matches);
applyTheme();
dark.addEventListener("change", applyTheme);

// a file dropped outside an editor drop zone would otherwise replace the whole app with it
for (const type of ["dragover", "drop"]) window.addEventListener(type, (e) => e.preventDefault());

createApp(App).use(createPinia()).mount("#app");
