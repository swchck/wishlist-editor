<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, unref, watch, type MaybeRef } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  CheckIcon,
  ChevronDownIcon,
  CircleAlertIcon,
  ExternalLinkIcon,
  Loader2Icon,
  ListIcon,
  PanelLeftIcon,
  PlusIcon,
  PanelRightIcon,
  UploadIcon,
  ImagePlusIcon,
} from "@lucide/vue";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { toast } from "vue-sonner";
import { api, errorMessage, type RemoteWishlist } from "@/lib/api";
import { useCreate } from "@/stores/create";
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
import { picturesOf, uploadPhotos } from "./upload";

const store = useWishlist();
const publish = usePublish();
const publishing = ref(false);
const repoName = computed(() => (store.repo ? `${store.repo.owner}/${store.repo.name}` : ""));
const count = computed(() => store.changes.length || (store.ahead ? 1 : 0));

const create = useCreate();
const others = ref<RemoteWishlist[] | null>(null);
const switcherOpen = ref(false);
// a bare @update:open listener made reka treat the menu as controlled and snap it shut
watch(switcherOpen, async (open) => {
  if (open) others.value = await api.listWishlists().catch(() => []);
});
async function switchTo(w: RemoteWishlist) {
  if (store.repo && w.owner === store.repo.owner && w.name === store.repo.name) return;
  try {
    await store.open(w);
  } catch (e) {
    toast.error("Не удалось открыть вишлист", { description: errorMessage(e) });
  }
}
async function newWishlist() {
  create.wizardOpen = true;
  await store.close();
}

interface PanelApi {
  collapse: () => void;
  expand: () => void;
  // reka exposes it as a computed ref, unwrapped only in templates
  isCollapsed: MaybeRef<boolean>;
}
const left = ref<PanelApi | null>(null);
const right = ref<PanelApi | null>(null);
const leftCollapsed = ref(false);
const rightCollapsed = ref(false);

function toggle(panel: PanelApi | null) {
  if (!panel) return;
  if (unref(panel.isCollapsed)) panel.expand();
  else panel.collapse();
}

// photos dropped anywhere in the editor go to the open card; dragenter/leave fire for every
// child element, so count them instead of trusting the last event
const dragDepth = ref(0);
const canDrop = computed(() => !!store.current?.file && !store.frozen);
const isFileDrag = (e: DragEvent) => !!e.dataTransfer?.types.includes("Files");
function onDragEnter(e: DragEvent) {
  if (isFileDrag(e)) dragDepth.value++;
}
function onDragLeave(e: DragEvent) {
  if (isFileDrag(e)) dragDepth.value = Math.max(0, dragDepth.value - 1);
}
// a drop into a portalled dialog or a cancelled drag never reaches this subtree's @drop
const endDrag = () => (dragDepth.value = 0);
function onDrop(e: DragEvent) {
  dragDepth.value = 0;
  if (!canDrop.value || !store.current) return;
  const pictures = picturesOf(e.dataTransfer?.files);
  if (!pictures.length) {
    toast.error("Это не картинка", { description: "Подойдут JPEG, PNG, WebP или GIF" });
    return;
  }
  void uploadPhotos(store.current.slug, pictures);
}

// inside text fields a paste is text; the photo URL field handles its own picture pastes
function onPaste(e: ClipboardEvent) {
  const t = e.target as HTMLElement | null;
  if (t?.closest("input, textarea, [contenteditable='true']") || !canDrop.value || !store.current) return;
  const pictures = picturesOf(e.clipboardData?.items);
  if (!pictures.length) return;
  e.preventDefault();
  void uploadPhotos(store.current.slug, pictures);
}

// ⌘\ — card list, ⌥⌘\ — preview; ⌘B would have been the usual choice, but it is bold in the body editor
function onKey(e: KeyboardEvent) {
  if (!(e.metaKey || e.ctrlKey) || e.code !== "Backslash") return;
  e.preventDefault();
  toggle(e.altKey ? right.value : left.value);
}
onMounted(() => {
  window.addEventListener("keydown", onKey);
  window.addEventListener("paste", onPaste);
  window.addEventListener("drop", endDrag);
  window.addEventListener("dragend", endDrag);
  // sizes, collapsed ones included, come back from the saved layout
  leftCollapsed.value = !!unref(left.value?.isCollapsed);
  rightCollapsed.value = !!unref(right.value?.isCollapsed);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  window.removeEventListener("paste", onPaste);
  window.removeEventListener("drop", endDrag);
  window.removeEventListener("dragend", endDrag);
});
</script>

