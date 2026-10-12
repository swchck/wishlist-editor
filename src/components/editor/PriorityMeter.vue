<script setup lang="ts">
import { PRIORITY_LABELS } from "@/lib/wishlist/schema";

const model = defineModel<number>({ required: true });
defineProps<{ disabled?: boolean }>();
// the site fills 6 − priority bars: priority 1 ("очень хочу") is a full meter
const levels = [5, 4, 3, 2, 1];
</script>

<template>
  <div class="flex h-9 items-center gap-1" role="radiogroup" aria-label="Насколько хочу">
    <button
      v-for="(p, i) in levels"
      :key="p"
      type="button"
      role="radio"
      :aria-checked="model === p"
      :aria-label="PRIORITY_LABELS[p]"
      :title="PRIORITY_LABELS[p]"
      :disabled="disabled"
      class="focus-visible:ring-ring/50 h-2 flex-1 rounded-sm outline-none focus-visible:ring-3"
      :class="i < 6 - model ? 'bg-foreground' : 'bg-border hover:bg-muted-foreground/40'"
      @click="model = p"
    />
    <span class="text-muted-foreground ml-2 text-xs whitespace-nowrap">{{ PRIORITY_LABELS[model] }}</span>
  </div>
</template>
