import { Link } from "react-router";
import { strings } from "@/i18n";

/** Temporary home until the shell lands in T-04. */
export function Placeholder() {
  return (
    <main className="flex h-full flex-col items-center justify-center gap-1.5 bg-background">
      <h1 className="text-large-title text-text">{strings.app.name}</h1>
      <p className="text-body text-text-muted">{strings.app.tagline}</p>
      {import.meta.env.DEV ? (
        <Link to="/dev/ui" className="u-tone mt-4 text-callout text-text-muted hover:text-text">
          {strings.devUi.title}
        </Link>
      ) : null}
    </main>
  );
}
