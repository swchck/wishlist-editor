<script setup lang="ts">
import { computed, ref } from "vue";
import { toast } from "vue-sonner";
import { MoreHorizontalIcon, Trash2Icon, PencilIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import Field from "./Field.vue";
import PriorityMeter from "./PriorityMeter.vue";
import MarkdownEditor from "./MarkdownEditor.vue";
import DeleteItemDialog from "./DeleteItemDialog.vue";
import RenameItemDialog from "./RenameItemDialog.vue";
import { COMMON_CURRENCIES, GIFT_TYPES, GIFT_TYPE_LABELS } from "@/lib/wishlist/schema";
import { useWishlist, type Card } from "@/stores/wishlist";

const props = defineProps<{ card: Card }>();
const store = useWishlist();
const deleting = ref(false);
const renaming = ref(false);

const d = computed(() => props.card.data);
const err = computed(() => props.card.errors);
const ro = computed(() => store.readOnly || !props.card.file);

function set(key: string, value: unknown) {
  store.updateField(props.card.slug, key, value);
}

function str(v: unknown) {
  return typeof v === "string" ? v : "";
}

function setPrice(raw: string | number) {
  const s = String(raw).replace(/\s/g, "").replace(",", ".");
  if (s === "") return set("price", undefined);
  const n = Number(s);
  if (Number.isFinite(n)) set("price", n);
}

const currency = computed(() => str(d.value.currency) || store.config?.defaultCurrency || "RUB");
const currencies = computed(() => [...new Set([...COMMON_CURRENCIES, currency.value])]);
const priority = computed({
  get: () => (typeof d.value.priority === "number" ? d.value.priority : 3),
  set: (v: number) => set("priority", v),
});

function onRenamed(slug: string) {
  toast.success(`Файл переименован в ${slug}.md`);
}
</script>

<template>
  <ScrollArea class="min-h-0">
    <div class="mx-auto grid max-w-3xl gap-5 px-8 pt-5 pb-10">
      <div class="text-muted-foreground flex items-center gap-2 font-mono text-xs">
        items/{{ card.slug }}.md
        <div class="flex-1" />
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button variant="ghost" size="icon-sm" :disabled="ro" aria-label="Действия с карточкой">
              <MoreHorizontalIcon />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem @select="renaming = true"><PencilIcon /> Переименовать файл</DropdownMenuItem>
            <DropdownMenuItem variant="destructive" @select="deleting = true"><Trash2Icon /> Удалить</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div
        v-if="card.parseError"
        class="border-destructive/40 bg-destructive/5 text-destructive rounded-lg border p-4 text-sm"
      >
        Файл не удаётся прочитать: {{ card.parseError }}. Исправьте его в текстовом редакторе — здесь он открыт только
        для чтения.
      </div>

      <template v-else>
        <div class="grid gap-1">
          <input
            id="item-title"
            :value="str(d.title)"
            :readonly="ro"
            placeholder="Название подарка"
            class="placeholder:text-muted-foreground/60 w-full bg-transparent text-[26px] leading-tight font-bold tracking-tight outline-none"
            @input="set('title', ($event.target as HTMLInputElement).value)"
          />
          <p v-if="err.title" class="text-destructive text-xs">{{ err.title }}</p>
        </div>

        <div class="grid grid-cols-4 gap-x-3 gap-y-4">
          <Field label="Тип" class="col-span-4">
            <ToggleGroup
              :model-value="str(d.type) || 'вещь'"
              type="single"
              variant="outline"
              class="w-full"
              :disabled="ro"
              @update:model-value="(v) => v && set('type', v)"
            >
              <ToggleGroupItem
                v-for="t in GIFT_TYPES"
                :key="t"
                :value="t"
                class="data-[state=on]:bg-foreground data-[state=on]:text-background flex-1 text-xs"
              >
                {{ GIFT_TYPE_LABELS[t] }}
              </ToggleGroupItem>
            </ToggleGroup>
          </Field>
          <Field label="Категория" for="item-category" class="col-span-2">
            <Input
              id="item-category"
              :model-value="str(d.category)"
              list="known-categories"
              placeholder="Музыка, Кухня…"
              :readonly="ro"
              @update:model-value="(v) => set('category', String(v).trim() || undefined)"
            />
            <datalist id="known-categories">
              <option v-for="c in store.categories" :key="c" :value="c" />
            </datalist>
          </Field>
          <Field label="Желательный срок" for="item-deadline" :error="err.deadline" class="col-span-2">
            <Input
              id="item-deadline"
              type="date"
              :model-value="str(d.deadline)"
              :readonly="ro"
              @update:model-value="(v) => set('deadline', v || undefined)"
            />
          </Field>

          <Field label="Цена" for="item-price" :error="err.price" class="col-span-1">
            <Input
              id="item-price"
              inputmode="decimal"
              class="font-mono tabular-nums"
              :model-value="typeof d.price === 'number' ? String(d.price) : ''"
              placeholder="Не указана"
              :readonly="ro"
              @update:model-value="setPrice"
            />
          </Field>
          <Field label="Валюта" :error="err.currency" class="col-span-1">
            <Select :model-value="currency" :disabled="ro" @update:model-value="(v) => set('currency', v)">
              <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem v-for="c in currencies" :key="c" :value="c">{{ c }}</SelectItem>
              </SelectContent>
            </Select>
          </Field>
          <Field label="Насколько хочу" class="col-span-2">
            <PriorityMeter v-model="priority" :disabled="ro" />
          </Field>

          <Field label="Ссылка на покупку" for="item-url" :error="err.url" class="col-span-4">
            <Input
              id="item-url"
              class="font-mono text-xs"
              :model-value="str(d.url)"
              placeholder="https://… — у свободного пожелания можно оставить пустой"
              :readonly="ro"
              @update:model-value="(v) => set('url', String(v).trim() || undefined)"
            />
          </Field>
          <Field label="Короткое описание для карточки" for="item-description" class="col-span-4">
            <Textarea
              id="item-description"
              rows="2"
              class="min-h-0 resize-none"
              :model-value="str(d.description)"
              placeholder="Пара слов, которые увидят в списке"
              :readonly="ro"
              @update:model-value="(v) => set('description', String(v) || undefined)"
            />
          </Field>
          <label class="col-span-4 flex items-center gap-2.5 text-sm">
            <Switch
              id="item-hidden"
              :model-value="d.hidden === true"
              :disabled="ro"
              @update:model-value="(v) => set('hidden', v ? true : undefined)"
            />
            Спрятать с сайта
            <span class="text-muted-foreground text-xs">— карточка останется в репозитории</span>
          </label>
        </div>

        <div class="border-t pt-2">
          <MarkdownEditor :value="card.file!.body" :readonly="ro" @change="(md) => store.updateBody(card.slug, md)" />
        </div>
      </template>
    </div>
    <DeleteItemDialog v-model:open="deleting" :card="card" />
    <RenameItemDialog v-model:open="renaming" :card="card" @renamed="onRenamed" />
  </ScrollArea>
</template>
