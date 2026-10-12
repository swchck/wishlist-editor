import { describe, expect, it } from "vitest";
import { photosOf, validateItem } from "./schema";

describe("validateItem", () => {
  it("accepts a minimal card and fills defaults", () => {
    const res = validateItem({ title: "X" });
    expect(res).toMatchObject({ ok: true, data: { type: "вещь", priority: 3, hidden: false } });
  });

  it("reports errors by field path", () => {
    const res = validateItem({
      title: " ",
      price: -1,
      url: "ftp://x",
      deadline: "2026-13-40",
      links: [{ label: "a", url: "nope" }],
      image: "pic.jpg",
    });
    expect(res.ok).toBe(false);
    if (res.ok) return;
    expect(Object.keys(res.errors).sort()).toEqual(
      ["deadline", "image", "links.0.url", "price", "title", "url"].sort(),
    );
  });

  it("keeps unknown keys", () => {
    const res = validateItem({ title: "X", custom: 1 });
    expect(res.ok && res.data.custom).toBe(1);
  });
});

describe("photosOf", () => {
  it("merges image and images", () => {
    expect(photosOf({ image: "https://a", images: ["./b"] })).toEqual(["https://a", "./b"]);
    expect(photosOf({ image: ["https://a", "https://b"] })).toEqual(["https://a", "https://b"]);
    expect(photosOf({})).toEqual([]);
  });
});
