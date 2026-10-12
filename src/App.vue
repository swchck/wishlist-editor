<script setup lang="ts">
import { onMounted } from "vue";
import { toast } from "vue-sonner";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Toaster } from "@/components/ui/sonner";
import "vue-sonner/style.css";
import WelcomeScreen from "@/components/WelcomeScreen.vue";
import EditorLayout from "@/components/editor/EditorLayout.vue";
import { useWishlist } from "@/stores/wishlist";

const store = useWishlist();

onMounted(() => {
  // closing the window must not drop the last 600 ms of typing
  let warned = false;
  void getCurrentWindow().onCloseRequested(async (event) => {
    await store.flushAll();
    // a failed final save would otherwise vanish with the window; a second close means "quit anyway"
    if (store.saveState === "error" && !warned) {
      warned = true;
      event.preventDefault();
      toast.error("Последние правки не сохранились", {
        description: `${store.saveError ?? ""} Закройте окно ещё раз, чтобы выйти без сохранения.`,
        duration: 10000,
      });
    }
  });
});
</script>

<template>
  <EditorLayout v-if="store.root" />
  <WelcomeScreen v-else />
  <Toaster position="bottom-right" rich-colors />
</template>
