import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/globals.css";

if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  await import("./dev/browser-mock");
}

// the header leaves room for the window buttons, which macOS hides in full screen
if (navigator.userAgent.includes("Mac") && "__TAURI_INTERNALS__" in window) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  const syncTrafficLights = async () =>
    document.documentElement.classList.toggle("traffic-lights", !(await win.isFullscreen()));
  await syncTrafficLights();
  void win.onResized(() => void syncTrafficLights());
} else if (navigator.userAgent.includes("Mac")) {
  document.documentElement.classList.add("traffic-lights");
}

const dark = window.matchMedia("(prefers-color-scheme: dark)");
const applyTheme = () => document.documentElement.classList.toggle("dark", dark.matches);
applyTheme();
dark.addEventListener("change", applyTheme);

// a file dropped outside an editor drop zone would otherwise replace the whole app with it
for (const type of ["dragover", "drop"]) window.addEventListener(type, (e) => e.preventDefault());

createApp(App).use(createPinia()).mount("#app");
