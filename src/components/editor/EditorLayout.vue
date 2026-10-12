<script setup lang="ts">
import { computed, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CheckIcon, ChevronDownIcon, CircleAlertIcon, ExternalLinkIcon, Loader2Icon, UploadIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import ItemList from "./ItemList.vue";
import ItemEditor from "./ItemEditor.vue";
import ItemInspector from "./ItemInspector.vue";
import PublishSheet from "./PublishSheet.vue";
import DeployBadge from "./DeployBadge.vue";
import { useWishlist } from "@/stores/wishlist";
import { usePublish } from "@/stores/publish";
import { SCHEMA_VERSION } from "@/lib/wishlist/schema";

const store = useWishlist();
const publish = usePublish();
const publishing = ref(false);
const repoName = computed(() => (store.repo ? `${store.repo.owner}/${store.repo.name}` : ""));
const count = computed(() => store.changes.length || (store.ahead ? 1 : 0));
</script>

<template>
  <div class="flex h-full flex-col">
    <header
      data-tauri-drag-region
      class="bg-muted/40 flex h-11 shrink-0 items-center gap-3 border-b pr-3 pl-3 text-sm mac:pl-20"
    >
      <Button
        variant="outline"
        size="sm"
        class="h-7 font-mono text-xs"
        title="Открыть другой вишлист"
        @click="store.close()"
      >
        {{ repoName }} <ChevronDownIcon />
      </Button>
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
      <DeployBadge v-if="publish.deploy" :deploy="publish.deploy" />
      <div class="flex-1" data-tauri-drag-region />
      <Button v-if="store.siteUrl" variant="ghost" size="sm" @click="openUrl(store.siteUrl)">
        <ExternalLinkIcon /> Сайт
      </Button>
      <Button size="sm" :disabled="!store.pending || store.readOnly" @click="publishing = true">
        <UploadIcon /> Опубликовать
        <span
          v-if="count"
          class="bg-background text-foreground grid h-4.5 min-w-4.5 place-items-center rounded-full px-1 text-[11px] font-semibold"
          >{{ count }}</span
        >
      </Button>
    </header>
    <Alert v-if="store.readOnly" class="rounded-none border-x-0 border-t-0">
      <AlertDescription>
        Вишлист в формате v{{ store.config?.schemaVersion }}, а редактор знает v{{ SCHEMA_VERSION }}. Открыт только для
        чтения — обновите приложение.
      </AlertDescription>
    </Alert>
    <Alert v-else-if="store.pull === null" class="rounded-none border-x-0 border-t-0">
      <AlertDescription>Нет связи с GitHub — правьте как обычно, опубликуете, когда сеть вернётся.</AlertDescription>
    </Alert>
    <Alert v-else-if="store.pull === 'blocked'" class="rounded-none border-x-0 border-t-0">
      <AlertDescription>
        На GitHub изменили те же карточки, что и здесь. При публикации покажем обе версии и попросим выбрать.
      </AlertDescription>
    </Alert>
    <Alert v-else-if="store.ahead" class="rounded-none border-x-0 border-t-0">
      <AlertDescription
        >Есть правки, которые не дошли до GitHub в прошлый раз. Нажмите «Опубликовать».</AlertDescription
      >
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
    <PublishSheet v-model:open="publishing" />
  </div>
</template>
