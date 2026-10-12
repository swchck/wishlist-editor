// Builds latest.json for tauri-plugin-updater from the signed bundles of a draft release
// and uploads it next to them. Run by the release workflow with GH_TOKEN, RELEASE_ID,
// GITHUB_REPOSITORY and GITHUB_REF_NAME set.
import { readFileSync } from "node:fs";
import process from "node:process";

const { GH_TOKEN, RELEASE_ID, GITHUB_REPOSITORY: repo, GITHUB_REF_NAME: tag } = process.env;
const api = `https://api.github.com/repos/${repo}/releases`;
const headers = { authorization: `Bearer ${GH_TOKEN}`, "x-github-api-version": "2022-11-28" };

// updater target → how tauri-action names that platform's bundle
const PLATFORMS = {
  "darwin-aarch64": /_aarch64\.app\.tar\.gz$/,
  "darwin-x86_64": /_x64\.app\.tar\.gz$/,
  "windows-x86_64": /_x64-setup\.exe$/,
};

async function gh(url, init = {}) {
  const res = await fetch(url, { ...init, headers: { ...headers, ...init.headers } });
  if (!res.ok) throw new Error(`${init.method ?? "GET"} ${url}: ${res.status} ${await res.text()}`);
  return res;
}

const assets = await (await gh(`${api}/${RELEASE_ID}/assets?per_page=100`)).json();
const platforms = {};
for (const [target, pattern] of Object.entries(PLATFORMS)) {
  const bundle = assets.find((a) => pattern.test(a.name));
  const sig = bundle && assets.find((a) => a.name === `${bundle.name}.sig`);
  if (!sig) throw new Error(`no signed bundle for ${target}; is TAURI_SIGNING_PRIVATE_KEY set?`);
  const signature = await (
    await gh(`${api}/assets/${sig.id}`, { headers: { accept: "application/octet-stream" } })
  ).text();
  // the draft's own asset URLs die on publish; the tag URL is what works afterwards
  platforms[target] = {
    signature,
    url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(bundle.name)}`,
  };
}

// what's new is everything above the first horizontal rule; the rest is install help
const notes = readFileSync(".github/release-notes.md", "utf8").split(/^---$/m)[0].trim();
const manifest = { version: tag.replace(/^v/, ""), notes, pub_date: new Date().toISOString(), platforms };

const old = assets.find((a) => a.name === "latest.json");
if (old) await gh(`${api}/assets/${old.id}`, { method: "DELETE" });
const upload = `https://uploads.github.com/repos/${repo}/releases/${RELEASE_ID}/assets?name=latest.json`;
await gh(upload, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(manifest, null, 2),
});
console.log(`latest.json: ${manifest.version}, ${Object.keys(platforms).join(", ")}`);
