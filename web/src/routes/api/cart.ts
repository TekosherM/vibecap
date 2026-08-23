import { createFileRoute } from "@tanstack/react-router";
import { getSql } from "@/lib/db";
import { cartBody } from "@/lib/server/subject";
import type { CatalogItem } from "@/lib/types";

export const Route = createFileRoute("/api/cart")({
  server: {
    handlers: {
      GET: async () => {
        const sql = await getSql();
        const items = await sql<CatalogItem>`
          select id, sku, name, price_cents, stock from catalog_items order by id
        `;
        return Response.json(cartBody(items));
      },
    },
  },
});
