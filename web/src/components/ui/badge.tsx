import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";

export function Badge({
  className,
  tone = "muted",
  ...props
}: ComponentProps<"span"> & {
  tone?: "muted" | "accent" | "danger" | "success" | "agent" | "info";
}) {
  const tones: Record<string, string> = {
    muted: "bg-surface-2 text-muted",
    accent: "bg-accent/15 text-accent",
    danger: "bg-danger/15 text-danger",
    success: "bg-success/15 text-success",
    agent: "bg-agent/15 text-agent",
    info: "bg-info/15 text-info",
  };
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium tracking-wide",
        tones[tone],
        className,
      )}
      {...props}
    />
  );
}
