import { Search } from "lucide-react";
import { Icon, Input, Select } from "@/components/ui";
import { strings } from "@/i18n";
import type { LibrarySort } from "@/lib/bindings";
import { TRACK_SORTS, type LibraryControls, type SortOption } from "./controls";

/** Text filter and sort (spec §3 Toolbar). */
export function LibraryToolbar({ controls, sorts = TRACK_SORTS }: { controls: LibraryControls; sorts?: SortOption[] }) {
  return (
    <div className="flex flex-wrap items-center gap-3 pb-4">
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
