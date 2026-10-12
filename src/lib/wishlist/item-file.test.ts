import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { itemData, mergeBody, newItemText, parseItem, serializeItem, setField, ItemParseError } from "./item-file";

const FIXTURES = join(__dirname, "__fixtures__");
const fixtures = readdirSync(FIXTURES).map((f) => [f, readFileSync(join(FIXTURES, f), "utf-8")] as const);

// opt-in check against a real wishlist checkout: WISHLIST_ITEMS=/path/to/src/content/items
const extraDir = process.env.WISHLIST_ITEMS;
const extra = extraDir
  ? readdirSync(extraDir)
      .filter((f) => f.endsWith(".md"))
      .map((f) => [f, readFileSync(join(extraDir, f), "utf-8")] as const)
  : [];

describe("round trip", () => {
  it.each([...fixtures, ...extra])("%s is reproduced byte for byte", (_, text) => {
    expect(serializeItem(parseItem(text))).toBe(text);
  });
});

describe("setField", () => {
  const tricky = fixtures.find(([f]) => f === "tricky.md")![1];

  it("changes one value and leaves the rest of the file alone", () => {
    const file = parseItem(tricky);
    setField(file, "price", 2600);
    const out = serializeItem(file);
    expect(out).toBe(tricky.replace("price: 2450", "price: 2600"));
  });

  it("inserts a new key at its canonical position", () => {
    const file = parseItem(tricky);
    setField(file, "category", "Уход");
    const keys = Object.keys(itemData(file));
    expect(keys.indexOf("category")).toBe(keys.indexOf("type") + 1);
  });

  it("deletes on empty values", () => {
    const file = parseItem(tricky);
    setField(file, "description", "");
    setField(file, "links", []);
    const data = itemData(file);
    expect(data).not.toHaveProperty("description");
    expect(data).not.toHaveProperty("links");
    expect(data.custom_key).toBe("keep me");
  });
});

describe("parseItem", () => {
  it("rejects a file without frontmatter", () => {
    expect(() => parseItem("# just markdown")).toThrow(ItemParseError);
  });

  it("rejects a non-map frontmatter", () => {
    expect(() => parseItem("---\n- a\n- b\n---\n")).toThrow(ItemParseError);
  });

  it("handles an empty frontmatter and body", () => {
    const file = parseItem("---\n---\n");
    expect(itemData(file)).toEqual({});
    expect(file.body).toBe("");
  });
});

describe("newItemText", () => {
  it("writes keys in canonical order and parses back", () => {
    const text = newItemText({ priority: 2, title: "Playdate", type: "вещь" }, "Тело\n");
    expect(text).toBe("---\ntitle: Playdate\ntype: вещь\npriority: 2\n---\nТело\n");
    expect(itemData(parseItem(text))).toEqual({ title: "Playdate", type: "вещь", priority: 2 });
  });
});

describe("mergeBody", () => {
  it("keeps the blank line after the frontmatter", () => {
    expect(mergeBody("\nСтарый текст.\n", "Новый текст.\n")).toBe("\nНовый текст.\n");
  });

  it("normalizes trailing whitespace to one newline", () => {
    expect(mergeBody("A\n", "B\n\n\n")).toBe("B\n");
  });

  it("returns an empty body when everything was deleted", () => {
    expect(mergeBody("\nA\n", "\n")).toBe("");
  });
});
