import { z } from "zod";

/** Item format version this editor reads and writes; must match `schemaVersion` in the repo. */
export const SCHEMA_VERSION = 1;

export const GIFT_TYPES = ["вещь", "впечатление", "навык", "услуга", "другое"] as const;
export type GiftType = (typeof GIFT_TYPES)[number];

export const GIFT_TYPE_LABELS: Record<GiftType, string> = {
  вещь: "Вещь",
  впечатление: "Впечатление",
  навык: "Навык",
  услуга: "Услуга",
  другое: "Другое",
};

export const PRIORITY_LABELS: Record<number, string> = {
  1: "Очень хочу",
  2: "Хочу",
  3: "Было бы здорово",
  4: "Приятная мелочь",
  5: "На всякий случай",
};

export const COMMON_CURRENCIES = ["RUB", "EUR", "USD", "RSD", "GBP"] as const;

export const ConfigSchema = z.object({
  schemaVersion: z.number().int(),
  title: z.string().min(1),
  defaultCurrency: z
    .string()
    .regex(/^[A-Z]{3}$/)
    .default("RUB"),
});
export type WishlistConfig = z.infer<typeof ConfigSchema>;

const httpUrl = z
  .string()
  .url("Нужна ссылка вида https://…")
  .refine((u) => /^https?:\/\//.test(u), "Ссылка должна начинаться с http(s)://");

// the site accepts a remote URL or a file next to the card (resolved by Astro's image())
const photoRef = z
  .string()
  .refine((v) => /^https?:\/\//.test(v) || /^\.\.?\//.test(v), "Картинка — это ссылка https://… или файл ./images/…");

const isoDate = z
  .string()
  .regex(/^\d{4}-\d{2}-\d{2}$/, "Дата в формате ГГГГ-ММ-ДД")
  .refine((d) => !Number.isNaN(Date.parse(d)), "Такой даты нет");

/**
 * Card frontmatter as the site's content collection accepts it (schemaVersion 1).
 * Mirrors `src/content.config.ts` in the template; unknown keys pass through untouched.
 */
export const ItemSchema = z
  .object({
    title: z.string().trim().min(1, "Без названия никак"),
    type: z.enum(GIFT_TYPES).default("вещь"),
    category: z.string().optional(),
    price: z.number().nonnegative("Цена не может быть отрицательной").optional(),
    currency: z
      .string()
      .regex(/^[A-Z]{3}$/, "Код валюты из трёх латинских букв, напр. EUR")
      .optional(),
    priority: z.number().int().min(1).max(5).default(3),
    deadline: isoDate.optional(),
    url: httpUrl.optional(),
    links: z.array(z.object({ label: z.string().min(1, "Подпись ссылки"), url: httpUrl })).optional(),
    image: z.union([photoRef, z.array(photoRef)]).optional(),
    images: z.array(photoRef).optional(),
    description: z.string().optional(),
    hidden: z.boolean().default(false),
  })
  .passthrough();

export type ItemData = z.infer<typeof ItemSchema>;

/** Field-path → first error message, e.g. `{ "links.0.url": "…" }`. */
export type FieldErrors = Record<string, string>;

export function validateItem(data: unknown): { ok: true; data: ItemData } | { ok: false; errors: FieldErrors } {
  const res = ItemSchema.safeParse(data);
  if (res.success) return { ok: true, data: res.data };
  const errors: FieldErrors = {};
  for (const issue of res.error.issues) {
    const key = issue.path.join(".") || "_";
    errors[key] ??= issue.message;
  }
  return { ok: false, errors };
}

/** Returns every photo of a card in display order, whichever of `image`/`images` holds them. */
export function photosOf(data: Pick<ItemData, "image" | "images">): string[] {
  const fromImage = data.image === undefined ? [] : Array.isArray(data.image) ? data.image : [data.image];
  return [...fromImage, ...(data.images ?? [])];
}
