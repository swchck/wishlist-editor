<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { initials } from "@/lib/wishlist/format";
import { useWishlist } from "@/stores/wishlist";

const props = defineProps<{ src?: string; title: string }>();
const store = useWishlist();
const failed = ref(false);
const resolved = computed(() => store.photoSrc(props.src));
// local photos keep their ref while the object URL behind it is recreated on reload
watch(resolved, () => (failed.value = false));
const shown = computed(() => (failed.value ? undefined : resolved.value));
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
