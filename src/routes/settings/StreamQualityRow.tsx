import { useState } from "react";
import { Select } from "@/components/ui";
import { strings } from "@/i18n";
import { commands, type StreamQuality } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { useUiStore } from "@/stores/ui";

const t = strings.settings.streamQuality;

const OPTIONS: Array<{ value: StreamQuality; label: string }> = [
  { value: "best", label: t.best },
  { value: "lossless", label: t.lossless },
  { value: "high", label: t.high },
  { value: "low", label: t.low },
];

/** Settings → Playback → Stream quality: the tier Unison asks extensions for (default High). */
export function StreamQualityRow() {
  const saved = useUiStore((st) => st.settings?.player.streamQuality ?? "high");
  const hydrate = useUiStore((st) => st.hydrate);
  const [error, setError] = useState<string | null>(null);

  const choose = (value: string) => {
    setError(null);
    commands
      .settingsGet()
      .then((s) => commands.settingsSet({ ...s, player: { ...s.player, streamQuality: value as StreamQuality } }))
      .then(hydrate)
      .catch((err: unknown) => setError(errorMessage(err)));
  };

  return (
    <div className="flex flex-col gap-1 px-4 py-3">
      <div className="flex items-center justify-between gap-4">
        <span className="text-body text-text">{t.title}</span>
        <div className="w-64">
          <Select label={t.title} value={saved} onValueChange={choose} options={OPTIONS} />
        </div>
      </div>
      <p className="text-footnote text-text-muted">{t.hint}</p>
      {error ? (
        <p role="alert" className="text-callout text-danger">
          {error}
        </p>
      ) : null}
    </div>
  );
}
