<script setup lang="ts">
import { computed } from "vue";
import { CircleAlertIcon, CircleCheckIcon, PlusIcon, XIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import CardThumb from "./CardThumb.vue";
import { GIFT_TYPE_LABELS, photosOf, type GiftType } from "@/lib/wishlist/schema";
import { formatPrice } from "@/lib/wishlist/format";
import { useWishlist, type Card } from "@/stores/wishlist";

interface Link {
  label: string;
  url: string;
}

const props = defineProps<{ card: Card }>();
const store = useWishlist();
const d = computed(() => props.card.data);
const ro = computed(() => store.readOnly || !props.card.file);

const photos = computed(() => photosOf(d.value as never));
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

// one photo goes to `image`, several to `images` — the two shapes the site's README documents
function setPhotos(next: string[]) {
  if (next.length <= 1) {
    set("images", undefined);
    set("image", next[0]);
  } else {
    set("image", undefined);
    set("images", next);
  }
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
  <ScrollArea class="bg-muted/30 min-h-0 border-l">
    <div class="grid gap-5 p-4">
      <section class="grid gap-2">
        <h4 class="text-muted-foreground text-[11px] font-semibold tracking-wider uppercase">Как увидят гости</h4>
        <div class="bg-card overflow-hidden rounded-xl border" :class="{ 'opacity-50': d.hidden }">
          <CardThumb
            :src="photos[0]"
            :title="String(d.title ?? card.slug)"
            class="aspect-[4/3] w-full rounded-none border-0 text-2xl"
          />
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
        <div v-for="(p, i) in photos" :key="i" class="flex gap-1.5">
          <Input
            :id="`photo-${i}`"
            :model-value="p"
            class="h-8 font-mono text-xs"
            :readonly="ro"
            @change="setPhotos(photos.map((x, j) => (j === i ? ($event.target as HTMLInputElement).value.trim() : x)))"
          />
          <Button
            variant="ghost"
            size="icon-sm"
            :disabled="ro"
            aria-label="Убрать фото"
            @click="setPhotos(photos.filter((_, j) => j !== i))"
          >
            <XIcon />
          </Button>
        </div>
        <Input
          id="photo-new"
          class="h-8 font-mono text-xs"
          placeholder="https://… и Enter"
          :readonly="ro"
          @keydown.enter.prevent="
            (e: KeyboardEvent) => {
              const el = e.target as HTMLInputElement;
              if (el.value.trim()) setPhotos([...photos, el.value.trim()]);
              el.value = '';
            }
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
