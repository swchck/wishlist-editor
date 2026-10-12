<script setup lang="ts">
import { onMounted } from "vue";
import { toast } from "vue-sonner";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Loader2Icon } from "@lucide/vue";
import { Toaster } from "@/components/ui/sonner";
import "vue-sonner/style.css";
import SignInScreen from "@/components/SignInScreen.vue";
import RepoPicker from "@/components/RepoPicker.vue";
import EditorLayout from "@/components/editor/EditorLayout.vue";
import { useSession } from "@/stores/session";
import { useWishlist } from "@/stores/wishlist";

const session = useSession();
const store = useWishlist();

onMounted(() => {
  void session.check();
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
  <div v-if="session.phase === 'checking'" class="grid h-full place-items-center" data-tauri-drag-region>
    <Loader2Icon class="text-muted-foreground size-5 animate-spin" />
  </div>
  <EditorLayout v-else-if="session.phase === 'signed-in' && store.repo" />
  <RepoPicker v-else-if="session.phase === 'signed-in'" />
  <SignInScreen v-else />
  <Toaster position="bottom-right" rich-colors />
</template>
