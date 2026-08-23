import type { CatalogItem } from "@/lib/types";

export function cartBody(items: CatalogItem[]) {
  const subtotal_cents = items.reduce((n, i) => n + i.price_cents, 0);
  return {
    items: items.map((i) => ({
      sku: i.sku,
      name: i.name,
      price_cents: i.price_cents,
      stock: i.stock,
      qty: 1,
    })),
    subtotal_cents,
    ui_total_cents: 4100,
    mismatch: subtotal_cents !== 4100,
  };
}

export function taxBody() {
  return {
    error: "tax is undefined",
    helper: "pricing.ts:88",
    zip: "94107",
  };
}
