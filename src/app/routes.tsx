import type { RouteObject } from "react-router";
import { DevUi } from "@/routes/dev/DevUi";
import { Placeholder } from "./Placeholder";

export const routes: RouteObject[] = [
  { path: "/", element: <Placeholder /> },
  ...(import.meta.env.DEV ? [{ path: "/dev/ui", element: <DevUi /> }] : []),
];
