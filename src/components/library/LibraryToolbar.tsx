import { Search } from "lucide-react";
import { Icon, Input, Segmented, Select } from "@/components/ui";
import { strings } from "@/i18n";
import type { LibrarySort } from "@/lib/bindings";
import { TRACK_SORTS, type LibraryControls, type SortOption, type SourceFilter } from "./controls";

/** Source filter (All / Spotify / YouTube), text filter and sort (spec §3 Toolbar). */
export function LibraryToolbar({ controls, sorts = TRACK_SORTS }: { controls: LibraryControls; sorts?: SortOption[] }) {
  return (
    <div className="flex flex-wrap items-center gap-3 pb-4">
      <Segmented<SourceFilter>
        label={strings.common.source}
        value={controls.source}
        onValueChange={controls.setSource}
        options={[
          { value: "all", label: strings.source.all },
          { value: "spotify", label: strings.source.spotify },
          { value: "youtube", label: strings.source.youtube },
        ]}
      />
      <div className="w-60">
        <Input
          shape="capsule"
          leading={<Icon as={Search} />}
          placeholder={strings.library.filterPlaceholder}
          aria-label={strings.common.filter}
          value={controls.filter}
          onChange={(e) => controls.setFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") controls.setFilter("");
          }}
        />
      </div>
      <div className="ml-auto w-44">
        <Select
          label={strings.common.sort}
          value={controls.sort}
          onValueChange={(v) => controls.setSort(v as LibrarySort)}
          options={sorts}
        />
      </div>
    </div>
  );
}
