<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { CircleAlertIcon, Loader2Icon, Undo2Icon } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import ConflictDialog from "./ConflictDialog.vue";
import { errorMessage } from "@/lib/api";
import { plural } from "@/lib/wishlist/changes";
import { usePublish } from "@/stores/publish";
import { useWishlist } from "@/stores/wishlist";

const open = defineModel<boolean>("open", { required: true });
const publish = usePublish();
const store = useWishlist();
const message = ref("");
const edited = ref(false);

watch(open, (v) => {
  if (!v) return;
  publish.reset();
  edited.value = false;
  message.value = publish.suggestedMessage;
});
// keep the suggestion in sync with discards until the user writes their own
watch(
  () => publish.suggestedMessage,
  (m) => {
    if (!edited.value) message.value = m;
  },
);

const MARKS = { added: "A", modified: "M", deleted: "D" } as const;
const MARK_CLASS = {
  added: "text-blue-600 dark:text-blue-400",
  modified: "text-amber-600 dark:text-amber-400",
  deleted: "text-destructive",
} as const;

const title = computed(() => {
  const n = publish.described.length;
  if (!n) return "Опубликовать прошлые правки";
  return `Опубликовать ${n} ${plural(n, "изменение", "изменения", "изменений")}`;
});

async function discard(path: string) {
  try {
    await store.discard(path);
  } catch (e) {
    toast.error("Не удалось отменить", { description: errorMessage(e) });
  }
}

function submit() {
  void publish.run(message.value.trim());
}

// publishing also finishes from the conflict dialog, so react to the outcome, not the click
watch(
  () => publish.phase,
  (phase) => {
    // the conflict dialog replaces the sheet; stacking two modals blurs the dialog behind the overlay
    if (phase === "conflicts") open.value = false;
    if (phase !== "done") return;
    open.value = false;
    publish.reset();
    toast.success("Опубликовано", { description: "Сайт обновится примерно через минуту." });
  },
);
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent class="flex w-[400px] flex-col gap-0 sm:max-w-[400px]">
      <SheetHeader>
        <SheetTitle>{{ title }}</SheetTitle>
        <SheetDescription>Изменения уйдут на GitHub, и сайт пересоберётся.</SheetDescription>
      </SheetHeader>
      <div class="grid min-h-0 flex-1 content-start gap-4 overflow-auto px-4">
        <div v-if="publish.described.length" class="divide-y rounded-lg border text-sm">
          <div v-for="c in publish.described" :key="c.path" class="flex items-center gap-2.5 px-3 py-2">
            <span class="w-3 font-mono text-[11px] font-semibold" :class="MARK_CLASS[c.kind]">{{ MARKS[c.kind] }}</span>
            <span class="min-w-0 flex-1 truncate" :title="c.path">{{ c.label }}</span>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="`Отменить изменение ${c.label}`"
              title="Вернуть как на GitHub"
              :disabled="publish.phase === 'publishing'"
              @click="discard(c.path)"
            >
              <Undo2Icon />
            </Button>
          </div>
        </div>

        <div
          v-if="publish.blockers.length"
          class="border-destructive/40 bg-destructive/5 text-destructive grid gap-1 rounded-lg border p-3 text-sm"
        >
          <div class="flex items-center gap-2 font-medium"><CircleAlertIcon class="size-4" /> Сначала исправьте</div>
          <button
            v-for="b in publish.blockers"
            :key="b.slug"
            class="text-left underline-offset-2 hover:underline"
            @click="
              store.selected = b.slug;
              open = false;
            "
          >
            {{ b.label }}
          </button>
          <p class="text-xs opacity-80">С ошибками в карточке сайт не соберётся.</p>
        </div>

        <div class="grid gap-1.5">
          <Label for="publish-message" class="text-muted-foreground text-xs">Описание изменений</Label>
          <Textarea
            id="publish-message"
            v-model="message"
            rows="3"
            class="resize-none text-sm"
            @input="edited = true"
          />
        </div>

        <div
          v-if="publish.phase === 'error'"
          class="border-destructive/40 bg-destructive/5 text-destructive rounded-lg border p-3 text-sm"
        >
          {{ publish.error }}
        </div>
      </div>
      <SheetFooter>
        <Button
          size="lg"
          :disabled="publish.phase === 'publishing' || publish.blockers.length > 0 || !store.pending"
          @click="submit"
        >
          <Loader2Icon v-if="publish.phase === 'publishing'" class="animate-spin" />
          {{ publish.phase === "publishing" ? "Публикую…" : "Опубликовать" }}
        </Button>
      </SheetFooter>
    </SheetContent>
  </Sheet>
  <ConflictDialog :message="message" />
</template>
