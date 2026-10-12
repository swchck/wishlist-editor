import { defineStore } from "pinia";
import { computed, markRaw, ref, shallowReactive } from "vue";
import { api, errorMessage } from "@/lib/api";
import {
  itemData,
  mergeBody,
  newItemText,
  parseItem,
  serializeItem,
  setField,
  type ItemFile,
} from "@/lib/wishlist/item-file";
import {
  checkLocalPhotos,
  ConfigSchema,
  SCHEMA_VERSION,
  validateItem,
  type FieldErrors,
  type GiftType,
  type ItemData,
  type WishlistConfig,
} from "@/lib/wishlist/schema";
import { slugify, uniqueSlug } from "@/lib/wishlist/slug";

const SAVE_DELAY_MS = 600;
const RECENT_KEY = "recent-workspaces";

export interface Card {
  slug: string;
  /** `null` when the file could not be parsed; the card is then shown read-only. */
  file: ItemFile | null;
  parseError?: string;
  data: Record<string, unknown>;
  errors: FieldErrors;
  savedText: string;
}

export type SaveState = "idle" | "pending" | "saving" | "error";

function validate(data: Record<string, unknown>, files: ReadonlySet<string>): FieldErrors {
  const res = validateItem(data);
  return { ...checkLocalPhotos(data as ItemData, files), ...(res.ok ? {} : res.errors) };
}

function loadCard(slug: string, text: string, files: ReadonlySet<string>): Card {
  try {
    const file = markRaw(parseItem(text));
    const data = itemData(file);
    return { slug, file, data, errors: validate(data, files), savedText: text };
  } catch (e) {
    return { slug, file: null, parseError: errorMessage(e), data: {}, errors: {}, savedText: text };
  }
}

function readRecent(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
  } catch {
    return [];
  }
}

export const useWishlist = defineStore("wishlist", () => {
  const root = ref<string | null>(null);
  const config = ref<WishlistConfig | null>(null);
  const cards = shallowReactive(new Map<string, Card>());
  const selected = ref<string | null>(null);
  const saveState = ref<SaveState>("idle");
  const saveError = ref<string | null>(null);
  const recent = ref<string[]>(readRecent());
  let files: ReadonlySet<string> = new Set();

  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  const readOnly = computed(() => config.value?.schemaVersion !== SCHEMA_VERSION);
  // same order as the site: most wanted first
  const list = computed(() =>
    [...cards.values()].sort(
      (a, b) =>
        (Number(a.data.priority) || 3) - (Number(b.data.priority) || 3) ||
        String(a.data.title ?? a.slug).localeCompare(String(b.data.title ?? b.slug), "ru"),
    ),
  );
  const current = computed(() => (selected.value ? (cards.get(selected.value) ?? null) : null));
  const categories = computed(() =>
    [...new Set(list.value.map((c) => c.data.category).filter((c): c is string => typeof c === "string"))].sort(),
  );

  function rememberRecent(path: string) {
    recent.value = [path, ...recent.value.filter((p) => p !== path)].slice(0, 5);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(recent.value));
    } catch {
      // private storage can be unavailable; the list is only a convenience
    }
  }

  async function open(path: string) {
    await flushAll();
    const snap = await api.openWorkspace(path);
    const cfg = ConfigSchema.parse(JSON.parse(snap.configText));
    cards.clear();
    files = new Set(snap.files);
    for (const raw of snap.items) cards.set(raw.slug, loadCard(raw.slug, raw.text, files));
    root.value = snap.root;
    config.value = cfg;
    selected.value = list.value[0]?.slug ?? null;
    saveState.value = "idle";
    rememberRecent(snap.root);
  }

  async function close() {
    await flushAll();
    root.value = null;
    config.value = null;
    cards.clear();
    selected.value = null;
  }

  function touch(card: Card) {
    const file = card.file!;
    const data = itemData(file);
    cards.set(card.slug, { ...card, data, errors: validate(data, files) });
    schedule(card.slug);
  }

  function updateField(slug: string, key: string, value: unknown) {
    const card = cards.get(slug);
    if (!card?.file || readOnly.value) return;
    setField(card.file, key, value);
    touch(card);
  }

  function updateBody(slug: string, markdown: string) {
    const card = cards.get(slug);
    if (!card?.file || readOnly.value) return;
    const body = mergeBody(card.file.body, markdown);
    if (card.file.body === body) return;
    card.file.body = body;
    touch(card);
  }

  function schedule(slug: string) {
    clearTimeout(timers.get(slug));
    saveState.value = "pending";
    timers.set(
      slug,
      setTimeout(() => void save(slug), SAVE_DELAY_MS),
    );
  }

  async function save(slug: string) {
    timers.delete(slug);
    const card = cards.get(slug);
    if (!card?.file) return;
    const text = serializeItem(card.file);
    if (text === card.savedText) {
      if (!timers.size) saveState.value = "idle";
      return;
    }
    saveState.value = "saving";
    try {
      await api.writeItem(slug, text);
      const latest = cards.get(slug);
      if (latest) cards.set(slug, { ...latest, savedText: text });
      saveError.value = null;
      saveState.value = timers.size ? "pending" : "idle";
    } catch (e) {
      saveError.value = errorMessage(e);
      saveState.value = "error";
    }
  }

  async function flushAll() {
    const pending = [...timers.keys()];
    for (const slug of pending) clearTimeout(timers.get(slug));
    await Promise.all(pending.map(save));
  }

  async function create(title: string, type: GiftType) {
    const slug = uniqueSlug(slugify(title), new Set(cards.keys()));
    const text = newItemText({ title: title.trim(), type, priority: 3 });
    await api.createItem(slug, text);
    cards.set(slug, loadCard(slug, text, files));
    selected.value = slug;
    return slug;
  }

  async function remove(slug: string) {
    clearTimeout(timers.get(slug));
    timers.delete(slug);
    await api.deleteItem(slug);
    const order = list.value.map((c) => c.slug);
    const i = order.indexOf(slug);
    cards.delete(slug);
    if (selected.value === slug) selected.value = order[i + 1] ?? order[i - 1] ?? null;
  }

  async function rename(from: string, to: string) {
    await save(from);
    await api.renameItem(from, to);
    const card = cards.get(from)!;
    cards.delete(from);
    cards.set(to, { ...card, slug: to });
    if (selected.value === from) selected.value = to;
  }

  return {
    root,
    config,
    cards,
    list,
    selected,
    current,
    categories,
    readOnly,
    saveState,
    saveError,
    recent,
    open,
    close,
    updateField,
    updateBody,
    flushAll,
    create,
    remove,
    rename,
  };
});
