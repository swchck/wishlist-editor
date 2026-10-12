<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { initials } from "@/lib/wishlist/format";
import { useWishlist } from "@/stores/wishlist";

const props = defineProps<{ src?: string; title: string }>();
const store = useWishlist();
const failed = ref(false);
watch(
  () => props.src,
  () => (failed.value = false),
);
const shown = computed(() => (failed.value ? undefined : store.photoSrc(props.src)));
</script>

<template>
  <span
    class="bg-muted text-muted-foreground grid shrink-0 place-items-center overflow-hidden rounded-md border text-[11px] font-semibold"
  >
    <img
      v-if="shown"
      :src="shown"
      alt=""
      class="size-full object-cover"
      referrerpolicy="no-referrer"
      @error="failed = true"
    />
    <template v-else>{{ initials(title) }}</template>
  </span>
</template>
