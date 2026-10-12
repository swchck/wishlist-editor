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
  photosOf,
  resolveItemPath,
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

const isRemote = (ref: string) => /^https?:\/\//.test(ref);
const MAX_PHOTO_BYTES = 40 * 1024 * 1024;

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
  /** Object URLs of local photos by path relative to the items folder; read lazily from disk. */
  const photoUrls = shallowReactive(new Map<string, string>());
  const photoRequested = new Set<string>();
  // a read for a reused file name must not land after the newer one
  const photoReads = new Map<string, number>();
  let photoReadSeq = 0;
  const inFlightUploads = new Set<Promise<unknown>>();

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
  /** Categories in use with their card counts, most used first. */
  const categories = computed(() => {
    const counts = new Map<string, number>();
    for (const c of list.value) {
      if (typeof c.data.category === "string" && c.data.category) {
        counts.set(c.data.category, (counts.get(c.data.category) ?? 0) + 1);
      }
    }
    return [...counts]
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name, "ru"));
  });
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
    dropPhotoUrls();
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
    await settleUploads();
    await flushAll();
    clearTimeout(statusTimer);
    if (repo.value) await api.closeRepo();
    dropPhotoUrls();
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
    // an upload landing after the snapshot was read would vanish from `files`
    await settleUploads();
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

  /** Writes the card if it changed; reports whether disk now matches the editor. */
  async function save(slug: string): Promise<boolean> {
    clearTimeout(timers.get(slug));
    timers.delete(slug);
    const card = cards.get(slug);
    if (!card?.file) return false;
    const text = serializeItem(card.file);
    if (text === card.savedText) {
      if (!timers.size) saveState.value = "idle";
      return true;
    }
    saveState.value = "saving";
    try {
      await api.writeItem(slug, text);
      const latest = cards.get(slug);
      if (latest) cards.set(slug, { ...latest, savedText: text });
      saveError.value = null;
      saveState.value = timers.size ? "pending" : "idle";
      scheduleStatus();
      return true;
    } catch (e) {
      saveError.value = errorMessage(e);
      saveState.value = "error";
      return false;
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
    const orphans = photosOf(cards.get(slug)?.data ?? {});
    cards.delete(slug);
    if (selected.value === slug) selected.value = order[i + 1] ?? order[i - 1] ?? null;
    await deleteUnusedPhotos(orphans);
    scheduleStatus();
  }

  /**
   * Returns what an `<img>` can show for a card photo reference: remote URLs as they are,
   * local files as object URLs once read from disk, `undefined` while loading or missing.
   */
  function photoSrc(ref: string | undefined): string | undefined {
    if (!ref || isRemote(ref)) return ref;
    const rel = resolveItemPath(ref);
    if (rel === null || !files.has(rel)) return undefined;
    if (!photoRequested.has(rel)) {
      photoRequested.add(rel);
      void loadPhoto(rel);
    }
    return photoUrls.get(rel);
  }

  async function loadPhoto(rel: string) {
    const gen = generation.value;
    const seq = ++photoReadSeq;
    photoReads.set(rel, seq);
    try {
      const bytes = await api.readPhoto(rel);
      if (gen !== generation.value || photoReads.get(rel) !== seq) return;
      forgetPhotoUrl(rel);
      photoUrls.set(rel, URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" })));
    } catch {
      // the thumbnail falls back to initials and the inspector already lists the missing file
    }
  }

  function forgetPhotoUrl(rel: string) {
    const url = photoUrls.get(rel);
    if (url) URL.revokeObjectURL(url);
    photoUrls.delete(rel);
  }

  function dropPhotoUrls() {
    for (const url of photoUrls.values()) URL.revokeObjectURL(url);
    photoUrls.clear();
    photoRequested.clear();
    photoReads.clear();
  }

  // errors are computed per card on edit; a file appearing or going changes other cards' too
  function setFiles(next: ReadonlySet<string>) {
    files = next;
    for (const card of cards.values()) {
      if (card.file) cards.set(card.slug, { ...card, errors: validate(card.data, files) });
    }
  }

  async function settleUploads() {
    await Promise.allSettled([...inFlightUploads]);
  }

  // one photo goes to `image`, several to `images` — the two shapes the site's README documents
  function setPhotos(slug: string, next: string[]) {
    if (next.length <= 1) {
      updateField(slug, "images", undefined);
      updateField(slug, "image", next[0]);
    } else {
      updateField(slug, "image", undefined);
      updateField(slug, "images", next);
    }
  }

  /**
   * Uploads pictures into the repo and appends them to the card's photos.
   * Returns one error message per picture that could not be added.
   */
  async function addPhotos(slug: string, pictures: Blob[]): Promise<string[]> {
    if (frozen.value || !cards.get(slug)?.file) return pictures.map(() => "карточку сейчас нельзя менять");
    const run = uploadInto(slug, pictures);
    inFlightUploads.add(run);
    try {
      return await run;
    } finally {
      inFlightUploads.delete(run);
    }
  }

  async function uploadInto(slug: string, pictures: Blob[]): Promise<string[]> {
    const added: string[] = [];
    const failed: string[] = [];
    for (const pic of pictures) {
      // checked here too: a 2 GB drop would be copied into the IPC body before Rust says no
      if (pic.size > MAX_PHOTO_BYTES) {
        failed.push("файл больше 40 МБ");
        continue;
      }
      try {
        const ref = await api.addPhoto(slug, new Uint8Array(await pic.arrayBuffer()));
        added.push(ref);
      } catch (e) {
        failed.push(errorMessage(e));
      }
    }
    if (!added.length) return failed;
    setFiles(new Set([...files, ...added.map((r) => resolveItemPath(r)!)]));
    const card = cards.get(slug);
    if (card?.file && !frozen.value) setPhotos(slug, [...photosOf(card.data), ...added]);
    // renamed or deleted mid-upload: nobody references these files, so don't leave them behind
    else await deleteUnusedPhotos(added);
    scheduleStatus();
    return failed;
  }

  /** Removes the card's photo number `index`, and its file once no card shows it. */
  async function removePhoto(slug: string, index: number) {
    const card = cards.get(slug);
    if (!card?.file || frozen.value) return;
    const all = photosOf(card.data);
    const ref = all[index];
    setPhotos(
      slug,
      all.filter((_, i) => i !== index),
    );
    // the card on disk must stop pointing at the file before the file goes, or the site build breaks
    if (ref && (await save(slug))) await deleteUnusedPhotos([ref]);
    scheduleStatus();
  }

  // only uploads live in images/; a photo kept elsewhere was put there by hand, leave it be
  async function deleteUnusedPhotos(refs: string[]) {
    const used = new Set(list.value.flatMap((c) => photosOf(c.data).map(resolveItemPath)));
    // an unparseable card has no data to look at, but its raw text may still name the file
    const unreadable = list.value.filter((c) => !c.file).map((c) => c.savedText);
    for (const ref of refs) {
      const rel = isRemote(ref) ? null : resolveItemPath(ref);
      if (!rel?.startsWith("images/") || used.has(rel) || !files.has(rel)) continue;
      if (unreadable.some((text) => text.includes(rel))) continue;
      try {
        await api.removePhoto(rel);
        const next = new Set(files);
        next.delete(rel);
        setFiles(next);
        forgetPhotoUrl(rel);
        photoRequested.delete(rel);
        photoReads.delete(rel);
      } catch {
        // a leftover file costs a few kilobytes in the repo; not worth failing the delete over
      }
    }
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
    photoSrc,
    addPhotos,
    removePhoto,
    setPhotos,
  };
});
