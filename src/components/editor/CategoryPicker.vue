<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { CheckIcon, ChevronsUpDownIcon, PlusIcon, XIcon } from "@lucide/vue";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

const props = defineProps<{
  modelValue: string;
  /** Categories already used in this wishlist with their card counts, most used first. */
  options: { name: string; count: number }[];
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const open = ref(false);
const query = ref("");
const active = ref(0);
const input = ref<HTMLInputElement>();

const q = computed(() => query.value.trim());
const matches = computed(() => props.options.filter((o) => o.name.toLowerCase().includes(q.value.toLowerCase())));
const canCreate = computed(
  () => q.value !== "" && !props.options.some((o) => o.name.toLowerCase() === q.value.toLowerCase()),
);
// flat list the arrow keys walk: create first when offered, then matches, then "none"
const rows = computed(() => [
  ...(canCreate.value ? [{ kind: "create" as const, name: q.value }] : []),
  ...matches.value.map((o) => ({ kind: "pick" as const, name: o.name, count: o.count })),
  ...(props.modelValue ? [{ kind: "clear" as const, name: "" }] : []),
]);

watch(open, async (v) => {
  if (!v) return;
  query.value = "";
  active.value = 0;
  await nextTick();
  input.value?.focus();
});
watch(q, () => (active.value = 0));

function choose(name: string) {
  emit("update:modelValue", name);
  open.value = false;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const n = rows.value.length;
    if (n) active.value = (active.value + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
  } else if (e.key === "Enter") {
    e.preventDefault();
    const row = rows.value[active.value];
    if (row) choose(row.name);
  }
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <button
        id="item-category"
        type="button"
        :disabled="disabled"
        class="border-input focus-visible:border-ring focus-visible:ring-ring/50 flex h-9 w-full items-center gap-2 rounded-md border bg-transparent px-3 text-left text-sm outline-none focus-visible:ring-3 disabled:opacity-50"
      >
        <span class="min-w-0 flex-1 truncate" :class="{ 'text-muted-foreground': !modelValue }">
          {{ modelValue || "Без категории" }}
        </span>
        <ChevronsUpDownIcon class="text-muted-foreground size-4 shrink-0" />
      </button>
    </PopoverTrigger>
    <PopoverContent align="start" class="w-(--reka-popover-trigger-width) min-w-56 p-0">
      <input
        ref="input"
        v-model="query"
        placeholder="Найти или создать категорию…"
        class="placeholder:text-muted-foreground w-full border-b bg-transparent px-3 py-2.5 text-sm outline-none"
        @keydown="onKey"
      />
      <ul class="max-h-64 overflow-auto p-1 text-sm" role="listbox">
        <li
          v-for="(row, i) in rows"
          :key="`${row.kind}:${row.name}`"
          role="option"
          :aria-selected="i === active"
          class="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5"
          :class="i === active ? 'bg-accent' : ''"
          @mouseenter="active = i"
          @click="choose(row.name)"
        >
          <template v-if="row.kind === 'create'"> <PlusIcon class="size-4" /> Создать «{{ row.name }}» </template>
          <template v-else-if="row.kind === 'clear'">
            <XIcon class="text-muted-foreground size-4" /> <span class="text-muted-foreground">Без категории</span>
          </template>
          <template v-else>
            <CheckIcon class="size-4" :class="row.name === modelValue ? 'opacity-100' : 'opacity-0'" />
            <span class="min-w-0 flex-1 truncate">{{ row.name }}</span>
            <span class="text-muted-foreground text-xs tabular-nums">{{ row.count }}</span>
          </template>
        </li>
        <li v-if="!rows.length" class="text-muted-foreground px-2 py-3 text-center text-xs">
          Начните вводить, чтобы создать первую категорию
        </li>
      </ul>
    </PopoverContent>
  </Popover>
</template>
