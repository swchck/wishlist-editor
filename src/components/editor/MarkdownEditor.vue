<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Crepe } from "@milkdown/crepe";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";

const props = defineProps<{ value: string; readonly?: boolean }>();
const emit = defineEmits<{ change: [markdown: string] }>();
const root = ref<HTMLDivElement>();
let crepe: Crepe | null = null;
let created = false;

/**
 * Crepe scopes its menu styles under `.milkdown`, and inside the editor column the menus
 * were clipped by the panel's overflow. They render into this full-window `.milkdown`
 * layer instead: transparent, click-through, only the menus themselves take clicks.
 */
function menuLayer(): HTMLElement {
  let layer = document.getElementById("md-menu-layer");
  if (!layer) {
    layer = document.createElement("div");
    layer.id = "md-menu-layer";
    layer.className = "milkdown";
    document.body.append(layer);
  }
  return layer;
}

onMounted(async () => {
  const editor = new Crepe({
    root: root.value!,
    defaultValue: props.value,
    // ImageBlock's default upload stores a session-only blob: URL in the markdown;
    // photos belong in the card's photo list, which uploads them into images/
    features: {
      [Crepe.Feature.Latex]: false,
      [Crepe.Feature.CodeMirror]: false,
      [Crepe.Feature.ImageBlock]: false,
      [Crepe.Feature.Table]: false,
    },
    featureConfigs: {
      [Crepe.Feature.BlockEdit]: {
        slashMenu: { root: menuLayer() },
        // only what a gift description needs; H1 would compete with the card title on the site
        textGroup: {
          label: "Текст",
          text: { label: "Обычный текст" },
          h1: null,
          h2: { label: "Заголовок" },
          h3: { label: "Подзаголовок" },
          h4: null,
          h5: null,
          h6: null,
          quote: { label: "Цитата" },
          divider: { label: "Разделитель" },
        },
        listGroup: {
          label: "Списки",
          bulletList: { label: "Маркированный список" },
          orderedList: { label: "Нумерованный список" },
          taskList: null,
        },
        advancedGroup: null,
      },
      [Crepe.Feature.Placeholder]: {
        text: "Пожелания по цвету, размеру, альтернативы… Наберите / для блоков",
        mode: "doc",
      },
    },
  });
  crepe = editor;
  editor.on((l) =>
    // Crepe re-serializes on load; only real edits may rewrite the body, or opening
    // a card would reformat its markdown on disk
    l.markdownUpdated((_ctx, md, prev) => {
      if (md !== prev) emit("change", md);
    }),
  );
  await editor.create();
  // switching cards quickly unmounts this before create() resolves
  if (crepe !== editor) {
    void editor.destroy();
    return;
  }
  created = true;
  editor.setReadonly(!!props.readonly);
  document.addEventListener("pointerdown", closeMenusOutside, true);
});

// Crepe closes its slash menu on Escape but not on a click elsewhere in the app
function closeMenusOutside(e: PointerEvent) {
  const target = e.target as Node;
  const menu = document.querySelector('.milkdown-slash-menu[data-show="true"]');
  if (!menu || menu.contains(target) || root.value?.contains(target)) return;
  root.value
    ?.querySelector(".ProseMirror")
    ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
}

watch(
  () => props.readonly,
  (ro) => created && crepe?.setReadonly(!!ro),
);

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", closeMenusOutside, true);
  // a half-created editor is destroyed by onMounted once create() resolves
  if (created) void crepe?.destroy();
  crepe = null;
});
</script>

<template>
  <div ref="root" class="md-editor" />
</template>

<style>
/* body raises specificity over Crepe's own `.milkdown` theme rule */
body .milkdown {
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
#md-menu-layer {
  position: fixed;
  inset: 0;
  z-index: 50;
  padding: 0;
  pointer-events: none;
}
#md-menu-layer > * {
  pointer-events: auto;
}
/* Crepe's menu is sized for a full-page editor; match the app's compact controls */
#md-menu-layer .milkdown-slash-menu {
  font-family: var(--font-sans);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 12px 32px -12px rgb(0 0 0 / 0.25);
}
#md-menu-layer .milkdown-slash-menu .tab-group ul {
  padding: 6px 8px;
  gap: 4px;
}
#md-menu-layer .milkdown-slash-menu .tab-group li {
  padding: 4px 10px;
  font-size: 13px;
}
#md-menu-layer .milkdown-slash-menu .menu-groups {
  max-height: 320px;
  padding: 4px 0;
}
#md-menu-layer .milkdown-slash-menu .menu-group h6 {
  padding: 8px 14px 4px;
  font-size: 11px;
  letter-spacing: 0.06em;
}
#md-menu-layer .milkdown-slash-menu .menu-group li {
  gap: 10px;
  padding: 6px 14px;
  font-size: 13.5px;
}
#md-menu-layer .milkdown-slash-menu .menu-group li svg {
  width: 18px;
  height: 18px;
  color: var(--muted-foreground);
}
.md-editor .milkdown .ProseMirror {
  padding: 8px 0 120px 48px;
  font-size: 15px;
  line-height: 1.65;
}
</style>
