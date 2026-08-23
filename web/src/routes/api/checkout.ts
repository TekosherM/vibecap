import { createFileRoute } from "@tanstack/react-router";
import { listSessions, subjectPay } from "@/lib/server/evidence";

export const Route = createFileRoute("/api/checkout")({
  server: {
    handlers: {
      POST: async () => {
        const sessions = await listSessions();
        const sessionId = sessions[0]?.id;
        if (!sessionId) return Response.json({ error: "no session" }, { status: 500 });
        const result = await subjectPay({ data: { sessionId } });
        return Response.json(result, { status: 402 });
      },
    },
  },
});
