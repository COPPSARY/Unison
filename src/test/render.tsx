import { render } from "@testing-library/react";
import { MemoryRouter, useRoutes } from "react-router";
import { Providers } from "@/app/Providers";
import { routes } from "@/app/routes";

function AppRoutes() {
  return useRoutes(routes);
}

/**
 * Renders the app's route table at `path`. Uses MemoryRouter + useRoutes rather than a data
 * router: under jsdom the data router's fetch Request rejects jsdom's AbortSignal.
 */
export function renderRoute(path: string) {
  return render(
    <Providers>
      <MemoryRouter initialEntries={[path]}>
        <AppRoutes />
      </MemoryRouter>
    </Providers>,
  );
}
