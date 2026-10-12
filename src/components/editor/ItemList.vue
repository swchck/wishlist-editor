<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { EyeOffIcon, PlusIcon, SearchIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import NewItemDialog from "./NewItemDialog.vue";
import CardThumb from "./CardThumb.vue";
import { useWishlist } from "@/stores/wishlist";
import { GIFT_TYPES, GIFT_TYPE_LABELS, photosOf, type GiftType } from "@/lib/wishlist/schema";
import { formatPrice } from "@/lib/wishlist/format";

const store = useWishlist();
const query = ref("");
const typeFilter = ref<"all" | GiftType>("all");
// reka's Select cannot hold an empty string, so "all" stands for no filter here too
const categoryFilter = ref("all");
const creating = ref(false);

const presentTypes = computed(() => GIFT_TYPES.filter((t) => store.list.some((c) => (c.data.type ?? "вещь") === t)));
// chips hide below two types, so a filter on a vanished type would have no way back
watch(presentTypes, (types) => {
  if (typeFilter.value !== "all" && (types.length < 2 || !types.includes(typeFilter.value))) typeFilter.value = "all";
});
watch(
  () => store.categories,
  (cats) => {
    if (categoryFilter.value !== "all" && !cats.some((c) => c.name === categoryFilter.value))
      categoryFilter.value = "all";
  },
);
const countOfType = (t: GiftType) => store.list.filter((c) => (c.data.type ?? "вещь") === t).length;

const visible = computed(() => {
  const q = query.value.trim().toLowerCase();
  return store.list.filter((c) => {
    if (typeFilter.value !== "all" && (c.data.type ?? "вещь") !== typeFilter.value) return false;
    if (categoryFilter.value !== "all" && c.data.category !== categoryFilter.value) return false;
    if (!q) return true;
    return [c.data.title, c.data.category, c.slug].some((v) => typeof v === "string" && v.toLowerCase().includes(q));
  });
});

function subtitle(data: Record<string, unknown>) {
  const currency = data.currency ?? store.config?.defaultCurrency;
  return [data.category, formatPrice(data.price, currency)].filter(Boolean).join(" · ");
}
</script>

<template>
  <aside class="flex min-h-0 flex-1 flex-col">
    <div class="grid gap-2.5 border-b p-3">
      <div class="relative">
        <SearchIcon class="text-muted-foreground absolute top-1/2 left-2.5 size-4 -translate-y-1/2" />
        <Input id="item-search" v-model="query" placeholder="Поиск по вишлисту" class="pl-8" />
      </div>
      <!-- chips wrap, so every type fits however narrow the column gets -->
      <div v-if="presentTypes.length > 1" class="flex flex-wrap gap-1" role="radiogroup" aria-label="Тип подарка">
        <button
          v-for="t in ['all', ...presentTypes] as const"
          :key="t"
          type="button"
          role="radio"
          :aria-checked="typeFilter === t"
          class="rounded-full border px-2.5 py-0.5 text-xs whitespace-nowrap"
          :class="
            typeFilter === t
              ? 'bg-foreground text-background border-foreground'
              : 'text-muted-foreground hover:bg-muted'
          "
          @click="typeFilter = t"
        >
          {{ t === "all" ? "Все" : GIFT_TYPE_LABELS[t] }}
          <span class="opacity-60">{{ t === "all" ? store.list.length : countOfType(t) }}</span>
        </button>
      </div>
      <Select v-if="store.categories.length > 1" v-model="categoryFilter">
        <SelectTrigger size="sm" class="w-full text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">Все категории</SelectItem>
          <SelectItem v-for="c in store.categories" :key="c.name" :value="c.name">
            {{ c.name }} <span class="text-muted-foreground">{{ c.count }}</span>
          </SelectItem>
        </SelectContent>
      </Select>
      <Button variant="outline" class="w-full" :disabled="store.frozen" @click="creating = true">
        <PlusIcon /> Новая карточка
      </Button>
    </div>
    <ScrollArea class="min-h-0 flex-1">
      <nav class="grid gap-px p-1.5">
        <button
          v-for="c in visible"
          :key="c.slug"
          class="grid grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-sm"
          :class="c.slug === store.selected ? 'bg-muted' : 'hover:bg-muted/50'"
          @click="store.selected = c.slug"
        >
          <CardThumb :src="photosOf(c.data as never)[0]" :title="String(c.data.title ?? c.slug)" class="size-8" />
          <span class="min-w-0">
            <span class="block truncate font-medium" :class="{ 'text-muted-foreground': c.data.hidden }">
              {{ c.data.title ?? c.slug }}
            </span>
            <span class="text-muted-foreground block truncate text-xs">{{ subtitle(c.data) || c.slug }}</span>
          </span>
          <span class="flex items-center gap-1">
            <span
              v-if="store.changeBySlug.get(c.slug)"
              class="size-1.5 rounded-full"
              :class="store.changeBySlug.get(c.slug) === 'added' ? 'bg-blue-500' : 'bg-amber-500'"
              :title="
                store.changeBySlug.get(c.slug) === 'added' ? 'Новая, не опубликована' : 'Изменена, не опубликована'
              "
            />
            <EyeOffIcon v-if="c.data.hidden" class="text-muted-foreground size-3.5" />
            <span
              v-if="c.parseError || Object.keys(c.errors).length"
              class="bg-destructive size-2 rounded-full"
              title="Есть ошибки"
            />
          </span>
        </button>
        <p v-if="!visible.length" class="text-muted-foreground p-4 text-center text-sm">Ничего не нашлось</p>
      </nav>
    </ScrollArea>
    <NewItemDialog v-model:open="creating" />
  </aside>
</template>
