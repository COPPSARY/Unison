import { strings } from "@/i18n";

export function App() {
  return (
    <main className="flex h-full flex-col items-center justify-center gap-2 bg-background">
      <h1 className="font-display text-3xl font-medium tracking-[-0.02em] text-text">
        {strings.app.name}
      </h1>
      <p className="text-text-muted">{strings.app.tagline}</p>
    </main>
  );
}
