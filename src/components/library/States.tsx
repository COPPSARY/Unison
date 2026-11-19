import { CircleAlert, SearchX, type LucideIcon } from "lucide-react";
import { EmptyState } from "@/components/shell/Page";
import { Button, Skeleton } from "@/components/ui";
import { format, strings } from "@/i18n";
import { errorMessage } from "@/lib/errors";

/** "No tracks match 'xyz'" + Clear filter. */
export function NoResults({ query, onClear, grid = false }: { query: string; onClear: () => void; grid?: boolean }) {
  return (
    <EmptyState
      icon={SearchX}
      body={format(grid ? strings.library.noResultsGrid : strings.library.noResults, { query })}
      action={
        <Button variant="secondary" size="sm" onClick={onClear}>
          {strings.common.clearFilter}
        </Button>
      }
    />
  );
}

/** "Couldn't load …" + the mapped reason + Retry. */
export function LoadError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  return (
    <div role="alert" className="flex flex-1">
      <EmptyState
        icon={CircleAlert}
        title={strings.library.errorLoad}
        body={errorMessage(error)}
        action={
          <Button variant="secondary" size="sm" onClick={onRetry}>
            {strings.library.retry}
          </Button>
        }
      />
    </div>
  );
}

export function Empty({ icon, title, body }: { icon: LucideIcon; title: string; body: string }) {
  return <EmptyState icon={icon} title={title} body={body} />;
}

/** Grid placeholder shaped like cover tiles. */
export function GridSkeleton({ round = false, count = 12 }: { round?: boolean; count?: number }) {
  return (
    <div aria-hidden="true" className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-x-4 gap-y-6">
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="flex flex-col gap-2">
          <Skeleton shape={round ? "circle" : "rect"} className="aspect-square w-full" />
          <Skeleton shape="text" className="w-3/4" />
          <Skeleton shape="text" className="w-1/2" />
        </div>
      ))}
    </div>
  );
}
