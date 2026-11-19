import "@fontsource-variable/inter/opsz.css";
import "@/styles/index.css";
import "@/stores/ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import { installSprings } from "@/lib/spring";

installSprings();

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
