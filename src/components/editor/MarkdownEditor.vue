<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Crepe } from "@milkdown/crepe";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";

const props = defineProps<{ value: string; readonly?: boolean }>();
const emit = defineEmits<{ change: [markdown: string] }>();
const root = ref<HTMLDivElement>();
let crepe: Crepe | null = null;

onMounted(async () => {
  crepe = new Crepe({
    root: root.value!,
    defaultValue: props.value,
    // ImageBlock's default upload stores a session-only blob: URL in the markdown;
    // photos go through the inspector until uploads into images/ exist
    features: { [Crepe.Feature.Latex]: false, [Crepe.Feature.CodeMirror]: false, [Crepe.Feature.ImageBlock]: false },
    featureConfigs: {
      [Crepe.Feature.Placeholder]: {
        text: "Пожелания по цвету, размеру, альтернативы… Наберите / для блоков",
        mode: "doc",
      },
    },
  });
  crepe.on((l) =>
    // Crepe re-serializes on load; only real edits may rewrite the body, or opening
    // a card would reformat its markdown on disk
    l.markdownUpdated((_ctx, md, prev) => {
      if (md !== prev) emit("change", md);
    }),
  );
  await crepe.create();
  crepe.setReadonly(!!props.readonly);
});

watch(
  () => props.readonly,
  (ro) => crepe?.setReadonly(!!ro),
);

onBeforeUnmount(() => {
  void crepe?.destroy();
  crepe = null;
});
</script>

<template>
  <div ref="root" class="md-editor" />
</template>

<style>
.md-editor .milkdown {
  --crepe-color-background: var(--background);
  --crepe-color-on-background: var(--foreground);
  --crepe-color-surface: var(--muted);
  --crepe-color-surface-low: var(--muted);
  --crepe-color-on-surface: var(--foreground);
  --crepe-color-on-surface-variant: var(--muted-foreground);
  --crepe-color-outline: var(--border);
  --crepe-color-primary: var(--foreground);
  --crepe-color-secondary: var(--muted);
  --crepe-color-on-secondary: var(--foreground);
  --crepe-color-inverse: var(--foreground);
  --crepe-color-on-inverse: var(--background);
  --crepe-color-inline-code: var(--foreground);
  --crepe-color-error: var(--destructive);
  --crepe-color-hover: var(--accent);
  --crepe-color-selected: var(--accent);
  --crepe-color-inline-area: var(--muted);
  --crepe-font-title: var(--font-sans);
  --crepe-font-default: var(--font-sans);
  --crepe-font-code: var(--font-mono);
  background: transparent;
}
.md-editor .milkdown .ProseMirror {
  padding: 8px 0 120px 48px;
  font-size: 15px;
  line-height: 1.65;
}
</style>
