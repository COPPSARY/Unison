import type { ReactNode } from "react";
import { GlassFilters, Toaster, TooltipProvider } from "@/components/ui";

export function Providers({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider delayDuration={500}>
      <GlassFilters />
      <Toaster>{children}</Toaster>
    </TooltipProvider>
  );
}
