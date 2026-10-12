import type { Change } from "@/lib/api";

export const ITEMS_PREFIX = "src/content/items/";

/** Returns the card slug for a changed path, or `null` for any other file. */
export function slugOfPath(path: string): string | null {
  if (!path.startsWith(ITEMS_PREFIX) || !path.endsWith(".md")) return null;
  const rest = path.slice(ITEMS_PREFIX.length, -".md".length);
  return rest.includes("/") ? null : rest;
}

export interface DescribedChange extends Change {
  slug: string | null;
  /** Card title when known, otherwise the slug or the file path. */
  label: string;
}

export function describeChanges(changes: Change[], titleOf: (slug: string) => string | undefined): DescribedChange[] {
  return changes.map((c) => {
    const slug = slugOfPath(c.path);
    const label = slug ? (titleOf(slug) ?? slug) : c.path;
    return { ...c, slug, label };
  });
}

const VERBS: Record<Change["kind"], string> = {
  added: "Добавить",
  modified: "Обновить",
  deleted: "Удалить",
};

// GitHub truncates commit subjects past ~72 characters in its lists
const SUBJECT_MAX = 72;

/** Builds a commit message like «Добавить: Playdate; обновить: OP-1 field». */
export function commitMessage(changes: DescribedChange[]): string {
  if (!changes.length) return "Обновить вишлист";
  const cards = changes.filter((c) => c.slug);
  const other = changes.length - cards.length;
  const parts = (["added", "modified", "deleted"] as const)
    .map((kind) => {
      const names = cards.filter((c) => c.kind === kind).map((c) => c.label);
      return names.length ? `${VERBS[kind]}: ${names.join(", ")}` : null;
    })
    .filter((p): p is string => p !== null);
  if (other) parts.push(`${parts.length ? "файлы" : "Обновить файлы"}: ${other}`);
  const subject = parts.map((p, i) => (i ? p[0].toLowerCase() + p.slice(1) : p)).join("; ");
  if (subject.length <= SUBJECT_MAX) return subject;
  const summary = `Обновить вишлист: ${cards.length} ${plural(cards.length, "карточка", "карточки", "карточек")}`;
  return `${summary}\n\n${parts.join("\n")}`;
}

export function plural(n: number, one: string, few: string, many: string): string {
  const m10 = n % 10;
  const m100 = n % 100;
  if (m10 === 1 && m100 !== 11) return one;
  if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return few;
  return many;
}
