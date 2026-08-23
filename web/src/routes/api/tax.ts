import { createFileRoute } from "@tanstack/react-router";
import { taxBody } from "@/lib/server/subject";

export const Route = createFileRoute("/api/tax")({
  server: {
    handlers: {
      GET: async () => Response.json(taxBody(), { status: 500 }),
    },
  },
});