<template>
  <div
    class="relative flex h-full flex-col"
    @dragenter="onDragEnter"
    @dragleave="onDragLeave"
    @dragover.prevent
    @drop.prevent="onDrop"
  >
    <div
      v-if="dragDepth > 0"
      class="bg-background/80 pointer-events-none absolute inset-0 z-50 grid place-items-center p-6 backdrop-blur-sm"
    >
      <div
        class="grid w-full max-w-md place-items-center gap-2 rounded-2xl border-2 border-dashed p-10 text-center"
        :class="canDrop ? 'border-primary' : 'border-muted-foreground/40'"
      >
        <ImagePlusIcon class="text-muted-foreground size-8" />
        <template v-if="canDrop">
          <div class="font-medium">Добавить фото в «{{ store.current?.data.title || store.current?.slug }}»</div>
          <div class="text-muted-foreground text-xs">Уменьшим до 1600 px и уберём геометку из файла</div>
        </template>
        <div v-else class="text-muted-foreground text-sm">Сначала откройте карточку</div>
      </div>
    </div>
    <header
      data-tauri-drag-region
      class="bg-muted/40 flex h-11 shrink-0 items-center gap-3 border-b pr-3 pl-3 text-sm mac:pl-20"
    >
      <Button
        variant="ghost"
        size="icon-sm"
        :aria-label="leftCollapsed ? 'Показать список карточек' : 'Скрыть список карточек'"
        :title="`${leftCollapsed ? 'Показать' : 'Скрыть'} список карточек (⌘\\)`"
        :aria-pressed="!leftCollapsed"
        @click="toggle(left)"
      >
        <PanelLeftIcon />
      </Button>
      <DropdownMenu v-model:open="switcherOpen">
        <DropdownMenuTrigger as-child>
          <Button variant="outline" size="sm" class="h-7 font-mono text-xs" :disabled="store.locked">
            {{ repoName }} <ChevronDownIcon />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" class="w-72">
          <DropdownMenuLabel class="text-muted-foreground text-xs font-normal">Ваши вишлисты</DropdownMenuLabel>
          <DropdownMenuItem v-if="others === null" disabled>
            <Loader2Icon class="animate-spin" /> Загружаю…
          </DropdownMenuItem>
          <DropdownMenuItem v-for="w in others ?? []" :key="`${w.owner}/${w.name}`" @select="switchTo(w)">
            <CheckIcon :class="`${w.owner}/${w.name}` === repoName ? 'opacity-100' : 'opacity-0'" />
            <span class="min-w-0 flex-1">
              <span class="block truncate">{{ w.title ?? w.name }}</span>
              <span class="text-muted-foreground block truncate font-mono text-[11px]">{{ w.owner }}/{{ w.name }}</span>
            </span>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem @select="newWishlist"><PlusIcon /> Создать новый вишлист…</DropdownMenuItem>
          <DropdownMenuItem @select="store.close()"><ListIcon /> Все вишлисты</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
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
      <Button
        variant="ghost"
        size="icon-sm"
        :aria-label="rightCollapsed ? 'Показать превью' : 'Скрыть превью'"
        :title="`${rightCollapsed ? 'Показать' : 'Скрыть'} превью и проверку (⌥⌘\\)`"
        :aria-pressed="!rightCollapsed"
        @click="toggle(right)"
      >
        <PanelRightIcon />
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
    <ResizablePanelGroup direction="horizontal" auto-save-id="editor-columns" class="min-h-0 flex-1">
      <ResizablePanel
        ref="left"
        size-unit="px"
        :default-size="272"
        :min-size="200"
        :max-size="440"
        collapsible
        :collapsed-size="0"
        class="flex min-h-0 flex-col"
        @collapse="leftCollapsed = true"
        @expand="leftCollapsed = false"
      >
        <ItemList />
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel :min-size="30" class="@container flex min-h-0 flex-col">
        <!-- Crepe reads its text only on mount, so a reload from disk must remount it -->
        <ItemEditor v-if="store.current" :key="`${store.current.slug}:${store.generation}`" :card="store.current" />
        <div v-else class="text-muted-foreground grid h-full place-items-center text-sm">
          Карточек пока нет — создайте первую
        </div>
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel
        ref="right"
        size-unit="px"
        :default-size="300"
        :min-size="240"
        :max-size="520"
        collapsible
        :collapsed-size="0"
        class="flex min-h-0 flex-col"
        @collapse="rightCollapsed = true"
        @expand="rightCollapsed = false"
      >
        <ItemInspector v-if="store.current" :card="store.current" />
        <div v-else class="bg-muted/30 h-full" />
      </ResizablePanel>
    </ResizablePanelGroup>
    <PublishSheet v-model:open="publishing" />
  </div>
</template>
