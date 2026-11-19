import type { ReactNode } from "react";
import { Toaster, TooltipProvider } from "@/components/ui";

export function Providers({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider delayDuration={400}>
      <Toaster>{children}</Toaster>
    </TooltipProvider>
  );
}
