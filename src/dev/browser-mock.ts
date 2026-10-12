// Lets `vite dev` run in a plain browser for UI work: Tauri IPC is answered from an
// in-memory copy of the test fixtures. Imported only in DEV outside Tauri.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const fixtures = import.meta.glob("../lib/wishlist/__fixtures__/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const files = new Map(
  Object.entries(fixtures).map(([path, text]) => [path.split("/").pop()!.replace(/\.md$/, ""), text]),
);
const CONFIG = JSON.stringify({ schemaVersion: 1, title: "Мой вишлист", defaultCurrency: "RUB" });
const ROOT = "/mock/wishlist";

type Args = Record<string, string>;
const items = () => [...files].sort(([a], [b]) => a.localeCompare(b)).map(([slug, text]) => ({ slug, text }));

mockWindows("main");
mockIPC(
  (cmd, payload) => {
    const a = (payload ?? {}) as Args;
    switch (cmd) {
      case "plugin:dialog|open":
        return ROOT;
      case "open_workspace":
        return { root: ROOT, configText: CONFIG, items: items(), files: ["images/tricky-2.webp"] };
      case "list_items":
        return items();
      case "write_item":
        files.set(a.slug, a.text);
        return;
      case "create_item":
        if (files.has(a.slug)) throw { kind: "already_exists", message: `карточка «${a.slug}» уже существует` };
        files.set(a.slug, a.text);
        return;
      case "delete_item":
        files.delete(a.slug);
        return;
      case "rename_item":
        files.set(a.to, files.get(a.from)!);
        files.delete(a.from);
        return;
    }
    console.warn("unmocked IPC", cmd, payload);
  },
  { shouldMockEvents: true },
);
