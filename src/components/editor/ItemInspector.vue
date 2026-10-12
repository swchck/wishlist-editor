<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  CircleAlertIcon,
  CircleCheckIcon,
  ImagePlusIcon,
  Loader2Icon,
  PlusIcon,
  XIcon,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import CardThumb from "./CardThumb.vue";
import { GIFT_TYPE_LABELS, photosOf, type GiftType } from "@/lib/wishlist/schema";
import { formatPrice } from "@/lib/wishlist/format";
import { useWishlist, type Card } from "@/stores/wishlist";
import { picturesOf, uploading, uploadPhotos } from "./upload";

interface Link {
  label: string;
  url: string;
}

const props = defineProps<{ card: Card }>();
const store = useWishlist();
const d = computed(() => props.card.data);
const ro = computed(() => store.frozen || !props.card.file);

const photos = computed(() => photosOf(d.value));
// which photo the preview shows; the site's card cycles through them the same way
const shownPhoto = ref(0);
watch(
  () => props.card.slug,
  () => (shownPhoto.value = 0),
);
watch(photos, (list) => {
  if (shownPhoto.value >= list.length) shownPhoto.value = Math.max(0, list.length - 1);
});
function step(by: number) {
  const n = photos.value.length;
  if (n) shownPhoto.value = (shownPhoto.value + by + n) % n;
}
const links = computed<Link[]>(() => (Array.isArray(d.value.links) ? (d.value.links as Link[]) : []));
const price = computed(() => formatPrice(d.value.price, d.value.currency ?? store.config?.defaultCurrency));
const typeLabel = computed(() => GIFT_TYPE_LABELS[(d.value.type as GiftType) ?? "вещь"] ?? String(d.value.type));
const errorList = computed(() => Object.entries(props.card.errors));

function set(key: string, value: unknown) {
  store.updateField(props.card.slug, key, value);
}

function setLinks(next: Link[]) {
  set("links", next);
}

function editLink(i: number, patch: Partial<Link>) {
  setLinks(links.value.map((l, j) => (j === i ? { ...l, ...patch } : l)));
}

function setPhotos(next: string[]) {
  store.setPhotos(props.card.slug, next);
}

const picker = ref<HTMLInputElement>();
async function addPictures(pictures: File[]) {
  const before = photos.value.length;
  await uploadPhotos(props.card.slug, pictures);
  if (photos.value.length > before) shownPhoto.value = before;
}
function onPicked(e: Event) {
  const input = e.target as HTMLInputElement;
  void addPictures(picturesOf(input.files));
  input.value = "";
}
// a screenshot copied to the clipboard pastes straight in; plain text pastes as usual
function onPaste(e: ClipboardEvent) {
  const pictures = picturesOf(e.clipboardData?.items);
  if (!pictures.length) return;
  e.preventDefault();
  void addPictures(pictures);
}
function addUrl(el: HTMLInputElement) {
  const url = el.value.trim();
  el.value = "";
  if (!url) return;
  setPhotos([...photos.value, url]);
  shownPhoto.value = photos.value.length - 1;
}

const FIELD_NAMES: Record<string, string> = {
  title: "Название",
  price: "Цена",
  currency: "Валюта",
  deadline: "Срок",
  url: "Ссылка",
  image: "Фото",
  images: "Фото",
  links: "Доп. ссылки",
  type: "Тип",
  priority: "Приоритет",
};

function fieldName(path: string) {
  const [head, idx] = path.split(".");
  return `${FIELD_NAMES[head] ?? head}${idx ? ` №${Number(idx) + 1}` : ""}`;
}
</script>

