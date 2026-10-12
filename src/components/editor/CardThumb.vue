<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { initials } from "@/lib/wishlist/format";

const props = defineProps<{ src?: string; title: string }>();
const failed = ref(false);
watch(
  () => props.src,
  () => (failed.value = false),
);
// local ./images/… files need the asset protocol, which arrives with image uploads
const remote = computed(() => (props.src && /^https?:\/\//.test(props.src) && !failed.value ? props.src : null));
</script>

<template>
  <span
    class="bg-muted text-muted-foreground grid shrink-0 place-items-center overflow-hidden rounded-md border text-[11px] font-semibold"
  >
    <img
      v-if="remote"
      :src="remote"
      alt=""
      class="size-full object-cover"
      referrerpolicy="no-referrer"
      @error="failed = true"
    />
    <template v-else>{{ initials(title) }}</template>
  </span>
</template>
