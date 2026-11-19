import { createContext, useContext, type RefObject } from "react";

/** The shell's scrolling `<main>`, so virtualised lists can use it as their scroll element. */
export const ScrollContainerContext = createContext<RefObject<HTMLElement | null> | null>(null);

export function useScrollContainer(): RefObject<HTMLElement | null> | null {
  return useContext(ScrollContainerContext);
}
