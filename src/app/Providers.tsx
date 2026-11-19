import { QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { GlassFilters, Toaster, TooltipProvider } from "@/components/ui";
import { createQueryClient } from "@/lib/queries";

export function Providers({ children }: { children: ReactNode }) {
  const [queryClient] = useState(createQueryClient);
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider delayDuration={500}>
        <GlassFilters />
        <Toaster>{children}</Toaster>
      </TooltipProvider>
    </QueryClientProvider>
  );
}
