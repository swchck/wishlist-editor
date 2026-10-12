<script setup lang="ts">
import { computed } from "vue";
import { CheckIcon, CircleAlertIcon, FolderOpenIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import ItemList from "./ItemList.vue";
import ItemEditor from "./ItemEditor.vue";
import ItemInspector from "./ItemInspector.vue";
import { useWishlist } from "@/stores/wishlist";
import { SCHEMA_VERSION } from "@/lib/wishlist/schema";

const store = useWishlist();
const folder = computed(() => store.root?.split(/[\\/]/).pop() ?? "");
</script>

<template>
  <div class="flex h-full flex-col">
    <header
      data-tauri-drag-region
      class="bg-muted/40 flex h-11 shrink-0 items-center gap-3 border-b pr-3 pl-3 text-sm mac:pl-20"
    >
      <span class="bg-background rounded-md border px-2.5 py-0.5 font-mono text-xs" :title="store.root ?? ''">{{
        folder
      }}</span>
      <span class="text-muted-foreground flex items-center gap-1.5 text-xs">
        <template v-if="store.saveState === 'saving' || store.saveState === 'pending'">
          <Loader2Icon class="size-3.5 animate-spin" /> Сохраняю…
        </template>
        <template v-else-if="store.saveState === 'error'">
          <CircleAlertIcon class="text-destructive size-3.5" />
          <span class="text-destructive">{{ store.saveError }}</span>
        </template>
        <template v-else><CheckIcon class="size-3.5" /> Сохранено</template>
      </span>
      <div class="flex-1" data-tauri-drag-region />
      <Button variant="ghost" size="sm" @click="store.close()"><FolderOpenIcon /> Другая папка</Button>
    </header>
    <Alert v-if="store.readOnly" class="rounded-none border-x-0 border-t-0">
      <AlertDescription>
        Вишлист в формате v{{ store.config?.schemaVersion }}, а редактор знает v{{ SCHEMA_VERSION }}. Открыт только для
        чтения — обновите приложение.
      </AlertDescription>
    </Alert>
    <div class="grid min-h-0 flex-1 grid-cols-[272px_minmax(0,1fr)_300px]">
      <ItemList />
      <ItemEditor v-if="store.current" :key="store.current.slug" :card="store.current" />
      <div v-else class="text-muted-foreground grid place-items-center text-sm">
        Карточек пока нет — создайте первую
      </div>
      <ItemInspector v-if="store.current" :card="store.current" />
      <div v-else class="bg-muted/30 border-l" />
    </div>
  </div>
</template>
