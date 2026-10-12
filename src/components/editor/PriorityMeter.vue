<script setup lang="ts">
import { computed, ref } from "vue";
import { PRIORITY_LABELS } from "@/lib/wishlist/schema";

const model = defineModel<number>({ required: true });
const props = defineProps<{ disabled?: boolean }>();

// the site fills 6 − priority bars: priority 1 ("очень хочу") is a full meter
const levels = [5, 4, 3, 2, 1];
const hovered = ref<number | null>(null);
const shown = computed(() => hovered.value ?? model.value);

// rating-style keys: right/up wants it more (lower priority number), left/down less
function onKey(e: KeyboardEvent) {
  if (props.disabled) return;
  const step = ({ ArrowRight: -1, ArrowUp: -1, ArrowLeft: 1, ArrowDown: 1 } as Record<string, number>)[e.key];
  if (!step) return;
  e.preventDefault();
  model.value = Math.min(5, Math.max(1, model.value + step));
}
</script>

<template>
  <div
    class="focus-visible:ring-ring/50 flex h-9 items-center gap-3 rounded-md outline-none focus-visible:ring-3"
    role="radiogroup"
    aria-label="Насколько хочу"
    :tabindex="disabled ? -1 : 0"
    @keydown="onKey"
    @mouseleave="hovered = null"
  >
    <div class="flex flex-1 items-center gap-1">
      <button
        v-for="(p, i) in levels"
        :key="p"
        type="button"
        role="radio"
        tabindex="-1"
        :aria-checked="model === p"
        :aria-label="PRIORITY_LABELS[p]"
        :title="PRIORITY_LABELS[p]"
        :disabled="disabled"
        class="flex h-6 flex-1 cursor-pointer items-center disabled:cursor-default"
        @mouseenter="!disabled && (hovered = p)"
        @click="model = p"
      >
        <span
          class="h-2.5 w-full rounded-full transition-colors"
          :class="
            i < 6 - shown
              ? hovered !== null
                ? 'bg-foreground/60'
                : 'bg-foreground'
              : hovered !== null
                ? 'bg-muted-foreground/20'
                : 'bg-border'
          "
        />
      </button>
    </div>
    <span
      class="min-w-[7.5rem] text-xs whitespace-nowrap"
      :class="hovered !== null ? 'text-foreground font-medium' : 'text-muted-foreground'"
    >
      {{ PRIORITY_LABELS[shown] }}
    </span>
  </div>
</template>
