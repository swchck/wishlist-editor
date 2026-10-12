import { ref } from "vue";
import { toast } from "vue-sonner";
import { useWishlist } from "@/stores/wishlist";

/** Pictures being compressed and stored right now, across every drop zone. */
export const uploading = ref(0);

/** Returns the pictures among dropped or pasted files; everything else is ignored. */
export function picturesOf(list: FileList | DataTransferItemList | null | undefined): File[] {
  if (!list) return [];
  const files =
    list instanceof FileList ? [...list] : [...list].flatMap((i) => (i.kind === "file" ? [i.getAsFile()!] : []));
  return files.filter((f) => f.type.startsWith("image/"));
}

/** Uploads pictures into the card and reports the ones that did not make it. */
export async function uploadPhotos(slug: string, pictures: File[]) {
  if (!pictures.length) return;
  const store = useWishlist();
  uploading.value += pictures.length;
  try {
    const failed = await store.addPhotos(slug, pictures);
    if (failed.length) {
      toast.error(failed.length === pictures.length ? "Фото не добавлено" : "Добавлены не все фото", {
        description: [...new Set(failed)].join("\n"),
      });
    }
  } finally {
    uploading.value -= pictures.length;
  }
}
