<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { Side } from "@/lib/api";
import { describeChanges, plural } from "@/lib/wishlist/changes";
import { usePublish } from "@/stores/publish";
import { useWishlist } from "@/stores/wishlist";

const props = defineProps<{ message: string }>();
const publish = usePublish();
const store = useWishlist();
const choices = reactive<Record<string, Side>>({});

const open = computed({
  get: () => publish.phase === "conflicts",
  set: (v) => {
    if (!v) publish.reset();
  },
});

watch(
  () => publish.conflicts,
  () => {
    for (const k of Object.keys(choices)) delete choices[k];
  },
);

const files = computed(() =>
  describeChanges(
    publish.conflicts.map((c) => ({ path: c.path, kind: "modified" as const })),
    (slug) => {
      const t = store.cards.get(slug)?.data.title;
      return typeof t === "string" ? t : undefined;
    },
  ).map((d, i) => {
    const c = publish.conflicts[i];
    return { ...d, local: markLines(c.local, c.remote), remote: markLines(c.remote, c.local) };
  }),
);

// a whole-line marker is enough for cards: they are short and differ in a field or two
function markLines(text: string | null, other: string | null) {
  if (text === null) return null;
  const theirs = new Set((other ?? "").split("\n"));
  return text.split("\n").map((line) => ({ line, changed: !theirs.has(line) }));
}
const ready = computed(() => publish.conflicts.every((c) => choices[c.path]));

function finish() {
  void publish.run(props.message, { ...choices });
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[85vh] overflow-auto sm:max-w-3xl">
      <DialogHeader>
        <DialogTitle>
          {{ files.length }} {{ plural(files.length, "карточку", "карточки", "карточек") }} изменили и здесь, и на
          GitHub
        </DialogTitle>
        <DialogDescription>Выберите, какую версию оставить. Вторая пропадёт.</DialogDescription>
      </DialogHeader>
      <section v-for="f in files" :key="f.path" class="grid gap-2">
        <div class="flex items-center gap-3">
          <b class="min-w-0 flex-1 truncate text-sm">{{ f.label }}</b>
          <ToggleGroup v-model="choices[f.path]" type="single" variant="outline">
            <ToggleGroupItem
              value="local"
              class="data-[state=on]:bg-foreground data-[state=on]:text-background px-3 text-xs"
            >
              Оставить мою
            </ToggleGroupItem>
            <ToggleGroupItem
              value="remote"
              class="data-[state=on]:bg-foreground data-[state=on]:text-background px-3 text-xs"
            >
              Взять с GitHub
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
        <div class="grid grid-cols-2 gap-2">
          <div
            v-for="side in ['local', 'remote'] as const"
            :key="side"
            class="overflow-hidden rounded-lg border"
            :class="{ 'ring-foreground ring-2': choices[f.path] === side }"
          >
            <header class="bg-muted border-b px-3 py-1.5 text-xs font-semibold">
              {{ side === "local" ? "На этом компьютере" : "На GitHub" }}
            </header>
            <pre
              class="max-h-64 overflow-auto py-2 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap"
            ><template v-if="f[side]"><span
              v-for="(l, i) in f[side]"
              :key="i"
              class="block px-3"
              :class="l.changed ? 'bg-amber-500/15 text-foreground' : 'text-muted-foreground'"
            >{{ l.line || " " }}</span></template><span v-else class="text-muted-foreground block px-3 italic">Файл удалён</span></pre>
          </div>
        </div>
      </section>
      <DialogFooter>
        <Button variant="ghost" @click="open = false">Позже</Button>
        <Button :disabled="!ready || publish.phase === 'publishing'" @click="finish">Опубликовать</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
