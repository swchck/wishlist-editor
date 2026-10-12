<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { errorMessage } from "@/lib/api";
import { SLUG_RE, slugify } from "@/lib/wishlist/slug";
import { useWishlist, type Card } from "@/stores/wishlist";

const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ card: Card }>();
const emit = defineEmits<{ renamed: [slug: string] }>();
const store = useWishlist();
const slug = ref("");
const error = ref<string | null>(null);
const busy = ref(false);

watch(open, (v) => {
  if (!v) return;
  slug.value = slugify(String(props.card.data.title ?? "")) || props.card.slug;
  error.value = null;
});

const problem = computed(() => {
  if (!SLUG_RE.test(slug.value)) return "Только латиница, цифры и дефис";
  if (slug.value !== props.card.slug && store.cards.has(slug.value)) return "Такой файл уже есть";
  return null;
});

async function submit() {
  if (problem.value || busy.value) return;
  if (slug.value === props.card.slug) return void (open.value = false);
  busy.value = true;
  try {
    await store.rename(props.card.slug, slug.value);
    emit("renamed", slug.value);
    open.value = false;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Имя файла</DialogTitle>
        <DialogDescription>Меняет адрес страницы подарка: старая ссылка перестанет открываться.</DialogDescription>
      </DialogHeader>
      <form class="grid gap-2" @submit.prevent="submit">
        <div class="flex items-center gap-1 font-mono text-sm">
          items/<Input id="rename-slug" v-model="slug" class="font-mono" autofocus />.md
        </div>
        <p v-if="problem || error" class="text-destructive text-xs">{{ problem ?? error }}</p>
        <DialogFooter class="mt-2">
          <Button type="button" variant="ghost" @click="open = false">Отмена</Button>
          <Button type="submit" :disabled="!!problem || busy">Переименовать</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
