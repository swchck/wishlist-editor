<script setup lang="ts">
import { ref } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { errorMessage } from "@/lib/api";
import { useWishlist } from "@/stores/wishlist";

const store = useWishlist();
const busy = ref(false);
const error = ref<string | null>(null);

async function openPath(path: string) {
  busy.value = true;
  error.value = null;
  try {
    await store.open(path);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}

async function pick() {
  const path = await openDialog({ directory: true, title: "Папка с вишлистом" });
  if (typeof path === "string") await openPath(path);
}

function shortPath(p: string) {
  return p.replace(/^\/Users\/[^/]+/, "~");
}
</script>

<template>
  <div class="grid h-full grid-cols-1 md:grid-cols-2">
    <div data-tauri-drag-region class="bg-muted/40 flex flex-col justify-center gap-5 border-r p-12">
      <div class="text-muted-foreground flex items-center gap-2 text-sm font-medium">
        <span class="text-foreground text-lg">✳</span> Wishlist Editor
      </div>
      <h1 class="text-3xl leading-tight font-bold tracking-tight text-balance">Откройте свой вишлист</h1>
      <p class="text-muted-foreground max-w-sm">
        Выберите папку репозитория — ту, где лежит <code class="font-mono text-xs">wishlist.config.json</code>. Правки
        сохраняются в файлы сразу.
      </p>
    </div>
    <div class="flex flex-col justify-center gap-4 p-12">
      <Button size="lg" class="w-fit" :disabled="busy" @click="pick">
        <Loader2Icon v-if="busy" class="animate-spin" />
        <FolderOpenIcon v-else />
        Выбрать папку
      </Button>
      <Alert v-if="error" variant="destructive">
        <AlertDescription>{{ error }}</AlertDescription>
      </Alert>
      <div v-if="store.recent.length" class="grid gap-2">
        <div class="text-muted-foreground text-xs font-semibold tracking-wider uppercase">Недавние</div>
        <div class="divide-y rounded-lg border">
          <button
            v-for="p in store.recent"
            :key="p"
            class="hover:bg-muted/60 flex w-full items-center px-3 py-2 text-left font-mono text-xs"
            :disabled="busy"
            @click="openPath(p)"
          >
            {{ shortPath(p) }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
