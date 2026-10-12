const SYMBOLS: Record<string, string> = { RUB: "₽", EUR: "€", USD: "$", GBP: "£", RSD: "дин." };

/** Formats a price the way cards show it: grouped digits and the currency sign. */
export function formatPrice(price: unknown, currency: unknown): string | null {
  if (typeof price !== "number") return null;
  const cur = typeof currency === "string" ? currency : "";
  const num = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 2 }).format(price);
  return `${num} ${SYMBOLS[cur] ?? cur}`.trim();
}

/** Returns up to three letters for a card's placeholder thumbnail. */
export function initials(title: string): string {
  const words = title.split(/[\s—–-]+/).filter((w) => /[\p{L}\p{N}]/u.test(w));
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return words
    .slice(0, 2)
    .map((w) => w[0])
    .join("")
    .toUpperCase();
}
