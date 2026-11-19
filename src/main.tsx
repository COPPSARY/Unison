import "@fontsource-variable/inter/opsz.css";
import "@/styles/index.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import { installSprings } from "@/lib/spring";
import { loadSettings } from "@/stores/ui";

installSprings();

const root = document.getElementById("root");
if (root) {
  // Apply saved theme/frost before the first paint to avoid a flash.
  void loadSettings().finally(() => {
    createRoot(root).render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  });
}
