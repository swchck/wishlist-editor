<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { toast } from "vue-sonner";
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
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { errorMessage } from "@/lib/api";
import { GIFT_TYPES, GIFT_TYPE_LABELS, type GiftType } from "@/lib/wishlist/schema";
import { slugify, uniqueSlug } from "@/lib/wishlist/slug";
import { useWishlist } from "@/stores/wishlist";

const open = defineModel<boolean>("open", { required: true });
const store = useWishlist();
const title = ref("");
const type = ref<GiftType>("вещь");
const busy = ref(false);

watch(open, (v) => {
  if (v) {
    title.value = "";
    type.value = "вещь";
  }
});

const slug = computed(() => uniqueSlug(slugify(title.value), new Set(store.cards.keys())));

async function submit() {
  if (!title.value.trim() || busy.value) return;
  busy.value = true;
  try {
    await store.create(title.value, type.value);
    open.value = false;
  } catch (e) {
    toast.error("Не удалось создать карточку", { description: errorMessage(e) });
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Новая карточка</DialogTitle>
        <DialogDescription>Остальные поля заполните в редакторе.</DialogDescription>
      </DialogHeader>
      <form class="grid gap-4" @submit.prevent="submit">
        <div class="grid gap-1.5">
          <Label for="new-title">Название</Label>
          <Input id="new-title" v-model="title" placeholder="Наушники Sony WH-1000XM5" autofocus />
          <p class="text-muted-foreground font-mono text-xs">items/{{ slug }}.md</p>
        </div>
        <div class="grid gap-1.5">
          <Label>Тип</Label>
          <ToggleGroup v-model="type" type="single" variant="outline" class="w-full">
            <ToggleGroupItem
              v-for="t in GIFT_TYPES"
              :key="t"
              :value="t"
              class="data-[state=on]:bg-foreground data-[state=on]:text-background flex-1 px-1 text-xs"
            >
              {{ GIFT_TYPE_LABELS[t] }}
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
        <DialogFooter>
          <Button type="button" variant="ghost" @click="open = false">Отмена</Button>
          <Button type="submit" :disabled="!title.trim() || busy">Создать</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
