import { describe, expect, it } from "vitest";
import { SLUG_RE, slugify, uniqueSlug } from "./slug";

describe("slugify", () => {
  it.each([
    ["Playdate + Cover", "playdate-cover"],
    ["Кофемолка Comandante C40", "kofemolka-comandante-c40"],
    ["Щётка — ёжик!", "schetka-ezhik"],
    ["   ", ""],
  ])("%s → %s", (title, slug) => {
    expect(slugify(title)).toBe(slug);
  });

  it("never ends with a dash after truncation", () => {
    const s = slugify("a".repeat(59) + " b");
    expect(s.endsWith("-")).toBe(false);
    expect(SLUG_RE.test(s)).toBe(true);
  });
});

describe("uniqueSlug", () => {
  it("adds the first free suffix", () => {
    expect(uniqueSlug("a", new Set())).toBe("a");
    expect(uniqueSlug("a", new Set(["a", "a-2"]))).toBe("a-3");
    expect(uniqueSlug("", new Set())).toBe("item");
  });
});
