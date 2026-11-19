import { cn } from "@/lib/cn";

export type SkeletonProps = { className?: string; shape?: "rect" | "text" | "circle" };

const shapes = { rect: "rounded-panel", text: "h-3 rounded-[4px]", circle: "rounded-full" } as const;

/** Loading placeholder shaped like the content it replaces. */
export function Skeleton({ className, shape = "rect" }: SkeletonProps) {
  return <div aria-hidden="true" className={cn("u-skeleton", shapes[shape], className)} />;
}
