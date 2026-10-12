import { describe, expect, it } from "vitest";
import { commitMessage, describeChanges, plural, slugOfPath } from "./changes";

describe("slugOfPath", () => {
  it.each([
    ["src/content/items/playdate.md", "playdate"],
    ["src/content/items/images/a.webp", null],
    ["src/content/items/images/x.md", null],
    ["wishlist.config.json", null],
  ])("%s → %s", (path, slug) => expect(slugOfPath(path)).toBe(slug));
});

describe("commitMessage", () => {
  const titles: Record<string, string> = { a: "Playdate", b: "OP-1 field" };
  const d = (changes: Parameters<typeof describeChanges>[0]) => describeChanges(changes, (s) => titles[s]);

  it("lists cards by verb", () => {
    expect(
      commitMessage(
        d([
          { path: "src/content/items/a.md", kind: "added" },
          { path: "src/content/items/b.md", kind: "modified" },
          { path: "src/content/items/gone.md", kind: "deleted" },
        ]),
      ),
    ).toBe("Добавить: Playdate; обновить: OP-1 field; удалить: gone");
  });

  it("mentions non-card files", () => {
    expect(commitMessage(d([{ path: "src/content/items/images/a.webp", kind: "added" }]))).toBe("Обновить файлы: 1");
  });

  it("moves a long list into the body", () => {
    const many = Array.from({ length: 12 }, (_, i) => ({
      path: `src/content/items/card-${i}.md`,
      kind: "modified" as const,
    }));
    const msg = commitMessage(d(many));
    expect(msg.split("\n")[0]).toBe("Обновить вишлист: 12 карточек");
    expect(msg).toContain("Обновить: card-0, card-1");
  });
});

describe("plural", () => {
  it.each([
    [1, "карточка"],
    [3, "карточки"],
    [5, "карточек"],
    [11, "карточек"],
    [22, "карточки"],
  ])("%i %s", (n, word) => expect(plural(n, "карточка", "карточки", "карточек")).toBe(word));
});
