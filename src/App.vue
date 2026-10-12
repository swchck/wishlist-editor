<script setup lang="ts">
import { onMounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Toaster } from "@/components/ui/sonner";
import "vue-sonner/style.css";
import WelcomeScreen from "@/components/WelcomeScreen.vue";
import EditorLayout from "@/components/editor/EditorLayout.vue";
import { useWishlist } from "@/stores/wishlist";

const store = useWishlist();

onMounted(() => {
  // closing the window must not drop the last 600 ms of typing
  void getCurrentWindow().onCloseRequested(async () => {
    await store.flushAll();
  });
});
</script>

<template>
  <EditorLayout v-if="store.root" />
  <WelcomeScreen v-else />
  <Toaster position="bottom-right" rich-colors />
</template>
