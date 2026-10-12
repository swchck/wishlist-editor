<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import { toast } from "vue-sonner";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { errorMessage } from "@/lib/api";
import { useWishlist } from "@/stores/wishlist";

const FIRST_CHECK_MS = 5_000;
const RECHECK_MS = 6 * 60 * 60 * 1000;

const store = useWishlist();
const update = shallowRef<Update | null>(null);
const open = ref(false);
const installing = ref(false);
const total = ref(0);
const done = ref(0);
const percent = computed(() => (total.value ? Math.min(100, Math.round((done.value / total.value) * 100)) : null));

let timer: ReturnType<typeof setTimeout> | undefined;

async function lookForUpdate() {
  timer = setTimeout(() => void lookForUpdate(), RECHECK_MS);
  if (update.value || installing.value) return;
  try {
    update.value = await check();
  } catch {
    // offline or GitHub hiccup: the next check comes in a few hours, nothing to tell the user
    return;
  }
  if (!update.value) return;
  toast(`Вышла версия ${update.value.version}`, {
    duration: Infinity,
    action: { label: "Подробнее", onClick: () => (open.value = true) },
  });
}

async function install() {
  const u = update.value;
  if (!u) return;
  if (store.locked) {
    toast.error("Дождитесь конца публикации", { description: "Обновление перезапустит приложение." });
    return;
  }
  await store.flushAll();
  if (store.saveState === "error") {
    toast.error("Последние правки не сохранились", { description: store.saveError ?? undefined });
    return;
  }
  installing.value = true;
  try {
    await u.downloadAndInstall((e) => {
      if (e.event === "Started") total.value = e.data.contentLength ?? 0;
      else if (e.event === "Progress") done.value += e.data.chunkLength;
    });
    // on Windows the installer has already closed the app by now
    await relaunch();
  } catch (e) {
    installing.value = false;
    done.value = 0;
    toast.error("Не удалось обновиться", { description: errorMessage(e) });
  }
}

onMounted(() => {
  if (!("__TAURI_INTERNALS__" in window) || import.meta.env.DEV) return;
  clearTimeout(timer);
  timer = setTimeout(() => void lookForUpdate(), FIRST_CHECK_MS);
});
onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      v-if="update"
      class="sm:max-w-md"
      @interact-outside="installing && $event.preventDefault()"
      @escape-key-down="installing && $event.preventDefault()"
    >
      <DialogHeader>
        <DialogTitle>Wishlist Editor {{ update.version }}</DialogTitle>
        <DialogDescription>Сейчас установлена {{ update.currentVersion }}.</DialogDescription>
      </DialogHeader>
      <div
        v-if="update.body"
        class="bg-muted/50 max-h-60 overflow-auto rounded-md p-3 text-sm leading-relaxed whitespace-pre-wrap"
      >
        {{ update.body }}
      </div>
      <div v-if="installing" class="grid gap-1.5">
        <div class="bg-muted h-1.5 overflow-hidden rounded-full">
          <div
            class="bg-primary h-full transition-[width]"
            :class="{ 'w-1/3 animate-pulse': percent === null }"
            :style="percent === null ? undefined : { width: `${percent}%` }"
          />
        </div>
        <div class="text-muted-foreground text-xs">
          {{ percent === null || percent < 100 ? "Скачиваю…" : "Устанавливаю и перезапускаю…" }}
        </div>
      </div>
      <DialogFooter>
        <Button variant="ghost" :disabled="installing" @click="open = false">Позже</Button>
        <Button :disabled="installing" @click="install">
          <Loader2Icon v-if="installing" class="animate-spin" /> Обновить и перезапустить
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
