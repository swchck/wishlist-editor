import { reactive } from "vue";
import { toast } from "vue-sonner";
import { useWishlist } from "@/stores/wishlist";

/** Pictures being compressed and stored right now, by card slug. */
export const uploading = reactive(new Map<string, number>());

/** Returns the pictures among dropped or pasted files; everything else is ignored. */
export function picturesOf(list: FileList | DataTransferItemList | null | undefined): File[] {
  if (!list) return [];
  const files =
    list instanceof FileList ? [...list] : [...list].flatMap((i) => (i.kind === "file" ? [i.getAsFile()!] : []));
  return files.filter((f) => f.type.startsWith("image/"));
}

/** Uploads pictures into the card and reports the ones that did not make it. */
export async function uploadPhotos(slug: string, pictures: File[]) {
  const store = useWishlist();
  if (!pictures.length || store.frozen) return;
  const bump = (by: number) => {
    const n = (uploading.get(slug) ?? 0) + by;
    if (n > 0) uploading.set(slug, n);
    else uploading.delete(slug);
  };
  bump(pictures.length);
  try {
    const failed = await store.addPhotos(slug, pictures);
    if (failed.length) {
      toast.error(failed.length === pictures.length ? "Фото не добавлено" : "Добавлены не все фото", {
        description: [...new Set(failed)].join("\n"),
      });
    }
  } finally {
    bump(-pictures.length);
  }
}
