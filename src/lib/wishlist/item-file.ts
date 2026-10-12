import { Document, isMap, parseDocument, YAMLMap } from "yaml";

/**
 * A card file split into its YAML frontmatter and markdown body.
 *
 * Edits go through the YAML `Document`, so untouched keys keep their quoting, comments
 * and order; serializing an unedited card reproduces the file byte for byte.
 */
export interface ItemFile {
  doc: Document;
  body: string;
}

const FENCE = /^---\r?\n([\s\S]*?)\r?\n?---(?:\r?\n|$)/;

// `lineWidth: 0` stops yaml from folding long descriptions onto several lines
const STRINGIFY = { lineWidth: 0, minContentWidth: 0 } as const;

/** Order new keys are inserted in, matching how the site's README documents a card. */
export const KEY_ORDER = [
  "title",
  "type",
  "category",
  "price",
  "currency",
  "priority",
  "deadline",
  "url",
  "links",
  "image",
  "images",
  "hidden",
  "description",
] as const;

export class ItemParseError extends Error {}

export function parseItem(text: string): ItemFile {
  const m = FENCE.exec(text);
  if (!m) throw new ItemParseError("Нет блока --- с полями карточки в начале файла");
  const doc = parseDocument(m[1], { keepSourceTokens: false });
  if (doc.errors.length) throw new ItemParseError(doc.errors[0].message);
  if (doc.contents !== null && !isMap(doc.contents)) {
    throw new ItemParseError("Поля карточки должны быть списком «ключ: значение»");
  }
  return { doc, body: text.slice(m[0].length) };
}

export function serializeItem({ doc, body }: ItemFile): string {
  const yaml = doc.contents === null ? "" : doc.toString(STRINGIFY);
  return `---\n${yaml}---\n${body}`;
}

/** Returns the frontmatter as plain JS values. */
export function itemData(file: ItemFile): Record<string, unknown> {
  return (file.doc.toJS() as Record<string, unknown> | null) ?? {};
}

/**
 * Sets a frontmatter key; `undefined`, `""` and empty arrays delete it.
 * A new key lands at its `KEY_ORDER` position rather than at the end.
 */
export function setField(file: ItemFile, key: string, value: unknown): void {
  const empty = value === undefined || value === "" || (Array.isArray(value) && value.length === 0);
  if (empty) {
    file.doc.delete(key);
    return;
  }
  if (file.doc.contents === null) file.doc.contents = new YAMLMap();
  if (file.doc.has(key)) {
    file.doc.set(key, value);
    return;
  }
  const map = file.doc.contents as YAMLMap;
  const pair = file.doc.createPair(key, value);
  const rank = (k: unknown) => {
    const i = KEY_ORDER.indexOf(String(k) as (typeof KEY_ORDER)[number]);
    return i === -1 ? KEY_ORDER.length : i;
  };
  const at = map.items.findIndex((p) => rank((p.key as { value?: unknown })?.value ?? p.key) > rank(key));
  if (at === -1) map.items.push(pair);
  else map.items.splice(at, 0, pair);
}

/** Builds the text of a brand-new card. */
export function newItemText(fields: Record<string, unknown>, body = ""): string {
  const file: ItemFile = { doc: new Document({}), body };
  for (const key of KEY_ORDER) if (key in fields) setField(file, key, fields[key]);
  return serializeItem(file);
}

/**
 * Merges markdown coming back from the WYSIWYG editor into the stored body.
 * Keeps the blank lines the file had right after the closing `---`, which the
 * editor drops, so a one-word edit stays a one-line diff.
 */
export function mergeBody(previous: string, edited: string): string {
  const lead = /^(?:[ \t]*\r?\n)*/.exec(previous)![0];
  const text = edited.replace(/^\s+/, "");
  return text ? `${lead}${text.replace(/\s*$/, "")}\n` : "";
}
