import { useQuery } from "@tanstack/react-query";
import { Volume2 } from "lucide-react";
import { useState } from "react";
import { Button, Icon, Select } from "@/components/ui";
import { strings } from "@/i18n";
import { commands } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { isTauri } from "@/lib/window";
import { useUiStore } from "@/stores/ui";

const t = strings.settings.output;
/** Select needs a string value; this one means "system default" (null in settings). */
const DEFAULT = "__default__";

/** Settings → Playback → Output device + Test sound (UP-05, NativeEngine). */
export function OutputRow() {
  const outputs = useQuery({
    queryKey: ["playerOutputs"],
    queryFn: () => commands.playerOutputs(),
    enabled: isTauri(),
  });
  const saved = useUiStore((st) => st.settings?.player.outputDevice ?? null);
  const hydrate = useUiStore((st) => st.hydrate);
  const [error, setError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  const choose = (value: string) => {
    setError(null);
    const device = value === DEFAULT ? null : value;
    commands
      .playerSetOutput(device)
      // Re-read settings so later UI saves carry the new device.
      .then(() => commands.settingsGet())
      .then(hydrate)
      .catch((err: unknown) => setError(errorMessage(err)));
  };

  const testSound = () => {
    setError(null);
    setTesting(true);
    commands
      .playerTestSound()
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setTimeout(() => setTesting(false), 1500));
  };

  const names = outputs.data ?? [];
  const options = [
    { value: DEFAULT, label: t.systemDefault },
    // Keep a saved device visible even if it is unplugged right now.
    ...(saved && !names.includes(saved) ? [saved] : []).concat(names).map((n) => ({ value: n, label: n })),
  ];

  return (
    <div className="flex flex-col gap-2 px-4 py-3">
      <div className="flex items-center justify-between gap-4">
        <span className="text-body text-text">{t.title}</span>
        <div className="flex items-center gap-2">
          <div className="w-64">
            <Select label={t.title} value={saved ?? DEFAULT} onValueChange={choose} options={options} />
          </div>
          <Button variant="secondary" size="sm" onClick={testSound} disabled={testing}>
            <Icon as={Volume2} /> {t.test}
          </Button>
        </div>
      </div>
      {error ? (
        <p role="alert" className="text-callout text-danger">
          {error}
        </p>
      ) : null}
    </div>
  );
}
