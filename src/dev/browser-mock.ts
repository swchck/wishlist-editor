// Lets `vite dev` run in a plain browser for UI work: Tauri IPC is answered from an
// in-memory copy of the test fixtures. Imported only in DEV outside Tauri.
//
// A card whose text contains «конфликт» makes the first publish report a conflict.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const fixtures = import.meta.glob("../lib/wishlist/__fixtures__/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const ITEMS = "src/content/items/";
const initial = new Map(
  Object.entries(fixtures).map(([path, text]) => [path.split("/").pop()!.replace(/\.md$/, ""), text]),
);
let committed = new Map(initial);
const files = new Map(initial);
const CONFIG = JSON.stringify({ schemaVersion: 1, title: "Мой вишлист", defaultCurrency: "RUB" });
const USER = { login: "swchck", id: 1, name: "Тестовый пользователь", avatarUrl: "" };
const REPO = { owner: "swchck", name: "wishlist" };
const SITE = "https://swchck.github.io/wishlist/";
let signedIn = false;
let deployTicks = 0;
let pagesTries = 0;

type Args = Record<string, unknown>;
const slugOf = (path: string) => path.slice(ITEMS.length, -".md".length);
const items = () => [...files].sort(([a], [b]) => a.localeCompare(b)).map(([slug, text]) => ({ slug, text }));
const snapshot = () => ({
  root: "/mock/repos/swchck/wishlist",
  configText: CONFIG,
  items: items(),
  files: ["images/tricky-2.webp"],
  writable: true,
});

function changes() {
  const out: { path: string; kind: string }[] = [];
  for (const [slug, text] of files) {
    if (!committed.has(slug)) out.push({ path: `${ITEMS}${slug}.md`, kind: "added" });
    else if (committed.get(slug) !== text) out.push({ path: `${ITEMS}${slug}.md`, kind: "modified" });
  }
  for (const slug of committed.keys()) {
    if (!files.has(slug)) out.push({ path: `${ITEMS}${slug}.md`, kind: "deleted" });
  }
  return out.sort((a, b) => a.path.localeCompare(b.path));
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

mockWindows("main");
mockIPC(
  async (cmd, payload) => {
    const a = (payload ?? {}) as Args;
    switch (cmd) {
      case "auth_status":
        return { user: signedIn ? USER : null, offline: false };
      case "auth_start":
        return { userCode: "WK4F-9Q2M", verificationUri: "https://github.com/login/device", expiresIn: 900 };
      case "auth_wait":
        await wait(2500);
        signedIn = true;
        return USER;
      case "auth_cancel":
        return;
      case "sign_out":
        signedIn = false;
        return;
      case "plugin:opener|open_url":
        console.info("open", a.url);
        return;
      case "list_wishlists":
        await wait(400);
        return [
          { ...REPO, title: "Мой вишлист", listId: "swchck/wishlist", items: files.size, siteUrl: SITE, updatedAt: "" },
        ];
      case "local_wishlists":
        return [REPO];
      case "open_repo":
        await wait(300);
        return {
          repo: REPO,
          snapshot: snapshot(),
          state: { changes: changes(), ahead: 0, behind: 0 },
          pull: "up_to_date",
          siteUrl: SITE,
        };
      case "close_repo":
        return;
      case "reload":
        return snapshot();
      case "repo_status":
        return { changes: changes(), ahead: 0, behind: 0 };
      case "discard_change": {
        const slug = slugOf(String(a.path));
        if (committed.has(slug)) files.set(slug, committed.get(slug)!);
        else files.delete(slug);
        return;
      }
      case "publish": {
        await wait(900);
        const choices = (a.choices ?? {}) as Record<string, string>;
        const hot = changes().find((c) => (files.get(slugOf(c.path)) ?? "").includes("конфликт"));
        if (hot && !choices[hot.path]) {
          const slug = slugOf(hot.path);
          const remote = (committed.get(slug) ?? "").replace(/priority: \d/, "priority: 1");
          return { outcome: "conflicts", files: [{ path: hot.path, local: files.get(slug), remote }] };
        }
        if (!changes().length) return { outcome: "nothingToPublish" };
        committed = new Map(files);
        deployTicks = 0;
        return { outcome: "pushed", sha: "3a91f2c".padEnd(40, "0") };
      }
      case "deploy_status":
        deployTicks++;
        return deployTicks < 3
          ? { status: "in_progress", conclusion: null, htmlUrl: "https://github.com" }
          : { status: "completed", conclusion: "success", htmlUrl: "https://github.com" };
      case "check_name":
        await wait(300);
        return a.name === "wishlist"
          ? "wishlist"
          : a.name === "dotfiles"
            ? "taken"
            : a.name === "half-done"
              ? "unfinished"
              : "free";
      case "create_repo":
        await wait(1500);
        return;
      case "ensure_pages":
        await wait(700);
        // the first attempt plays the "GitHub refused, enable it by hand" path
        pagesTries++;
        return {
          enabled: pagesTries > 1,
          settingsUrl: `https://github.com/swchck/${a.name}/settings/pages`,
          siteUrl: pagesTries > 1 ? `https://swchck.github.io/${a.name}/` : null,
        };
      case "check_reservations":
        await wait(600);
        if (String(a.anonKey).startsWith("sb_secret_")) return "secretKey";
        return String(a.url).includes("old") ? "oldSchema" : String(a.anonKey).length < 5 ? "badKey" : "ready";
      case "finish_setup":
        await wait(1500);
        deployTicks = 0;
        return { sha: "5b02e1d".padEnd(40, "0"), siteUrl: `https://swchck.github.io/${a.name}/` };
      case "local_status":
        return { cloned: true, unpublished: changes().length };
      case "forget_local":
        return;
      case "delete_start":
        return { userCode: "DEL3-7XQ2", verificationUri: "https://github.com/login/device", expiresIn: 900 };
      case "delete_wait":
        await wait(2500);
        return;
      case "delete_cancel":
        return;
      case "write_item":
        files.set(String(a.slug), String(a.text));
        return;
      case "create_item":
        if (files.has(String(a.slug))) {
          throw { kind: "already_exists", message: `карточка «${a.slug}» уже существует` };
        }
        files.set(String(a.slug), String(a.text));
        return;
      case "delete_item":
        files.delete(String(a.slug));
        return;
      case "rename_item":
        files.set(String(a.to), files.get(String(a.from))!);
        files.delete(String(a.from));
        return;
    }
    console.warn("unmocked IPC", cmd, payload);
  },
  { shouldMockEvents: true },
);
