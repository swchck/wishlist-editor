import { defineStore } from "pinia";
import { computed, markRaw, ref, shallowReactive } from "vue";
import { api, errorMessage, type Change, type Pull, type RepoId, type Snapshot } from "@/lib/api";
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
import { slugOfPath } from "@/lib/wishlist/changes";

const SAVE_DELAY_MS = 600;

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

export const useWishlist = defineStore("wishlist", () => {
  const repo = ref<RepoId | null>(null);
  const siteUrl = ref<string | null>(null);
  /** How opening went: `null` means GitHub was unreachable and the local copy opened as is. */
  const pull = ref<Pull | null>(null);
  const config = ref<WishlistConfig | null>(null);
  const cards = shallowReactive(new Map<string, Card>());
  const selected = ref<string | null>(null);
  const saveState = ref<SaveState>("idle");
  const saveError = ref<string | null>(null);
  const changes = ref<Change[]>([]);
  /** Commits that are local only, e.g. from a publish that could not push. */
  const ahead = ref(0);
  let files: ReadonlySet<string> = new Set();

  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  const writable = ref(true);
  /** Set while publishing: git rewrites files underneath, so edits must wait. */
  const locked = ref(false);
  const readOnly = computed(() => !writable.value || config.value?.schemaVersion !== SCHEMA_VERSION);
  const frozen = computed(() => readOnly.value || locked.value);
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
  const changeBySlug = computed(() => {
    const m = new Map<string, Change["kind"]>();
    for (const c of changes.value) {
      const slug = slugOfPath(c.path);
      if (slug) m.set(slug, c.kind);
    }
    return m;
  });
  const pending = computed(() => changes.value.length + ahead.value > 0);

  /** Bumped whenever cards are re-read from disk; editors key on it to drop stale text. */
  const generation = ref(0);

  function applySnapshot(snap: Snapshot) {
    generation.value++;
    config.value = ConfigSchema.parse(JSON.parse(snap.configText));
    writable.value = snap.writable;
    files = new Set(snap.files);
    cards.clear();
    for (const raw of snap.items) cards.set(raw.slug, loadCard(raw.slug, raw.text, files));
    if (!selected.value || !cards.has(selected.value)) selected.value = list.value[0]?.slug ?? null;
  }

  async function open(id: RepoId) {
    await close();
    const opened = await api.openRepo(id.owner, id.name);
    applySnapshot(opened.snapshot);
    selected.value = list.value[0]?.slug ?? null;
    repo.value = opened.repo;
    siteUrl.value = opened.siteUrl;
    pull.value = opened.pull;
    changes.value = opened.state.changes;
    ahead.value = opened.state.ahead;
    saveState.value = "idle";
  }

  async function close() {
    await flushAll();
    clearTimeout(statusTimer);
    if (repo.value) await api.closeRepo();
    repo.value = null;
    siteUrl.value = null;
    config.value = null;
    cards.clear();
    selected.value = null;
    changes.value = [];
    ahead.value = 0;
  }

  /** Re-reads cards from disk after git moved files underneath (publish, discard). */
  async function reload() {
    await flushAll();
    applySnapshot(await api.reload());
    await refreshStatus();
  }

  async function refreshStatus() {
    clearTimeout(statusTimer);
    if (!repo.value) return;
    const st = await api.repoStatus();
    changes.value = st.changes;
    ahead.value = st.ahead;
  }

  // git status walks the whole tree; after a burst of saves one call is enough
  function scheduleStatus() {
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => void refreshStatus().catch(() => {}), 300);
  }

  async function discard(path: string) {
    await flushAll();
    await api.discardChange(path);
    await reload();
  }

  function touch(card: Card) {
    const file = card.file!;
    const data = itemData(file);
    cards.set(card.slug, { ...card, data, errors: validate(data, files) });
    schedule(card.slug);
  }

  function updateField(slug: string, key: string, value: unknown) {
    const card = cards.get(slug);
    if (!card?.file || frozen.value) return;
    setField(card.file, key, value);
    touch(card);
  }

  function updateBody(slug: string, markdown: string) {
    const card = cards.get(slug);
    if (!card?.file || frozen.value) return;
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
    clearTimeout(timers.get(slug));
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
      scheduleStatus();
    } catch (e) {
      saveError.value = errorMessage(e);
      saveState.value = "error";
    }
  }

  async function flushAll() {
    const slugs = [...timers.keys()];
    await Promise.all(slugs.map(save));
  }

  async function create(title: string, type: GiftType) {
    const slug = uniqueSlug(slugify(title), new Set(cards.keys()));
    const text = newItemText({ title: title.trim(), type, priority: 3 });
    await api.createItem(slug, text);
    cards.set(slug, loadCard(slug, text, files));
    selected.value = slug;
    scheduleStatus();
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
    scheduleStatus();
  }

  async function rename(from: string, to: string) {
    await save(from);
    await api.renameItem(from, to);
    const card = cards.get(from)!;
    cards.delete(from);
    cards.set(to, { ...card, slug: to });
    if (selected.value === from) selected.value = to;
    scheduleStatus();
  }

  return {
    repo,
    siteUrl,
    pull,
    config,
    cards,
    list,
    selected,
    current,
    categories,
    readOnly,
    generation,
    locked,
    frozen,
    saveState,
    saveError,
    changes,
    ahead,
    changeBySlug,
    pending,
    open,
    close,
    reload,
    refreshStatus,
    discard,
    updateField,
    updateBody,
    flushAll,
    create,
    remove,
    rename,
  };
});
