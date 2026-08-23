import { createFileRoute } from "@tanstack/react-router";
import { couponBody } from "@/lib/server/subject";

export const Route = createFileRoute("/api/coupon")({
  server: {
    handlers: {
      POST: async () => Response.json(couponBody(), { status: 422 }),
    },
  },
});