<template>
  <ScrollArea class="bg-muted/30 min-h-0 flex-1">
    <div class="grid gap-5 p-4">
      <section class="grid gap-2">
        <h4 class="text-muted-foreground text-[11px] font-semibold tracking-wider uppercase">Как увидят гости</h4>
        <div class="bg-card overflow-hidden rounded-xl border" :class="{ 'opacity-50': d.hidden }">
          <div class="group/photo relative">
            <CardThumb
              :src="photos[shownPhoto]"
              :title="String(d.title ?? card.slug)"
              class="aspect-[4/3] w-full rounded-none border-0 text-2xl"
            />
            <template v-if="photos.length > 1">
              <button
                v-for="dir in [-1, 1]"
                :key="dir"
                type="button"
                class="bg-background/80 absolute top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-full opacity-0 shadow transition-opacity group-hover/photo:opacity-100 focus-visible:opacity-100"
                :class="dir < 0 ? 'left-2' : 'right-2'"
                :aria-label="dir < 0 ? 'Предыдущее фото' : 'Следующее фото'"
                @click="step(dir)"
              >
                <ChevronLeftIcon v-if="dir < 0" class="size-4" />
                <ChevronRightIcon v-else class="size-4" />
              </button>
              <div class="absolute inset-x-0 bottom-2 flex justify-center gap-1.5">
                <button
                  v-for="(_, i) in photos"
                  :key="i"
                  type="button"
                  :aria-label="`Фото ${i + 1}`"
                  class="size-1.5 rounded-full shadow"
                  :class="i === shownPhoto ? 'bg-white' : 'bg-white/50'"
                  @click="shownPhoto = i"
                />
              </div>
            </template>
          </div>
          <div class="grid gap-0.5 p-3">
            <div class="text-muted-foreground text-xs">{{ [d.category, typeLabel].filter(Boolean).join(" · ") }}</div>
            <div class="text-sm leading-snug font-semibold">{{ d.title || "Без названия" }}</div>
            <div v-if="price" class="text-[15px] font-bold tabular-nums">{{ price }}</div>
          </div>
        </div>
      </section>

      <section class="grid gap-2">
        <h4 class="text-muted-foreground text-[11px] font-semibold tracking-wider uppercase">Проверка</h4>
        <div v-if="card.parseError" class="text-destructive flex gap-2 text-xs">
          <CircleAlertIcon class="size-4 shrink-0" /> Файл не читается
        </div>
        <div v-else-if="!errorList.length" class="flex items-center gap-2 text-xs">
          <CircleCheckIcon class="size-4 text-emerald-600 dark:text-emerald-400" /> Карточка соберётся на сайте
        </div>
        <ul v-else class="grid gap-1.5 text-xs">
          <li v-for="[path, msg] in errorList" :key="path" class="text-destructive flex gap-2">
            <CircleAlertIcon class="mt-px size-3.5 shrink-0" />
            <span
              ><b class="font-semibold">{{ fieldName(path) }}:</b> {{ msg }}</span
            >
          </li>
        </ul>
      </section>

      <section v-if="!card.parseError" class="grid gap-2">
        <h4 class="text-muted-foreground text-[11px] font-semibold tracking-wider uppercase">Фото</h4>
        <div class="grid grid-cols-[repeat(auto-fill,minmax(4rem,1fr))] gap-2">
          <div v-for="(p, i) in photos" :key="`${i}:${p}`" class="group/tile relative">
            <button
              type="button"
              class="block aspect-square w-full rounded-md ring-offset-2 outline-none focus-visible:ring-2"
              :class="i === shownPhoto ? 'ring-primary ring-2' : ''"
              :title="p"
              :aria-label="`Показать фото ${i + 1}`"
              @click="shownPhoto = i"
            >
              <CardThumb :src="p" :title="String(i + 1)" class="size-full" />
            </button>
            <button
              v-if="!ro"
              type="button"
              class="bg-background/90 absolute -top-1.5 -right-1.5 grid size-5 place-items-center rounded-full border opacity-0 shadow-sm transition-opacity group-hover/tile:opacity-100 focus-visible:opacity-100"
              aria-label="Убрать фото"
              @click="store.removePhoto(card.slug, i)"
            >
              <XIcon class="size-3" />
            </button>
          </div>
          <div
            v-for="n in uploading"
            :key="`up${n}`"
            class="bg-muted grid aspect-square place-items-center rounded-md border"
          >
            <Loader2Icon class="text-muted-foreground size-4 animate-spin" />
          </div>
          <button
            v-if="!ro"
            type="button"
            class="text-muted-foreground hover:text-foreground hover:border-foreground/40 grid aspect-square place-items-center rounded-md border border-dashed transition-colors"
            aria-label="Добавить фото с компьютера"
            title="Добавить фото с компьютера"
            @click="picker?.click()"
          >
            <ImagePlusIcon class="size-5" />
          </button>
        </div>
        <input ref="picker" type="file" accept="image/*" multiple hidden @change="onPicked" />
        <p v-if="!ro" class="text-muted-foreground text-[11px] leading-snug">
          Перетащите фото в окно, вставьте из буфера (⌘V) или добавьте ссылку:
        </p>
        <Input
          id="photo-new"
          class="h-8 font-mono text-xs"
          placeholder="https://… и Enter"
          :readonly="ro"
          @paste="onPaste"
          @keydown.enter.prevent="addUrl($event.target as HTMLInputElement)"
        />
        <Input
          v-if="photos[shownPhoto] && /^https?:/.test(photos[shownPhoto])"
          :id="`photo-${shownPhoto}`"
          :model-value="photos[shownPhoto]"
          class="h-8 font-mono text-xs"
          :readonly="ro"
          aria-label="Ссылка на выбранное фото"
          @change="
            setPhotos(photos.map((x, j) => (j === shownPhoto ? ($event.target as HTMLInputElement).value.trim() : x)))
          "
        />
      </section>

      <section v-if="!card.parseError" class="grid gap-2">
        <h4 class="text-muted-foreground text-[11px] font-semibold tracking-wider uppercase">Доп. ссылки</h4>
        <div v-for="(l, i) in links" :key="i" class="grid gap-1 rounded-lg border p-2">
          <div class="flex gap-1.5">
            <Input
              :id="`link-label-${i}`"
              :model-value="l.label"
              class="h-8 text-xs"
              placeholder="Подпись"
              :readonly="ro"
              @change="editLink(i, { label: ($event.target as HTMLInputElement).value })"
            />
            <Button
              variant="ghost"
              size="icon-sm"
              :disabled="ro"
              aria-label="Убрать ссылку"
              @click="setLinks(links.filter((_, j) => j !== i))"
            >
              <XIcon />
            </Button>
          </div>
          <Input
            :id="`link-url-${i}`"
            :model-value="l.url"
            class="h-8 font-mono text-xs"
            placeholder="https://…"
            :readonly="ro"
            @change="editLink(i, { url: ($event.target as HTMLInputElement).value.trim() })"
          />
        </div>
        <Button
          variant="ghost"
          size="sm"
          class="justify-start px-1"
          :disabled="ro"
          @click="setLinks([...links, { label: 'Альтернатива', url: 'https://' }])"
        >
          <PlusIcon /> Ссылка
        </Button>
      </section>
    </div>
  </ScrollArea>
</template>
