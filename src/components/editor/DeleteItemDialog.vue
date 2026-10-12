<script setup lang="ts">
import { ref } from "vue";
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
import { errorMessage } from "@/lib/api";
import { useWishlist, type Card } from "@/stores/wishlist";

const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ card: Card }>();
const store = useWishlist();
const busy = ref(false);

async function confirm() {
  busy.value = true;
  try {
    await store.remove(props.card.slug);
    open.value = false;
  } catch (e) {
    toast.error("Не удалось удалить", { description: errorMessage(e) });
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Удалить «{{ card.data.title ?? card.slug }}»?</DialogTitle>
        <DialogDescription>
          Файл {{ card.slug }}.md будет удалён из папки. Если хотите убрать подарок только с сайта, включите «Спрятать с
          сайта».
        </DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="ghost" @click="open = false">Отмена</Button>
        <Button variant="destructive" :disabled="busy" @click="confirm">Удалить</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
